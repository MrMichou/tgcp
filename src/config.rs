//! Configuration Management
//!
//! Handles persistent configuration storage for tgcp.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub fn save(&self) -> Result<()> {
        let Some(path) = Self::config_path() else {
            return Ok(());
        };

        // Create parent directory
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;

        Ok(())
    }

    /// Get effective project (CLI > config > gcloud default)
    pub fn effective_project(&self) -> String {
        self.project_id
            .clone()
            .or_else(crate::gcp::auth::get_default_project)
            .unwrap_or_else(|| "".to_string())
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
}
