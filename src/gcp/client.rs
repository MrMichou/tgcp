//! GCP Client
//!
//! Main client for interacting with GCP APIs, combining authentication
//! and HTTP functionality.

use super::auth::GcpCredentials;
use super::http::GcpHttpClient;
use anyhow::{Context, Result};
use serde_json::Value;

/// Main GCP client
#[derive(Clone)]
pub struct GcpClient {
    pub credentials: GcpCredentials,
    pub http: GcpHttpClient,
    pub project_id: String,
    pub zone: String,
}

impl GcpClient {
    /// Create a new GCP client
    pub async fn new(project_id: &str, zone: &str) -> Result<Self> {
        let credentials = GcpCredentials::new()
            .await
            .context("Failed to initialize GCP credentials")?;

        let http = GcpHttpClient::new()?;

        Ok(Self {
            credentials,
            http,
            project_id: project_id.to_string(),
            zone: zone.to_string(),
        })
    }

    /// Get the current access token
    pub async fn get_token(&self) -> Result<String> {
        self.credentials.get_token().await
    }

    /// Make a GET request to a GCP API
    pub async fn get(&self, url: &str) -> Result<Value> {
        let token = self.get_token().await?;
        self.http.get(url, &token).await
    }

    /// Make a POST request to a GCP API
    pub async fn post(&self, url: &str, body: Option<&Value>) -> Result<Value> {
        let token = self.get_token().await?;
        self.http.post(url, &token, body).await
    }

    /// Make a DELETE request to a GCP API
    pub async fn delete(&self, url: &str) -> Result<Value> {
        let token = self.get_token().await?;
        self.http.delete(url, &token).await
    }

    /// Switch to a different project
    pub async fn switch_project(&mut self, project_id: &str) -> Result<()> {
        self.project_id = project_id.to_string();
        // Refresh token in case of project-specific credentials
        self.credentials.refresh_token().await?;
        Ok(())
    }

    /// Switch to a different zone
    pub fn switch_zone(&mut self, zone: &str) {
        self.zone = zone.to_string();
    }

    /// Get the region from the current zone
    pub fn get_region(&self) -> String {
        let parts: Vec<&str> = self.zone.rsplitn(2, '-').collect();
        if parts.len() == 2 {
            parts[1].to_string()
        } else {
            self.zone.clone()
        }
    }

    // =========================================================================
    // Compute Engine API helpers
    // =========================================================================

    /// Build Compute Engine API URL
    pub fn compute_url(&self, path: &str) -> String {
        format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/{}",
            self.project_id, path
        )
    }

    /// Build zonal Compute Engine API URL
    pub fn compute_zonal_url(&self, resource: &str) -> String {
        self.compute_url(&format!("zones/{}/{}", self.zone, resource))
    }

    /// Build regional Compute Engine API URL
    pub fn compute_regional_url(&self, resource: &str) -> String {
        self.compute_url(&format!("regions/{}/{}", self.get_region(), resource))
    }

    /// Build global Compute Engine API URL
    pub fn compute_global_url(&self, resource: &str) -> String {
        self.compute_url(&format!("global/{}", resource))
    }

    /// Build aggregated Compute Engine API URL (all zones)
    pub fn compute_aggregated_url(&self, resource: &str) -> String {
        self.compute_url(&format!("aggregated/{}", resource))
    }

    // =========================================================================
    // Cloud Storage API helpers
    // =========================================================================

    /// Build Cloud Storage API URL
    pub fn storage_url(&self, path: &str) -> String {
        format!("https://storage.googleapis.com/storage/v1/{}", path)
    }

    /// Build Cloud Storage bucket URL
    pub fn storage_bucket_url(&self, bucket: &str) -> String {
        self.storage_url(&format!("b/{}", bucket))
    }

    /// Build Cloud Storage objects URL
    pub fn storage_objects_url(&self, bucket: &str) -> String {
        self.storage_url(&format!("b/{}/o", bucket))
    }

    // =========================================================================
    // GKE API helpers
    // =========================================================================

    /// Build GKE API URL
    pub fn container_url(&self, path: &str) -> String {
        format!(
            "https://container.googleapis.com/v1/projects/{}/{}",
            self.project_id, path
        )
    }

    /// Build GKE location URL (region or zone)
    pub fn container_location_url(&self, location: &str, resource: &str) -> String {
        self.container_url(&format!("locations/{}/{}", location, resource))
    }

    // =========================================================================
    // Resource Manager API helpers
    // =========================================================================

    /// Build Resource Manager API URL
    pub fn resourcemanager_url(&self, path: &str) -> String {
        format!("https://cloudresourcemanager.googleapis.com/v1/{}", path)
    }
}

/// Format a GCP API error for display
pub fn format_gcp_error(error: &anyhow::Error) -> String {
    super::http::format_gcp_error(error)
}
