//! GCP Authentication
//!
//! Handles authentication using Application Default Credentials (ADC),
//! service account keys, or gcloud CLI credentials.

use anyhow::{Context, Result};
use gcp_auth::TokenProvider;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Default scopes for GCP API access
pub const DEFAULT_SCOPES: &[&str] = &["https://www.googleapis.com/auth/cloud-platform"];

/// Token expiry buffer - refresh tokens this much before they actually expire
/// This prevents using tokens that are about to expire during a request
const TOKEN_EXPIRY_BUFFER: Duration = Duration::from_secs(60);

/// Default token TTL if we can't determine expiry (conservative: 30 minutes)
const DEFAULT_TOKEN_TTL: Duration = Duration::from_secs(30 * 60);

/// GCP credentials holder with token caching
#[derive(Clone)]
pub struct GcpCredentials {
    provider: Arc<dyn TokenProvider>,
    token_cache: Arc<RwLock<Option<CachedToken>>>,
}

#[derive(Clone)]
struct CachedToken {
    token: String,
    /// When this token expires (with buffer applied)
    expires_at: Instant,
}

impl CachedToken {
    /// Check if this cached token is still valid
    fn is_valid(&self) -> bool {
        Instant::now() < self.expires_at
    }
}

impl GcpCredentials {
    /// Create new GCP credentials using Application Default Credentials
    pub async fn new() -> Result<Self> {
        let provider = gcp_auth::provider().await.context(
            "Failed to initialize GCP authentication. Run 'gcloud auth application-default login'",
        )?;

        Ok(Self {
            provider,
            token_cache: Arc::new(RwLock::new(None)),
        })
    }

    /// Get an access token for API calls
    /// Security: Checks token expiry before returning cached token
    pub async fn get_token(&self) -> Result<String> {
        // Check cache first - but only return if token is still valid
        {
            let cache = self.token_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.is_valid() {
                    return Ok(cached.token.clone());
                }
                // Token expired or about to expire, will fetch new one
                tracing::debug!("Cached token expired, fetching new token");
            }
        }

        // Fetch new token
        let token = self
            .provider
            .token(DEFAULT_SCOPES)
            .await
            .context("Failed to get access token")?;

        let token_str = token.as_str().to_string();

        // Calculate expiry time with buffer
        // gcp_auth Token has expires_at() but it returns Option<DateTime>
        // We'll use a conservative default TTL
        let expires_at = Instant::now() + DEFAULT_TOKEN_TTL - TOKEN_EXPIRY_BUFFER;

        // Cache it with expiry
        {
            let mut cache = self.token_cache.write().await;
            *cache = Some(CachedToken {
                token: token_str.clone(),
                expires_at,
            });
        }

        tracing::debug!("New token cached, expires in ~{} minutes",
            (DEFAULT_TOKEN_TTL - TOKEN_EXPIRY_BUFFER).as_secs() / 60);

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

/// Validate a GCP project ID format
/// Project IDs must be 6-30 characters, lowercase letters, digits, and hyphens
/// Must start with a letter and cannot end with a hyphen
fn validate_project_id(project: &str) -> bool {
    if project.len() < 6 || project.len() > 30 {
        return false;
    }

    let mut chars = project.chars();

    // Must start with a letter
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {},
        _ => return false,
    }

    // Must not end with a hyphen
    if project.ends_with('-') {
        return false;
    }

    // All chars must be lowercase, digit, or hyphen
    project.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Read the default project from gcloud configuration
/// Security: Validates project ID format before returning
pub fn get_default_project() -> Option<String> {
    // Check environment variable first
    if let Ok(project) = std::env::var("CLOUDSDK_CORE_PROJECT") {
        if validate_project_id(&project) {
            return Some(project);
        }
        tracing::warn!("Invalid project ID format in CLOUDSDK_CORE_PROJECT");
    }
    if let Ok(project) = std::env::var("GOOGLE_CLOUD_PROJECT") {
        if validate_project_id(&project) {
            return Some(project);
        }
        tracing::warn!("Invalid project ID format in GOOGLE_CLOUD_PROJECT");
    }
    if let Ok(project) = std::env::var("GCLOUD_PROJECT") {
        if validate_project_id(&project) {
            return Some(project);
        }
        tracing::warn!("Invalid project ID format in GCLOUD_PROJECT");
    }

    // Try to read from gcloud config
    let config_dir = get_gcloud_config_dir()?;
    let properties_path = config_dir.join("properties");

    if let Ok(content) = std::fs::read_to_string(&properties_path) {
        for line in content.lines() {
            let line = line.trim();
            // Security: Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with("project") && line.contains('=') {
                if let Some(value) = line.split('=').nth(1) {
                    let project = value.trim().to_string();
                    if validate_project_id(&project) {
                        return Some(project);
                    }
                }
            }
        }
    }

    // Try active configuration
    let active_config_path = config_dir.join("active_config");
    if let Ok(active_config) = std::fs::read_to_string(&active_config_path) {
        let config_name = active_config.trim();

        // Security: Validate config name to prevent path traversal
        if !config_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            tracing::warn!("Invalid characters in active_config name");
            return None;
        }

        let config_path = config_dir
            .join("configurations")
            .join(format!("config_{}", config_name));

        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let mut in_core_section = false;
            for line in content.lines() {
                let line = line.trim();
                // Security: Skip comments
                if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                    continue;
                }
                if line == "[core]" {
                    in_core_section = true;
                } else if line.starts_with('[') {
                    in_core_section = false;
                } else if in_core_section && line.starts_with("project") && line.contains('=') {
                    if let Some(value) = line.split('=').nth(1) {
                        let project = value.trim().to_string();
                        if validate_project_id(&project) {
                            return Some(project);
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_zones() {
        let zones = list_zones();
        assert!(!zones.is_empty());
        assert!(zones.contains(&"us-central1-a".to_string()));
    }
}
