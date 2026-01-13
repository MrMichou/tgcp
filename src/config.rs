//! Configuration Management
//!
//! Handles persistent configuration storage for tgcp.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
}
