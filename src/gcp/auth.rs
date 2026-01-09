//! GCP Authentication
//!
//! Handles authentication using Application Default Credentials (ADC),
//! service account keys, or gcloud CLI credentials.

use anyhow::{Context, Result};
use gcp_auth::TokenProvider;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Default scopes for GCP API access
pub const DEFAULT_SCOPES: &[&str] = &["https://www.googleapis.com/auth/cloud-platform"];

/// GCP credentials holder with token caching
#[derive(Clone)]
pub struct GcpCredentials {
    provider: Arc<dyn TokenProvider>,
    token_cache: Arc<RwLock<Option<CachedToken>>>,
}

#[derive(Clone)]
struct CachedToken {
    token: String,
    // Token expiry could be tracked here if needed
}

impl GcpCredentials {
    /// Create new GCP credentials using Application Default Credentials
    pub async fn new() -> Result<Self> {
        let provider = gcp_auth::provider()
            .await
            .context("Failed to initialize GCP authentication. Run 'gcloud auth application-default login'")?;

        Ok(Self {
            provider: Arc::from(provider),
            token_cache: Arc::new(RwLock::new(None)),
        })
    }

    /// Get an access token for API calls
    pub async fn get_token(&self) -> Result<String> {
        // Check cache first
        {
            let cache = self.token_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                return Ok(cached.token.clone());
            }
        }

        // Fetch new token
        let token = self
            .provider
            .token(DEFAULT_SCOPES)
            .await
            .context("Failed to get access token")?;

        let token_str = token.as_str().to_string();

        // Cache it
        {
            let mut cache = self.token_cache.write().await;
            *cache = Some(CachedToken {
                token: token_str.clone(),
            });
        }

        Ok(token_str)
    }

    /// Force refresh the token
    pub async fn refresh_token(&self) -> Result<String> {
        // Clear cache
        {
            let mut cache = self.token_cache.write().await;
            *cache = None;
        }

        // Get fresh token
        self.get_token().await
    }
}

/// Get the gcloud configuration directory
pub fn get_gcloud_config_dir() -> Option<PathBuf> {
    // Check CLOUDSDK_CONFIG environment variable first
    if let Ok(path) = std::env::var("CLOUDSDK_CONFIG") {
        return Some(PathBuf::from(path));
    }

    // Default to ~/.config/gcloud on Linux/macOS
    dirs::config_dir().map(|p| p.join("gcloud"))
}

/// Read the default project from gcloud configuration
pub fn get_default_project() -> Option<String> {
    // Check environment variable first
    if let Ok(project) = std::env::var("CLOUDSDK_CORE_PROJECT") {
        return Some(project);
    }
    if let Ok(project) = std::env::var("GOOGLE_CLOUD_PROJECT") {
        return Some(project);
    }
    if let Ok(project) = std::env::var("GCLOUD_PROJECT") {
        return Some(project);
    }

    // Try to read from gcloud config
    let config_dir = get_gcloud_config_dir()?;
    let properties_path = config_dir.join("properties");

    if let Ok(content) = std::fs::read_to_string(&properties_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("project") {
                if let Some(value) = line.split('=').nth(1) {
                    return Some(value.trim().to_string());
                }
            }
        }
    }

    // Try active configuration
    let active_config_path = config_dir.join("active_config");
    if let Ok(active_config) = std::fs::read_to_string(&active_config_path) {
        let config_name = active_config.trim();
        let config_path = config_dir
            .join("configurations")
            .join(format!("config_{}", config_name));

        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let mut in_core_section = false;
            for line in content.lines() {
                let line = line.trim();
                if line == "[core]" {
                    in_core_section = true;
                } else if line.starts_with('[') {
                    in_core_section = false;
                } else if in_core_section && line.starts_with("project") {
                    if let Some(value) = line.split('=').nth(1) {
                        return Some(value.trim().to_string());
                    }
                }
            }
        }
    }

    None
}

/// Get the default zone from gcloud configuration
pub fn get_default_zone() -> Option<String> {
    // Check environment variable first
    if let Ok(zone) = std::env::var("CLOUDSDK_COMPUTE_ZONE") {
        return Some(zone);
    }

    // Try to read from gcloud config
    let config_dir = get_gcloud_config_dir()?;

    // Try active configuration
    let active_config_path = config_dir.join("active_config");
    if let Ok(active_config) = std::fs::read_to_string(&active_config_path) {
        let config_name = active_config.trim();
        let config_path = config_dir
            .join("configurations")
            .join(format!("config_{}", config_name));

        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let mut in_compute_section = false;
            for line in content.lines() {
                let line = line.trim();
                if line == "[compute]" {
                    in_compute_section = true;
                } else if line.starts_with('[') {
                    in_compute_section = false;
                } else if in_compute_section && line.starts_with("zone") {
                    if let Some(value) = line.split('=').nth(1) {
                        return Some(value.trim().to_string());
                    }
                }
            }
        }
    }

    None
}

/// Get the default region from gcloud configuration
pub fn get_default_region() -> Option<String> {
    // Check environment variable first
    if let Ok(region) = std::env::var("CLOUDSDK_COMPUTE_REGION") {
        return Some(region);
    }

    // Try to read from gcloud config
    let config_dir = get_gcloud_config_dir()?;

    // Try active configuration
    let active_config_path = config_dir.join("active_config");
    if let Ok(active_config) = std::fs::read_to_string(&active_config_path) {
        let config_name = active_config.trim();
        let config_path = config_dir
            .join("configurations")
            .join(format!("config_{}", config_name));

        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let mut in_compute_section = false;
            for line in content.lines() {
                let line = line.trim();
                if line == "[compute]" {
                    in_compute_section = true;
                } else if line.starts_with('[') {
                    in_compute_section = false;
                } else if in_compute_section && line.starts_with("region") {
                    if let Some(value) = line.split('=').nth(1) {
                        return Some(value.trim().to_string());
                    }
                }
            }
        }
    }

    // Derive region from zone if available
    get_default_zone().map(|zone| {
        // Zone format: us-central1-a -> Region: us-central1
        let parts: Vec<&str> = zone.rsplitn(2, '-').collect();
        if parts.len() == 2 {
            parts[1].to_string()
        } else {
            zone
        }
    })
}

/// List all available zones
pub fn list_zones() -> Vec<String> {
    // Common GCP zones - in practice, this would be fetched from the API
    vec![
        // US
        "us-central1-a".to_string(),
        "us-central1-b".to_string(),
        "us-central1-c".to_string(),
        "us-central1-f".to_string(),
        "us-east1-b".to_string(),
        "us-east1-c".to_string(),
        "us-east1-d".to_string(),
        "us-east4-a".to_string(),
        "us-east4-b".to_string(),
        "us-east4-c".to_string(),
        "us-west1-a".to_string(),
        "us-west1-b".to_string(),
        "us-west1-c".to_string(),
        "us-west2-a".to_string(),
        "us-west2-b".to_string(),
        "us-west2-c".to_string(),
        "us-west3-a".to_string(),
        "us-west3-b".to_string(),
        "us-west3-c".to_string(),
        "us-west4-a".to_string(),
        "us-west4-b".to_string(),
        "us-west4-c".to_string(),
        // Europe
        "europe-west1-b".to_string(),
        "europe-west1-c".to_string(),
        "europe-west1-d".to_string(),
        "europe-west2-a".to_string(),
        "europe-west2-b".to_string(),
        "europe-west2-c".to_string(),
        "europe-west3-a".to_string(),
        "europe-west3-b".to_string(),
        "europe-west3-c".to_string(),
        "europe-west4-a".to_string(),
        "europe-west4-b".to_string(),
        "europe-west4-c".to_string(),
        "europe-north1-a".to_string(),
        "europe-north1-b".to_string(),
        "europe-north1-c".to_string(),
        // Asia
        "asia-east1-a".to_string(),
        "asia-east1-b".to_string(),
        "asia-east1-c".to_string(),
        "asia-east2-a".to_string(),
        "asia-east2-b".to_string(),
        "asia-east2-c".to_string(),
        "asia-northeast1-a".to_string(),
        "asia-northeast1-b".to_string(),
        "asia-northeast1-c".to_string(),
        "asia-southeast1-a".to_string(),
        "asia-southeast1-b".to_string(),
        "asia-southeast1-c".to_string(),
        // Australia
        "australia-southeast1-a".to_string(),
        "australia-southeast1-b".to_string(),
        "australia-southeast1-c".to_string(),
        // South America
        "southamerica-east1-a".to_string(),
        "southamerica-east1-b".to_string(),
        "southamerica-east1-c".to_string(),
    ]
}

/// List all available regions (derived from zones)
pub fn list_regions() -> Vec<String> {
    let mut regions: Vec<String> = list_zones()
        .iter()
        .map(|zone| {
            let parts: Vec<&str> = zone.rsplitn(2, '-').collect();
            if parts.len() == 2 {
                parts[1].to_string()
            } else {
                zone.clone()
            }
        })
        .collect();

    regions.sort();
    regions.dedup();
    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_zones() {
        let zones = list_zones();
        assert!(!zones.is_empty());
        assert!(zones.contains(&"us-central1-a".to_string()));
    }

    #[test]
    fn test_list_regions() {
        let regions = list_regions();
        assert!(!regions.is_empty());
        assert!(regions.contains(&"us-central1".to_string()));
    }
}
