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
//! `gcloud` is spawned directly (never through a shell), so `extra_args` are
//! not subject to shell interpretation. They are additionally validated by
//! [`validate_ssh_extra_args`]: only a fixed set of gcloud and ssh flags is
//! accepted, `-o` options must be in an allow-list of option names, and values
//! passed through `--ssh-flag` are validated by the same rules as direct ssh
//! arguments. Flags known to execute local commands or load arbitrary code
//! (e.g. `ProxyCommand`, `LocalCommand`, `-F`, `-I`) are rejected. This
//! reduces, but does not eliminate, the risk from a tampered config file: the
//! config file must still be treated as trusted input.

use anyhow::{anyhow, Context, Result};
use std::process::{Command, Stdio};

/// gcloud-level flags accepted in `extra_args` that take no value
const ALLOWED_GCLOUD_SWITCHES: &[&str] = &["--internal-ip", "--dry-run"];

/// ssh option names (for `-o Name=value`) that are considered safe.
/// Compared case-insensitively, as ssh does.
const ALLOWED_SSH_OPTIONS: &[&str] = &[
    "StrictHostKeyChecking",
    "UserKnownHostsFile",
    "ServerAliveInterval",
    "ServerAliveCountMax",
    "ConnectTimeout",
    "LogLevel",
    "ForwardAgent",
    "IdentitiesOnly",
    "Compression",
    "TCPKeepAlive",
    "AddKeysToAgent",
];

/// Single-letter ssh flags that take a value (attached or as the next argument).
/// Case-sensitive: `-i` (identity file) is not `-I` (PKCS#11 provider).
/// `-o` is handled separately so its value can be checked.
const SSH_FLAGS_WITH_VALUE: &[char] = &[
    'i', // Identity file
    'l', // Login name
    'L', // Local port forwarding
    'R', // Remote port forwarding
    'D', // Dynamic port forwarding (SOCKS proxy)
    'p', // Port
];

/// Single-letter ssh flags that take no value
const SSH_SWITCHES: &[char] = &['q', 'v', '4', '6'];

/// Single-letter ssh flags that are explicitly forbidden, with the reason
const FORBIDDEN_SSH_FLAGS: &[(char, &str)] = &[
    ('F', "loads an arbitrary ssh config file"),
    ('I', "loads an arbitrary PKCS#11 shared library"),
];

/// Validate that SSH extra_args only contain safe arguments
/// Returns Ok(()) if all args are safe, Err with details if unsafe arg found
pub fn validate_ssh_extra_args(args: &[String]) -> Result<()> {
    // Check every argument, including flag values, for control characters
    if let Some(arg) = args.iter().find(|a| a.chars().any(char::is_control)) {
        return Err(anyhow!(
            "SSH argument '{}' contains control characters",
            arg.escape_debug()
        ));
    }
    validate_args(args, true)
}

/// Validate a list of arguments.
///
/// When `allow_gcloud_flags` is false (i.e. when validating the content of an
/// `--ssh-flag` value), only raw ssh flags are accepted.
fn validate_args(args: &[String], allow_gcloud_flags: bool) -> Result<()> {
    let mut iter = args.iter();

    while let Some(arg) = iter.next() {
        if arg.starts_with("--") {
            if !allow_gcloud_flags {
                return Err(anyhow!(
                    "SSH argument '{}' is not allowed inside --ssh-flag",
                    arg
                ));
            }
            validate_gcloud_flag(arg, &mut iter)?;
            continue;
        }

        let mut chars = arg.chars();
        let flag = match (chars.next(), chars.next()) {
            (Some('-'), Some(flag)) => flag,
            _ => return Err(anyhow!("SSH argument '{}' is not in the allowed list", arg)),
        };
        let attached = chars.as_str();

        if let Some((_, reason)) = FORBIDDEN_SSH_FLAGS.iter().find(|(f, _)| *f == flag) {
            return Err(anyhow!(
                "SSH argument '-{}' is not allowed for security reasons ({})",
                flag,
                reason
            ));
        }

        if flag == 'o' {
            let value = take_value(arg, attached, &mut iter)?;
            validate_ssh_option(&value)?;
        } else if SSH_FLAGS_WITH_VALUE.contains(&flag) {
            take_value(arg, attached, &mut iter)?;
        } else if SSH_SWITCHES.contains(&flag) {
            // Allow repeated verbosity (-vvv), otherwise no attached value
            let ok = attached.is_empty() || (flag == 'v' && attached.chars().all(|c| c == 'v'));
            if !ok {
                return Err(anyhow!("SSH argument '{}' is not in the allowed list", arg));
            }
        } else {
            return Err(anyhow!("SSH argument '{}' is not in the allowed list", arg));
        }
    }
    Ok(())
}

/// Validate a `--long` gcloud flag, consuming its value from `iter` if needed
fn validate_gcloud_flag<'a, I>(arg: &str, iter: &mut I) -> Result<()>
where
    I: Iterator<Item = &'a String>,
{
    let (name, inline_value) = match arg.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (arg, None),
    };

    match name {
        "--ssh-flag" => {
            let value = inline_or_next(arg, inline_value, iter)?;
            // gcloud splits each --ssh-flag value on whitespace and passes the
            // pieces to ssh, so validate them as raw ssh arguments.
            let tokens: Vec<String> = value.split_whitespace().map(str::to_string).collect();
            if tokens.is_empty() {
                return Err(anyhow!("SSH argument '{}' has an empty value", arg));
            }
            validate_args(&tokens, false)
        },
        "--ssh-key-file" => inline_or_next(arg, inline_value, iter).map(|_| ()),
        _ if ALLOWED_GCLOUD_SWITCHES.contains(&name) && inline_value.is_none() => Ok(()),
        _ => Err(anyhow!("SSH argument '{}' is not in the allowed list", arg)),
    }
}

/// Get a flag value from `--flag=value` or from the following argument
fn inline_or_next<'a, I>(arg: &str, inline_value: Option<&str>, iter: &mut I) -> Result<String>
where
    I: Iterator<Item = &'a String>,
{
    match inline_value {
        Some(v) if !v.is_empty() => Ok(v.to_string()),
        Some(_) => Err(anyhow!("SSH argument '{}' has an empty value", arg)),
        None => iter
            .next()
            .cloned()
            .ok_or_else(|| anyhow!("SSH argument '{}' is missing its value", arg)),
    }
}

/// Get a short flag value, either attached (`-p22`) or as the next argument (`-p 22`)
fn take_value<'a, I>(arg: &str, attached: &str, iter: &mut I) -> Result<String>
where
    I: Iterator<Item = &'a String>,
{
    if attached.is_empty() {
        iter.next()
            .cloned()
            .ok_or_else(|| anyhow!("SSH argument '{}' is missing its value", arg))
    } else {
        Ok(attached.to_string())
    }
}

/// Validate the value of an ssh `-o` option against the allow-list.
/// ssh accepts both `Name=value` and `Name value` forms.
fn validate_ssh_option(option: &str) -> Result<()> {
    let name = option
        .trim_start()
        .split(|c: char| c == '=' || c.is_whitespace())
        .next()
        .unwrap_or_default();

    if ALLOWED_SSH_OPTIONS
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(name))
    {
        Ok(())
    } else {
        Err(anyhow!(
            "SSH option '{}' is not allowed. Allowed options: {}",
            name,
            ALLOWED_SSH_OPTIONS.join(", ")
        ))
    }
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
///
/// The opener is spawned detached with null stdio and is not waited on, so it
/// neither blocks the UI nor writes over the TUI.
pub fn open_browser(url: &str) -> ShellResult {
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        // Avoid `cmd /C start`, which re-parses the URL with cmd.exe metacharacters
        ("rundll32", vec!["url.dll,FileProtocolHandler", url])
    } else {
        // Linux - try xdg-open first
        ("xdg-open", vec![url])
    };

    match Command::new(cmd)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            // Reap the process in the background so it doesn't linger as a zombie
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            ShellResult::Success
        },
        Err(e) => ShellResult::Error(format!("Failed to execute {}: {}", cmd, e)),
    }
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

/// Terminal preparation for shell execution.
/// Implements `Drop` to ensure terminal is always restored, even if code panics.
pub struct TerminalGuard {
    restored: bool,
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

        Ok(Self { restored: false })
    }

    /// Restore terminal after command completes
    pub fn restore(mut self) -> Result<()> {
        self.restored = true;
        Self::do_restore()
    }

    fn do_restore() -> Result<()> {
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

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if !self.restored {
            let _ = Self::do_restore();
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Tests for validate_ssh_extra_args
    // =========================================================================

    #[test]
    fn test_ssh_args_valid_basic() {
        let args = vec![
            "-oStrictHostKeyChecking=no".to_string(),
            "-i".to_string(),
            "/path/to/key".to_string(),
        ];
        assert!(validate_ssh_extra_args(&args).is_ok());
    }

    #[test]
    fn test_ssh_args_valid_with_separate_o() {
        // -o followed by a separate argument
        let args = vec!["-o".to_string(), "StrictHostKeyChecking=no".to_string()];
        assert!(validate_ssh_extra_args(&args).is_ok());
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_ssh_args_blocked_proxycommand_attached() {
        // ProxyCommand attached to -o
        let args = vec!["-oProxyCommand=nc %h %p".to_string()];
        let result = validate_ssh_extra_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ProxyCommand"));
    }

    #[test]
    fn test_ssh_args_blocked_proxycommand_separate() {
        // ProxyCommand as a separate argument after -o
        let args = vec!["-o".to_string(), "ProxyCommand=nc %h %p".to_string()];
        let result = validate_ssh_extra_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ProxyCommand"));
    }

    #[test]
    fn test_ssh_args_blocked_proxycommand_space_form() {
        // ssh also accepts "Name value" without '='
        assert!(validate_ssh_extra_args(&args(&["-o", "ProxyCommand nc %h %p"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["-o", " ProxyCommand=x"])).is_err());
    }

    #[test]
    fn test_ssh_args_blocked_localcommand() {
        let args = vec!["-oLocalCommand=whoami".to_string()];
        let result = validate_ssh_extra_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("LocalCommand"));
    }

    #[test]
    fn test_ssh_args_blocked_options_not_in_allowlist() {
        // Options not on the deny-list before, but dangerous
        for opt in [
            "-oProxyJump=evil",
            "-oKnownHostsCommand=id",
            "-oPKCS11Provider=/tmp/evil.so",
            "-oInclude=/tmp/evil",
            "-oRemoteCommand=id",
            "-oMatch=exec id",
        ] {
            assert!(
                validate_ssh_extra_args(&args(&[opt])).is_err(),
                "{} should be rejected",
                opt
            );
        }
    }

    #[test]
    fn test_ssh_args_allowed_options() {
        for opt in [
            "-oStrictHostKeyChecking=no",
            "-oUserKnownHostsFile=/dev/null",
            "-oServerAliveInterval=30",
            "-oServerAliveCountMax=3",
            "-oConnectTimeout=10",
            "-oLogLevel=ERROR",
            "-oForwardAgent=yes",
            "-oIdentitiesOnly=yes",
            "-oCompression=yes",
            "-oTCPKeepAlive=yes",
            "-oAddKeysToAgent=yes",
            "-ostricthostkeychecking=no",
        ] {
            assert!(
                validate_ssh_extra_args(&args(&[opt])).is_ok(),
                "{} should be allowed",
                opt
            );
        }
        assert!(validate_ssh_extra_args(&args(&["-o", "ConnectTimeout 10"])).is_ok());
    }

    #[test]
    fn test_ssh_args_ssh_flag_value_is_validated() {
        // Value as next argument
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag", "-oProxyCommand=id"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag", "-o ProxyCommand=id"])).is_err());
        // Previously the value was skipped entirely
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag", "-F/tmp/cfg"])).is_err());
        // Valid
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag", "-vvv"])).is_ok());
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag", "-L 80:localhost:80"])).is_ok());
    }

    #[test]
    fn test_ssh_args_ssh_flag_equals_form_is_validated() {
        // `=` form previously bypassed the -o check entirely
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag=-oProxyCommand=id"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag=-o ProxyCommand=id"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag=-I/tmp/evil.so"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag=-oStrictHostKeyChecking=no"])).is_ok());
    }

    #[test]
    fn test_ssh_args_ssh_flag_rejects_nested_gcloud_flags_and_missing_values() {
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag=--ssh-flag=-v"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["--ssh-flag="])).is_err());
        // A dangling -o can't be completed by a later --ssh-flag
        assert!(
            validate_ssh_extra_args(&args(&["--ssh-flag=-o", "--ssh-flag=ProxyCommand=id"]))
                .is_err()
        );
    }

    #[test]
    fn test_ssh_args_blocked_pkcs11_and_config_file() {
        // -I must not be treated as -i
        assert!(validate_ssh_extra_args(&args(&["-I", "/tmp/evil.so"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["-I/tmp/evil.so"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["-F", "/tmp/cfg"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["-F/tmp/cfg"])).is_err());
        // -i is still allowed
        assert!(validate_ssh_extra_args(&args(&["-i", "/path/key"])).is_ok());
        assert!(validate_ssh_extra_args(&args(&["-i/path/key"])).is_ok());
    }

    #[test]
    fn test_ssh_args_value_flag_consumes_value() {
        // The value of -i is not interpreted as a flag
        assert!(validate_ssh_extra_args(&args(&["-i", "-oProxyCommand=id"])).is_ok());
        // But the next flag after a value is validated
        assert!(validate_ssh_extra_args(&args(&["-i", "key", "-oProxyCommand=id"])).is_err());
    }

    #[test]
    fn test_ssh_args_prefix_matching_removed() {
        // Previously any arg starting with an allowed prefix was accepted
        assert!(validate_ssh_extra_args(&args(&["--internal-ipX"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["-qX"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["-O", "exit"])).is_err());
        assert!(validate_ssh_extra_args(&args(&["--ssh-key-file=/k", "--dry-run"])).is_ok());
    }

    #[test]
    fn test_ssh_args_rejects_control_chars() {
        assert!(
            validate_ssh_extra_args(&args(&["-o", "LogLevel=ERROR\nProxyCommand=id"])).is_err()
        );
    }

    #[test]
    fn test_ssh_args_blocked_permitlocalcommand() {
        let args = vec!["-oPermitLocalCommand=yes".to_string()];
        let result = validate_ssh_extra_args(&args);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.to_lowercase().contains("permitlocalcommand"),
            "Expected error to contain 'permitlocalcommand', got: {}",
            err_msg
        );
    }

    #[test]
    fn test_ssh_args_blocked_unknown_arg() {
        // Unknown argument not in whitelist
        let args = vec!["--execute-command".to_string()];
        let result = validate_ssh_extra_args(&args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not in the allowed list"));
    }

    #[test]
    fn test_ssh_args_case_insensitive_blocking() {
        // Test case insensitivity
        let args = vec!["-oPROXYCOMMAND=bad".to_string()];
        assert!(validate_ssh_extra_args(&args).is_err());

        let args = vec!["-oProxyCommand=bad".to_string()];
        assert!(validate_ssh_extra_args(&args).is_err());
    }

    #[test]
    fn test_ssh_args_empty() {
        let args: Vec<String> = vec![];
        assert!(validate_ssh_extra_args(&args).is_ok());
    }

    #[test]
    fn test_ssh_args_multiple_valid() {
        let args = vec![
            "-v".to_string(),
            "-o".to_string(),
            "ConnectTimeout=10".to_string(),
            "-L".to_string(),
            "8080:localhost:80".to_string(),
            "--internal-ip".to_string(),
        ];
        let result = validate_ssh_extra_args(&args);
        assert!(
            result.is_ok(),
            "Expected Ok, got: {:?}",
            result.unwrap_err()
        );
    }

    #[test]
    fn test_ssh_args_o_at_end() {
        // -o at the end with no value is rejected: a dangling -o could
        // otherwise be completed by an argument gcloud appends after it
        let args = vec!["-o".to_string()];
        assert!(validate_ssh_extra_args(&args).is_err());
    }

    // =========================================================================
    // Tests for validate_gcp_resource_name
    // =========================================================================

    #[test]
    fn test_resource_name_valid() {
        assert!(validate_gcp_resource_name("my-instance", "Instance").is_ok());
        assert!(validate_gcp_resource_name("instance-1", "Instance").is_ok());
        assert!(validate_gcp_resource_name("a", "Instance").is_ok());
        assert!(validate_gcp_resource_name("a1b2c3", "Instance").is_ok());
    }

    #[test]
    fn test_resource_name_empty() {
        let result = validate_gcp_resource_name("", "Instance");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_resource_name_too_long() {
        let long_name = "a".repeat(64);
        let result = validate_gcp_resource_name(&long_name, "Instance");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("exceeds maximum length"));
    }

    #[test]
    fn test_resource_name_invalid_chars() {
        // Uppercase not allowed
        let result = validate_gcp_resource_name("MyInstance", "Instance");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid characters"));

        // Spaces not allowed
        let result = validate_gcp_resource_name("my instance", "Instance");
        assert!(result.is_err());

        // Underscores not allowed
        let result = validate_gcp_resource_name("my_instance", "Instance");
        assert!(result.is_err());

        // Special characters not allowed
        let result = validate_gcp_resource_name("my-instance!", "Instance");
        assert!(result.is_err());
    }

    #[test]
    fn test_resource_name_must_start_with_letter() {
        let result = validate_gcp_resource_name("1-instance", "Instance");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must start with a lowercase letter"));

        let result = validate_gcp_resource_name("-instance", "Instance");
        assert!(result.is_err());
    }

    #[test]
    fn test_resource_name_cannot_end_with_hyphen() {
        let result = validate_gcp_resource_name("my-instance-", "Instance");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must not end with a hyphen"));
    }

    #[test]
    fn test_resource_name_injection_attempts() {
        // Shell injection attempts should fail due to invalid characters
        assert!(validate_gcp_resource_name("instance; rm -rf /", "Instance").is_err());
        assert!(validate_gcp_resource_name("instance$(whoami)", "Instance").is_err());
        assert!(validate_gcp_resource_name("instance`id`", "Instance").is_err());
        assert!(validate_gcp_resource_name("instance|cat /etc/passwd", "Instance").is_err());
    }

    #[test]
    fn test_resource_name_boundary() {
        // Exactly 63 characters (max length)
        let max_name = "a".repeat(63);
        assert!(validate_gcp_resource_name(&max_name, "Instance").is_ok());
    }
}
