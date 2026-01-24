//! Configuration Management
//!
//! Handles persistent configuration storage for tgcp.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// User configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Last used project ID
    #[serde(default)]
    pub project_id: Option<String>,
    /// Last used zone
    #[serde(default)]
    pub zone: Option<String>,
    /// Last viewed resource
    #[serde(default)]
    pub last_resource: Option<String>,
    /// Theme name
    #[serde(default)]
    pub theme: Option<String>,
    /// Project-specific themes (project_id -> theme_name)
    #[serde(default)]
    pub project_themes: HashMap<String, String>,
    /// Custom aliases (alias -> resource_key)
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    /// SSH options
    #[serde(default)]
    pub ssh: SshConfig,
    /// Notification options
    #[serde(default)]
    pub notifications: NotificationConfig,
    /// Hidden columns per resource type (resource_key -> set of column headers)
    #[serde(default)]
    pub hidden_columns: HashMap<String, HashSet<String>>,
}

/// SSH configuration options
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SshConfig {
    /// Always use IAP tunneling
    #[serde(default)]
    pub use_iap: bool,
    /// Extra arguments to pass to gcloud compute ssh
    #[serde(default)]
    pub extra_args: Vec<String>,
}

/// Notification configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Enable notifications
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Detail level: "minimal", "detailed", "verbose"
    #[serde(default = "default_detail_level")]
    pub detail_level: String,
    /// Toast duration in seconds
    #[serde(default = "default_toast_duration")]
    pub toast_duration_secs: u64,
    /// Maximum notifications to keep in history
    #[serde(default = "default_max_history")]
    pub max_history: usize,
    /// Polling interval in milliseconds for pending operations
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    /// Automatically poll pending operations
    #[serde(default = "default_true")]
    pub auto_poll: bool,
    /// Sound configuration: "off", "errors_only", "all"
    #[serde(default = "default_sound")]
    pub sound: String,
}

fn default_true() -> bool {
    true
}

fn default_detail_level() -> String {
    "detailed".to_string()
}

fn default_toast_duration() -> u64 {
    5
}

fn default_max_history() -> usize {
    50
}

fn default_poll_interval() -> u64 {
    2000
}

fn default_sound() -> String {
    "off".to_string()
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            detail_level: "detailed".to_string(),
            toast_duration_secs: 5,
            max_history: 50,
            poll_interval_ms: 2000,
            auto_poll: true,
            sound: "off".to_string(),
        }
    }
}

impl Config {
    /// Get the config file path
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("tgcp").join("config.json"))
    }

    /// Load configuration from disk
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save configuration to disk
    /// Security: Sets restrictive file permissions (0600 on Unix)
    pub fn save(&self) -> Result<()> {
        let Some(path) = Self::config_path() else {
            return Ok(());
        };

        // Create parent directory with restricted permissions
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;

            // Security: Set directory permissions to 0700 (owner only)
            #[cfg(unix)]
            {
                let dir_perms = std::fs::Permissions::from_mode(0o700);
                let _ = std::fs::set_permissions(parent, dir_perms);
            }
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, &content)?;

        // Security: Set file permissions to 0600 (owner read/write only)
        #[cfg(unix)]
        {
            let file_perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&path, file_perms)?;
        }

        Ok(())
    }

    /// Get effective project (CLI > config > gcloud default)
    pub fn effective_project(&self) -> String {
        self.project_id
            .clone()
            .or_else(crate::gcp::auth::get_default_project)
            .unwrap_or_default()
    }

    /// Get effective zone (CLI > config > gcloud default)
    pub fn effective_zone(&self) -> String {
        self.zone
            .clone()
            .or_else(crate::gcp::auth::get_default_zone)
            .unwrap_or_else(|| "us-central1-a".to_string())
    }

    /// Set project and save
    pub fn set_project(&mut self, project_id: &str) -> Result<()> {
        self.project_id = Some(project_id.to_string());
        self.save()
    }

    /// Set zone and save
    pub fn set_zone(&mut self, zone: &str) -> Result<()> {
        self.zone = Some(zone.to_string());
        self.save()
    }

    /// Set theme and save
    pub fn set_theme(&mut self, theme: &str) -> Result<()> {
        self.theme = Some(theme.to_string());
        self.save()
    }

    /// Get theme for current project (or default)
    pub fn effective_theme(&self, project_id: &str) -> String {
        // Check project-specific theme first
        if let Some(theme) = self.project_themes.get(project_id) {
            return theme.clone();
        }
        // Fall back to default theme
        self.theme.clone().unwrap_or_else(|| "default".to_string())
    }

    /// Add alias
    pub fn add_alias(&mut self, alias: &str, resource_key: &str) -> Result<()> {
        self.aliases
            .insert(alias.to_string(), resource_key.to_string());
        self.save()
    }

    /// Resolve alias to resource key
    pub fn resolve_alias(&self, alias: &str) -> Option<&String> {
        self.aliases.get(alias)
    }

    /// Get hidden columns for a resource type
    pub fn get_hidden_columns(&self, resource_key: &str) -> HashSet<String> {
        self.hidden_columns
            .get(resource_key)
            .cloned()
            .unwrap_or_default()
    }

    /// Set hidden columns for a resource type and save
    pub fn set_hidden_columns(
        &mut self,
        resource_key: &str,
        hidden: HashSet<String>,
    ) -> Result<()> {
        if hidden.is_empty() {
            self.hidden_columns.remove(resource_key);
        } else {
            self.hidden_columns.insert(resource_key.to_string(), hidden);
        }
        self.save()
    }
}
