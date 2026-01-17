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
    // Cloud Billing API helpers
    // =========================================================================

    /// Build Cloud Billing API URL
    pub fn billing_url(&self, path: &str) -> String {
        format!("https://cloudbilling.googleapis.com/v1/{}", path)
    }

    /// Build Cloud Billing Budget API URL for a billing account
    pub fn billing_budgets_url(&self, billing_account: &str, path: &str) -> String {
        format!(
            "https://billingbudgets.googleapis.com/v1/{}/{}",
            billing_account, path
        )
    }

    // =========================================================================
    // Resource Manager API helpers
    // =========================================================================

    /// Build Resource Manager API URL
    pub fn resourcemanager_url(&self, path: &str) -> String {
        format!("https://cloudresourcemanager.googleapis.com/v1/{}", path)
    }

    /// List all available zones for the current project
    pub async fn list_zones(&self) -> Result<Vec<String>> {
        let url = self.compute_url("zones");
        let response = self.get(&url).await?;

        let zones = response
            .get("items")
            .and_then(|v| v.as_array())
            .map(|arr| {
                let mut zones: Vec<String> = arr
                    .iter()
                    .filter_map(|z| z.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect();
                zones.sort();
                zones
            })
            .unwrap_or_default();

        Ok(zones)
    }

    // =========================================================================
    // Operations API helpers
    // =========================================================================

    /// Build zonal operations URL
    pub fn compute_zonal_operation_url(&self, operation: &str) -> String {
        self.compute_url(&format!("zones/{}/operations/{}", self.zone, operation))
    }

    /// Build global operations URL
    pub fn compute_global_operation_url(&self, operation: &str) -> String {
        self.compute_url(&format!("global/operations/{}", operation))
    }

    /// Poll a GCP operation until completion
    /// Returns the operation status: "RUNNING", "DONE", or error
    pub async fn poll_operation(&self, operation_url: &str) -> Result<OperationStatus> {
        let response = self.get(operation_url).await?;

        let status = response
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("UNKNOWN");

        match status {
            "DONE" => {
                // Check for errors in the operation
                if let Some(error) = response.get("error") {
                    let error_msg = error
                        .get("errors")
                        .and_then(|e| e.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown operation error");
                    Ok(OperationStatus::Failed(error_msg.to_string()))
                } else {
                    Ok(OperationStatus::Done)
                }
            }
            "RUNNING" | "PENDING" => Ok(OperationStatus::Running),
            other => Ok(OperationStatus::Unknown(other.to_string())),
        }
    }
}

/// Status of a GCP operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationStatus {
    Running,
    Done,
    Failed(String),
    Unknown(String),
}

impl OperationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed(_))
    }
}

/// Extract operation self-link URL from a GCP API response
pub fn extract_operation_url(response: &Value) -> Option<String> {
    response
        .get("selfLink")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Extract operation name from a GCP API response
pub fn extract_operation_name(response: &Value) -> Option<String> {
    response.get("name").and_then(|v| v.as_str()).map(String::from)
}

/// Format a GCP API error for display
pub fn format_gcp_error(error: &anyhow::Error) -> String {
    super::http::format_gcp_error(error)
}
