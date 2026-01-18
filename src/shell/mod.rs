//! Shell Integration
//!
//! Handles SSH connections and shell execution for tgcp.
//!
//! # Features
//!
//! - SSH to VM instances using `gcloud compute ssh`
//! - IAP tunnel support for instances without external IPs
//! - Serial console access for debugging
//! - Browser launch for GCP Console
//!
//! # Security
//!
//! All SSH arguments are validated against a whitelist to prevent
//! command injection attacks. See [`validate_ssh_extra_args`] for details.

use anyhow::{anyhow, Context, Result};
use std::process::{Command, Stdio};

/// Whitelist of allowed SSH argument prefixes for security
/// These are safe gcloud compute ssh arguments that don't allow arbitrary command execution
const ALLOWED_SSH_ARG_PREFIXES: &[&str] = &[
    "-o",             // SSH options (e.g., -o StrictHostKeyChecking=no)
    "-i",             // Identity file
    "-L",             // Local port forwarding
    "-R",             // Remote port forwarding
    "-D",             // Dynamic port forwarding (SOCKS proxy)
    "-p",             // Port
    "-q",             // Quiet mode
    "-v",             // Verbose mode
    "-4",             // IPv4 only
    "-6",             // IPv6 only
    "--ssh-flag",     // gcloud ssh flag passthrough
    "--ssh-key-file", // SSH key file
    "--internal-ip",  // Use internal IP
    "--dry-run",      // Dry run mode
];

/// Validate that SSH extra_args only contain safe arguments
/// Returns Ok(()) if all args are safe, Err with details if unsafe arg found
pub fn validate_ssh_extra_args(args: &[String]) -> Result<()> {
    for arg in args {
        let arg_lower = arg.to_lowercase();

        // Check if arg starts with an allowed prefix
        let is_allowed = ALLOWED_SSH_ARG_PREFIXES
            .iter()
            .any(|&prefix| arg_lower.starts_with(prefix));

        if !is_allowed {
            return Err(anyhow!(
                "SSH argument '{}' is not in the allowed list. \
                Allowed prefixes: {:?}",
                arg,
                ALLOWED_SSH_ARG_PREFIXES
            ));
        }

        // Additional check: block potential command injection via -o
        if arg_lower.starts_with("-o") {
            let option_value = if arg.len() > 2 {
                &arg[2..]
            } else {
                continue; // -o by itself, value is next arg
            };

            // Block dangerous SSH options
            let dangerous_options = ["proxycommand", "localcommand", "permitlocalcommand"];
            for dangerous in dangerous_options {
                if option_value.to_lowercase().contains(dangerous) {
                    return Err(anyhow!(
                        "SSH option '{}' contains potentially dangerous option '{}'. \
                        This option is not allowed for security reasons.",
                        arg,
                        dangerous
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Validate GCP resource name (instance, zone, project)
/// GCP resource names follow specific patterns: lowercase alphanumeric with hyphens
pub fn validate_gcp_resource_name(name: &str, resource_type: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("{} name cannot be empty", resource_type));
    }

    if name.len() > 63 {
        return Err(anyhow!(
            "{} name '{}' exceeds maximum length of 63 characters",
            resource_type,
            name
        ));
    }

    // GCP resource names: lowercase letters, numbers, hyphens
    // Must start with a letter and end with a letter or number
    let valid_chars = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');

    if !valid_chars {
        return Err(anyhow!(
            "{} name '{}' contains invalid characters. \
            Only lowercase letters, numbers, and hyphens are allowed.",
            resource_type,
            name
        ));
    }

    // Must start with a letter
    if let Some(first) = name.chars().next() {
        if !first.is_ascii_lowercase() {
            return Err(anyhow!(
                "{} name '{}' must start with a lowercase letter",
                resource_type,
                name
            ));
        }
    }

    // Must not end with a hyphen
    if name.ends_with('-') {
        return Err(anyhow!(
            "{} name '{}' must not end with a hyphen",
            resource_type,
            name
        ));
    }

    Ok(())
}

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
    /// Error launching command
    Error(String),
}

/// Execute SSH to a GCE instance
///
/// This function suspends the TUI, runs SSH, and returns when done.
/// Security: Validates all inputs before executing the command.
pub fn ssh_to_instance(opts: &SshOptions) -> ShellResult {
    // Security: Validate resource names to prevent injection
    if let Err(e) = validate_gcp_resource_name(&opts.instance, "Instance") {
        return ShellResult::Error(format!("Invalid instance name: {}", e));
    }

    // Zone validation is more lenient (contains region prefix)
    if opts.zone.is_empty() || opts.zone.len() > 63 {
        return ShellResult::Error("Invalid zone name".to_string());
    }

    // Project validation
    if opts.project.is_empty() || opts.project.len() > 63 {
        return ShellResult::Error("Invalid project name".to_string());
    }

    // Security: Validate extra_args against whitelist
    if let Err(e) = validate_ssh_extra_args(&opts.extra_args) {
        return ShellResult::Error(format!("Security validation failed: {}", e));
    }

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

    // Security: Log command without potentially sensitive extra_args
    tracing::info!(
        "Executing SSH: instance={}, zone={}, project={}, iap={}",
        opts.instance,
        opts.zone,
        opts.project,
        opts.use_iap
    );

    execute_command("gcloud", &args)
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
/// Security: All dynamic values are URL-encoded to prevent injection
pub fn console_url(resource_type: &str, resource_name: &str, project: &str, zone: &str) -> String {
    // Security: URL-encode all dynamic values to prevent URL manipulation
    let encoded_name = urlencoding::encode(resource_name);
    let encoded_project = urlencoding::encode(project);
    let encoded_zone = urlencoding::encode(zone);

    match resource_type {
        "compute-instances" => {
            format!(
                "https://console.cloud.google.com/compute/instancesDetail/zones/{}/instances/{}?project={}",
                encoded_zone, encoded_name, encoded_project
            )
        },
        "compute-disks" => {
            format!(
                "https://console.cloud.google.com/compute/disksDetail/zones/{}/disks/{}?project={}",
                encoded_zone, encoded_name, encoded_project
            )
        },
        "storage-buckets" => {
            format!(
                "https://console.cloud.google.com/storage/browser/{}?project={}",
                encoded_name, encoded_project
            )
        },
        "gke-clusters" => {
            format!(
                "https://console.cloud.google.com/kubernetes/clusters/details/{}/{}?project={}",
                encoded_zone, encoded_name, encoded_project
            )
        },
        _ => {
            format!(
                "https://console.cloud.google.com/home/dashboard?project={}",
                encoded_project
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
