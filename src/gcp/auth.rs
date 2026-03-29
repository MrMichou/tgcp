//! GCP Authentication
//!
//! Handles authentication using Application Default Credentials (ADC),
//! service account keys, or gcloud CLI credentials.

use anyhow::{Context, Result};
use gcp_auth::TokenProvider;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

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
    token_cache: Arc<Mutex<Option<CachedToken>>>,
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
            token_cache: Arc::new(Mutex::new(None)),
        })
    }

    /// Get an access token for API calls
    /// Security: Checks token expiry before returning cached token.
    /// Uses a single Mutex lock scope to prevent concurrent callers from all
    /// fetching new tokens simultaneously (TOCTOU race).
    pub async fn get_token(&self) -> Result<String> {
        let mut cache = self.token_cache.lock().await;

        // Return cached token if still valid
        if let Some(cached) = cache.as_ref() {
            if cached.is_valid() {
                return Ok(cached.token.clone());
            }
            tracing::debug!("Cached token expired, fetching new token");
        }

        // Fetch new token while still holding the lock
        let token = self
            .provider
            .token(DEFAULT_SCOPES)
            .await
            .context("Failed to get access token")?;

        let token_str = token.as_str().to_string();

        let expires_at = {
            let token_expiry = token.expires_at();
            let now_utc = chrono::Utc::now();
            let remaining = (token_expiry - now_utc)
                .to_std()
                .unwrap_or(DEFAULT_TOKEN_TTL);
            Instant::now() + remaining - TOKEN_EXPIRY_BUFFER
        };

        *cache = Some(CachedToken {
            token: token_str.clone(),
            expires_at,
        });

        tracing::debug!(
            "New token cached, expires in ~{} minutes",
            expires_at
                .saturating_duration_since(Instant::now())
                .as_secs()
                / 60
        );

        Ok(token_str)
    }

    /// Force refresh the token
    pub async fn refresh_token(&self) -> Result<String> {
        {
            let mut cache = self.token_cache.lock().await;
            *cache = None;
        }
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
    project
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
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
        if !config_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
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

/// Validate a GCP zone name format
/// Zone names follow the pattern: region-zone (e.g., us-central1-a)
/// Only alphanumeric characters and hyphens are allowed
fn validate_zone_name(zone: &str) -> bool {
    if zone.is_empty() || zone.len() > 63 {
        return false;
    }
    zone.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Get the default zone from gcloud configuration
/// Security: Validates zone format and config name before returning
pub fn get_default_zone() -> Option<String> {
    // Check environment variable first
    if let Ok(zone) = std::env::var("CLOUDSDK_COMPUTE_ZONE") {
        if validate_zone_name(&zone) {
            return Some(zone);
        }
        tracing::warn!("Invalid zone format in CLOUDSDK_COMPUTE_ZONE");
        return None;
    }

    // Try to read from gcloud config
    let config_dir = get_gcloud_config_dir()?;

    // Try active configuration
    let active_config_path = config_dir.join("active_config");
    if let Ok(active_config) = std::fs::read_to_string(&active_config_path) {
        let config_name = active_config.trim();

        // Security: Validate config name to prevent path traversal
        if !config_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            tracing::warn!("Invalid characters in active_config name");
            return None;
        }

        let config_path = config_dir
            .join("configurations")
            .join(format!("config_{}", config_name));

        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let mut in_compute_section = false;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                    continue;
                }
                if line == "[compute]" {
                    in_compute_section = true;
                } else if line.starts_with('[') {
                    in_compute_section = false;
                } else if in_compute_section && line.starts_with("zone") {
                    if let Some(value) = line.split('=').nth(1) {
                        let zone = value.trim().to_string();
                        if validate_zone_name(&zone) {
                            return Some(zone);
                        }
                        tracing::warn!("Invalid zone format in gcloud config");
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

    // =========================================================================
    // validate_project_id tests
    // =========================================================================

    #[test]
    fn test_valid_project_ids() {
        assert!(validate_project_id("my-project-123"));
        assert!(validate_project_id("abcdef")); // min 6 chars
        assert!(validate_project_id(&"a".repeat(30))); // max 30 chars
        assert!(validate_project_id("project-1"));
        assert!(validate_project_id("a12345"));
    }

    #[test]
    fn test_project_id_too_short() {
        assert!(!validate_project_id("short")); // 5 chars
        assert!(!validate_project_id("ab"));
        assert!(!validate_project_id(""));
    }

    #[test]
    fn test_project_id_too_long() {
        assert!(!validate_project_id(&"a".repeat(31)));
    }

    #[test]
    fn test_project_id_must_start_with_letter() {
        assert!(!validate_project_id("1starts-with-digit"));
        assert!(!validate_project_id("-starts-with-hyphen"));
    }

    #[test]
    fn test_project_id_no_uppercase() {
        assert!(!validate_project_id("UPPERCASE-PROJECT"));
        assert!(!validate_project_id("Mixed-Case"));
    }

    #[test]
    fn test_project_id_no_end_hyphen() {
        assert!(!validate_project_id("ends-with-"));
    }

    #[test]
    fn test_project_id_no_special_chars() {
        assert!(!validate_project_id("has space here"));
        assert!(!validate_project_id("has_underscore"));
        assert!(!validate_project_id("has.dot.here"));
    }

    // =========================================================================
    // validate_zone_name tests
    // =========================================================================

    #[test]
    fn test_valid_zone_names() {
        assert!(validate_zone_name("us-central1-a"));
        assert!(validate_zone_name("europe-west1-b"));
        assert!(validate_zone_name("asia-east1-c"));
        assert!(validate_zone_name(&"a".repeat(63))); // max 63 chars
    }

    #[test]
    fn test_zone_name_empty() {
        assert!(!validate_zone_name(""));
    }

    #[test]
    fn test_zone_name_too_long() {
        assert!(!validate_zone_name(&"a".repeat(64)));
    }

    #[test]
    fn test_zone_name_no_uppercase() {
        assert!(!validate_zone_name("US-CENTRAL1-A"));
    }

    #[test]
    fn test_zone_name_no_special_chars() {
        assert!(!validate_zone_name("zone with space"));
        assert!(!validate_zone_name("zone_underscore"));
        assert!(!validate_zone_name("zone.dot"));
    }

    // =========================================================================
    // get_gcloud_config_dir tests
    // =========================================================================

    #[test]
    fn test_gcloud_config_dir_returns_some() {
        // In most environments, config_dir() returns Some
        let result = get_gcloud_config_dir();
        // We can't guarantee this is Some in all CI environments,
        // but we can at least verify it doesn't panic
        if let Some(path) = result {
            assert!(path.to_string_lossy().contains("gcloud"));
        }
    }
}
