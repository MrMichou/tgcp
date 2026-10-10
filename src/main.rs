mod app;
mod config;
mod event;
mod gcp;
mod notification;
mod resource;
mod shell;
mod task;
mod theme;
mod ui;

/// Version injected at compile time via TGCP_VERSION env var (set by CI/CD),
/// or "dev" for local builds.
pub const VERSION: &str = match option_env!("TGCP_VERSION") {
    Some(v) => v,
    None => "dev",
};

use anyhow::Result;
use app::App;
use clap::{Parser, ValueEnum};
use config::Config;
use crossterm::{
    event::{
        poll, read, DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use gcp::auth;
use ratatui::prelude::*;
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use tracing::Level;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use ui::splash::{render as render_splash, SplashState};

/// Terminal UI for GCP
#[derive(Parser, Debug)]
#[command(name = "tgcp", version, about, long_about = None)]
struct Args {
    /// GCP project to use
    #[arg(short, long)]
    project: Option<String>,

    /// GCP zone to use
    #[arg(short, long)]
    zone: Option<String>,

    /// Log level for debugging
    #[arg(long, value_enum, default_value = "off")]
    log_level: LogLevel,

    /// Run in read-only mode (block all write operations)
    #[arg(long)]
    readonly: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    fn to_tracing_level(self) -> Option<Level> {
        match self {
            LogLevel::Off => None,
            LogLevel::Error => Some(Level::ERROR),
            LogLevel::Warn => Some(Level::WARN),
            LogLevel::Info => Some(Level::INFO),
            LogLevel::Debug => Some(Level::DEBUG),
            LogLevel::Trace => Some(Level::TRACE),
        }
    }
}

fn setup_logging(level: LogLevel) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let tracing_level = level.to_tracing_level()?;

    let log_path = get_log_path();

    if let Some(parent) = log_path.parent() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        // Security: log directory is owner-only (0700)
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        let _ = builder.create(parent);
    }

    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    // Security: log file is owner-only (0600) since it may contain resource names
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let file = match options.open(&log_path) {
        Ok(f) => {
            // Tighten permissions on log files created by older versions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
            }
            f
        },
        Err(e) => {
            eprintln!("Warning: could not open log file {:?}: {}", log_path, e);
            return None;
        },
    };

    let (non_blocking, guard) = tracing_appender::non_blocking(file);

    tracing_subscriber::fmt()
        .with_max_level(tracing_level)
        .with_writer(non_blocking.with_max_level(tracing_level))
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    tracing::info!("tgcp started with log level: {:?}", level);
    tracing::info!("Log file: {:?}", log_path);

    Some(guard)
}

fn get_log_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir.join("tgcp").join("tgcp.log");
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".tgcp").join("tgcp.log");
    }
    PathBuf::from("tgcp.log")
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let _log_guard = setup_logging(args.log_level);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    // Install panic hook to restore terminal state before printing panic info.
    // Without this, a panic leaves the terminal in raw mode (no echo, no line editing).
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize and run
    let result = initialize_with_splash(&mut terminal, &args).await;

    match result {
        Ok(Some(mut app)) => {
            let run_result = run_app(&mut terminal, &mut app).await;
            cleanup_terminal(&mut terminal)?;

            if let Err(err) = run_result {
                eprintln!("Error: {err:?}");
            }
        },
        Ok(None) => {
            cleanup_terminal(&mut terminal)?;
        },
        Err(err) => {
            cleanup_terminal(&mut terminal)?;
            eprintln!("Initialization error: {err:?}");
        },
    }

    Ok(())
}

fn cleanup_terminal<B: Backend + std::io::Write>(terminal: &mut Terminal<B>) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

async fn initialize_with_splash<B: Backend>(
    terminal: &mut Terminal<B>,
    args: &Args,
) -> Result<Option<App>>
where
    B::Error: Send + Sync + 'static,
{
    let mut splash = SplashState::new();

    // Render initial splash
    terminal.draw(|f| render_splash(f, &splash))?;

    if check_abort()? {
        return Ok(None);
    }

    // Step 1: Load configuration
    let config = Config::load();

    // Security: validate CLI-provided identifiers before they reach URLs or gcloud
    if let Some(project) = &args.project {
        if !auth::validate_project_id(project) {
            return Err(anyhow::anyhow!("Invalid project ID: {:?}", project));
        }
    }
    if let Some(zone) = &args.zone {
        if !auth::validate_zone_name(zone) {
            return Err(anyhow::anyhow!("Invalid zone: {:?}", zone));
        }
    }

    let project = args
        .project
        .clone()
        .unwrap_or_else(|| config.effective_project());
    let zone = args.zone.clone().unwrap_or_else(|| config.effective_zone());

    if project.is_empty() {
        splash.set_message("Error: No project configured");
        terminal.draw(|f| render_splash(f, &splash))?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        return Err(anyhow::anyhow!(
            "No GCP project configured. Set GOOGLE_CLOUD_PROJECT or use --project flag"
        ));
    }

    tracing::info!("Using project: {}, zone: {}", project, zone);

    splash.set_message(&format!("Loading GCP config [project: {}]", project));
    terminal.draw(|f| render_splash(f, &splash))?;
    splash.complete_step();

    if check_abort()? {
        return Ok(None);
    }

    // Step 2: Initialize GCP client
    splash.set_message(&format!("Connecting to GCP [{}]", zone));
    terminal.draw(|f| render_splash(f, &splash))?;

    let client = gcp::client::GcpClient::new(&project, &zone).await?;
    splash.complete_step();

    if check_abort()? {
        return Ok(None);
    }

    // Steps 3-5: Fetch projects, zones and the first page of instances concurrently
    splash.set_message(&format!(
        "Fetching projects, zones and instances [{}]",
        zone
    ));
    terminal.draw(|f| render_splash(f, &splash))?;

    let (projects_result, zones_result, instances_result) = tokio::join!(
        gcp::projects::list_project_ids(&client),
        client.list_zones(),
        resource::fetch_resources_paginated("compute-instances", &client, &[], None),
    );

    let available_projects = match projects_result {
        Ok(projects) if !projects.is_empty() => {
            tracing::info!("Loaded {} projects", projects.len());
            projects
        },
        Ok(_) => {
            tracing::warn!("No projects returned, using current project only");
            vec![project.clone()]
        },
        Err(e) => {
            tracing::warn!("Failed to list projects: {}, using current project only", e);
            vec![project.clone()]
        },
    };
    splash.complete_step();

    let available_zones = match zones_result {
        Ok(zones) if !zones.is_empty() => {
            tracing::info!("Loaded {} zones", zones.len());
            // Add "all" as first option to show all zones
            let mut all_zones = vec!["all".to_string()];
            all_zones.extend(zones);
            all_zones
        },
        Ok(_) => {
            tracing::warn!("No zones returned, using static list");
            let mut all_zones = vec!["all".to_string()];
            all_zones.extend(auth::list_zones());
            all_zones
        },
        Err(e) => {
            tracing::warn!("Failed to list zones: {}, using static list", e);
            let mut all_zones = vec!["all".to_string()];
            all_zones.extend(auth::list_zones());
            all_zones
        },
    };
    splash.complete_step();

    // Only the first page is loaded; the app paginates from there
    let (instances, initial_next_token, initial_error) = match instances_result {
        Ok(page) => (page.items, page.next_token, None),
        Err(e) => {
            let error_msg = gcp::client::format_gcp_error(&e);
            (Vec::new(), None, Some(error_msg))
        },
    };

    if check_abort()? {
        return Ok(None);
    }

    splash.complete_step();
    splash.set_message("Ready!");
    terminal.draw(|f| render_splash(f, &splash))?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut app = App::from_initialized(
        client,
        project,
        zone,
        available_projects,
        available_zones,
        instances,
        initial_next_token,
        config,
        args.readonly,
    );

    if let Some(err) = initial_error {
        app.error_message = Some(err);
    }

    Ok(Some(app))
}

fn check_abort() -> Result<bool> {
    if poll(Duration::from_millis(50))? {
        if let Event::Key(key) = read()? {
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Interval of the UI tick, used for time-based UI (toasts, spinners) and
/// for scheduling operation polls
const TICK_INTERVAL_MS: u64 = 200;

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    let mut task_results = app
        .take_task_receiver()
        .ok_or_else(|| anyhow::anyhow!("Task receiver already taken"))?;
    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(TICK_INTERVAL_MS));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // The startup page is loaded without metrics; fetch them now
    app.start_metrics_fetch();

    // Track toast visibility so the frame where a toast expires is redrawn
    let mut toast_was_visible = false;

    loop {
        let toast_visible = app.notification_manager.current_toast().is_some();
        if app.needs_redraw() || toast_was_visible {
            terminal.draw(|f| ui::render(f, app))?;
            app.dirty = false;
        }
        toast_was_visible = toast_visible;

        // Network calls run in background tasks: wait for whichever comes
        // first between user input, a task result and the UI tick
        tokio::select! {
            event = events.next() => match event {
                Some(Ok(event)) => {
                    if event::handle_event(app, event)? {
                        return Ok(());
                    }
                },
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(()),
            },
            Some(result) = task_results.recv() => app.handle_task_result(result),
            _ = tick.tick() => {},
        }

        if let Some(request) = app.pending_ssh.take() {
            // Release the event stream so it doesn't read keys meant for ssh
            drop(events);
            event::execute_ssh_to_instance(app, &request);
            events = EventStream::new();
            terminal.clear()?;
            app.mark_dirty();
        }

        // Start polling pending operations that are due
        app.poll_pending_operations();

        // Auto-refresh (disabled by default)
        if app.needs_refresh() && !app.loading {
            app.refresh_current();
        }
    }
}
