mod app;
mod config;
mod event;
mod gcp;
mod resource;
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
    event::{poll, read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
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
    let Some(tracing_level) = level.to_tracing_level() else {
        return None;
    };

    let log_path = get_log_path();

    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("Failed to open log file");

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
        }
        Ok(None) => {
            cleanup_terminal(&mut terminal)?;
        }
        Err(err) => {
            cleanup_terminal(&mut terminal)?;
            eprintln!("Initialization error: {err:?}");
        }
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

    // Step 3: Fetch available projects
    splash.set_message("Fetching projects");
    terminal.draw(|f| render_splash(f, &splash))?;

    let available_projects = match gcp::projects::list_project_ids(&client).await {
        Ok(projects) if !projects.is_empty() => {
            tracing::info!("Loaded {} projects", projects.len());
            projects
        }
        Ok(_) => {
            tracing::warn!("No projects returned, using current project only");
            vec![project.clone()]
        }
        Err(e) => {
            tracing::warn!("Failed to list projects: {}, using current project only", e);
            vec![project.clone()]
        }
    };

    splash.complete_step();

    if check_abort()? {
        return Ok(None);
    }

    // Step 4: Fetch available zones
    splash.set_message("Fetching zones");
    terminal.draw(|f| render_splash(f, &splash))?;

    let available_zones = match client.list_zones().await {
        Ok(zones) if !zones.is_empty() => {
            tracing::info!("Loaded {} zones", zones.len());
            // Add "all" as first option to show all zones
            let mut all_zones = vec!["all".to_string()];
            all_zones.extend(zones);
            all_zones
        }
        Ok(_) => {
            tracing::warn!("No zones returned, using static list");
            let mut all_zones = vec!["all".to_string()];
            all_zones.extend(auth::list_zones());
            all_zones
        }
        Err(e) => {
            tracing::warn!("Failed to list zones: {}, using static list", e);
            let mut all_zones = vec!["all".to_string()];
            all_zones.extend(auth::list_zones());
            all_zones
        }
    };

    splash.complete_step();

    if check_abort()? {
        return Ok(None);
    }

    // Step 5: Fetch initial data (VM instances)
    splash.set_message(&format!("Fetching instances from {}", zone));
    terminal.draw(|f| render_splash(f, &splash))?;

    let (instances, initial_error) = {
        match resource::fetch_resources("compute-instances", &client, &[]).await {
            Ok(items) => (items, None),
            Err(e) => {
                let error_msg = gcp::client::format_gcp_error(&e);
                (Vec::new(), Some(error_msg))
            }
        }
    };

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

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if event::handle_events(app).await? {
            return Ok(());
        }

        // Auto-refresh (disabled by default)
        if app.needs_refresh() {
            let _ = app.refresh_current().await;
        }
    }
}
