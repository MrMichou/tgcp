//! Shell Integration
//!
//! Handles SSH connections and shell execution for tgcp.

use anyhow::{Context, Result};
use std::process::{Command, Stdio};

/// SSH connection options
#[derive(Debug, Clone)]
pub struct SshOptions {
    /// Instance name
    pub instance: String,
    /// Zone
    pub zone: String,
    /// Project ID
    pub project: String,
    /// Use IAP tunneling
    pub use_iap: bool,
    /// Additional SSH arguments
    pub extra_args: Vec<String>,
}

impl SshOptions {
    pub fn new(instance: &str, zone: &str, project: &str) -> Self {
        Self {
            instance: instance.to_string(),
            zone: zone.to_string(),
            project: project.to_string(),
            use_iap: false,
            extra_args: Vec::new(),
        }
    }

    pub fn with_iap(mut self) -> Self {
        self.use_iap = true;
        self
    }
}

/// Result of a shell operation
#[derive(Debug)]
pub enum ShellResult {
    /// Command completed successfully
    Success,
    /// Command failed with exit code
    Failed(i32),
    /// Command was interrupted
    Interrupted,
    /// Error launching command
    Error(String),
}

/// Execute SSH to a GCE instance
///
/// This function suspends the TUI, runs SSH, and returns when done.
pub fn ssh_to_instance(opts: &SshOptions) -> ShellResult {
    let mut args = vec![
        "compute".to_string(),
        "ssh".to_string(),
        opts.instance.clone(),
        "--zone".to_string(),
        opts.zone.clone(),
        "--project".to_string(),
        opts.project.clone(),
    ];

    if opts.use_iap {
        args.push("--tunnel-through-iap".to_string());
    }

    args.extend(opts.extra_args.clone());

    tracing::info!("Executing: gcloud {}", args.join(" "));

    execute_command("gcloud", &args)
}

/// Execute serial console connection
pub fn serial_console(instance: &str, zone: &str, project: &str, port: u8) -> ShellResult {
    let args = vec![
        "compute".to_string(),
        "connect-to-serial-port".to_string(),
        instance.to_string(),
        "--zone".to_string(),
        zone.to_string(),
        "--project".to_string(),
        project.to_string(),
        "--port".to_string(),
        port.to_string(),
    ];

    tracing::info!("Executing: gcloud {}", args.join(" "));

    execute_command("gcloud", &args)
}

/// Execute kubectl exec into a pod
pub fn kubectl_exec(
    pod: &str,
    namespace: &str,
    container: Option<&str>,
    command: &[&str],
) -> ShellResult {
    let mut args = vec![
        "exec".to_string(),
        "-it".to_string(),
        pod.to_string(),
        "-n".to_string(),
        namespace.to_string(),
    ];

    if let Some(c) = container {
        args.push("-c".to_string());
        args.push(c.to_string());
    }

    args.push("--".to_string());

    if command.is_empty() {
        args.push("/bin/sh".to_string());
    } else {
        args.extend(command.iter().map(|s| s.to_string()));
    }

    tracing::info!("Executing: kubectl {}", args.join(" "));

    execute_command("kubectl", &args)
}

/// Open URL in browser (for console links)
pub fn open_browser(url: &str) -> ShellResult {
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", url])
    } else {
        // Linux - try xdg-open first
        ("xdg-open", vec![url])
    };

    execute_command(cmd, &args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

/// Execute a command, inheriting stdio
fn execute_command(cmd: &str, args: &[String]) -> ShellResult {
    match Command::new(cmd)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(mut child) => match child.wait() {
            Ok(status) => {
                if status.success() {
                    ShellResult::Success
                } else {
                    ShellResult::Failed(status.code().unwrap_or(-1))
                }
            },
            Err(e) => ShellResult::Error(format!("Failed to wait for process: {}", e)),
        },
        Err(e) => ShellResult::Error(format!("Failed to execute {}: {}", cmd, e)),
    }
}

/// Build GCP Console URL for a resource
pub fn console_url(resource_type: &str, resource_name: &str, project: &str, zone: &str) -> String {
    match resource_type {
        "compute-instances" => {
            format!(
                "https://console.cloud.google.com/compute/instancesDetail/zones/{}/instances/{}?project={}",
                zone, resource_name, project
            )
        },
        "compute-disks" => {
            format!(
                "https://console.cloud.google.com/compute/disksDetail/zones/{}/disks/{}?project={}",
                zone, resource_name, project
            )
        },
        "storage-buckets" => {
            format!(
                "https://console.cloud.google.com/storage/browser/{}?project={}",
                resource_name, project
            )
        },
        "gke-clusters" => {
            format!(
                "https://console.cloud.google.com/kubernetes/clusters/details/{}/{}?project={}",
                zone, resource_name, project
            )
        },
        _ => {
            format!(
                "https://console.cloud.google.com/home/dashboard?project={}",
                project
            )
        },
    }
}

/// Terminal preparation for shell execution
pub struct TerminalGuard {
    _private: (),
}

impl TerminalGuard {
    /// Prepare terminal for external command
    /// This should be called before spawning a shell command
    pub fn prepare() -> Result<Self> {
        // Disable raw mode to let the subprocess handle input normally
        crossterm::terminal::disable_raw_mode().context("Failed to disable raw mode")?;

        // Leave alternate screen so user can see command output
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )
        .context("Failed to leave alternate screen")?;

        Ok(Self { _private: () })
    }

    /// Restore terminal after command completes
    pub fn restore(self) -> Result<()> {
        // Re-enter alternate screen
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )
        .context("Failed to enter alternate screen")?;

        // Re-enable raw mode
        crossterm::terminal::enable_raw_mode().context("Failed to enable raw mode")?;

        Ok(())
    }
}

/// Execute a shell command with terminal handling
pub fn execute_with_terminal_handling<F>(f: F) -> Result<ShellResult>
where
    F: FnOnce() -> ShellResult,
{
    let guard = TerminalGuard::prepare()?;

    // Clear the screen before running command
    print!("\x1B[2J\x1B[H");
    std::io::Write::flush(&mut std::io::stdout())?;

    let result = f();

    // Wait for user to press Enter before returning
    if matches!(result, ShellResult::Success | ShellResult::Failed(_)) {
        println!("\nPress Enter to return to tgcp...");
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
    }

    guard.restore()?;

    Ok(result)
}
