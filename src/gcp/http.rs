//! HTTP utilities for GCP REST API calls

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;

/// Base URLs for GCP services
#[allow(dead_code)]
pub mod base_urls {
    pub const COMPUTE: &str = "https://compute.googleapis.com/compute/v1";
    pub const STORAGE: &str = "https://storage.googleapis.com/storage/v1";
    pub const CONTAINER: &str = "https://container.googleapis.com/v1";
    pub const CLOUDRESOURCEMANAGER: &str = "https://cloudresourcemanager.googleapis.com/v1";
    pub const IAM: &str = "https://iam.googleapis.com/v1";
}

/// HTTP client wrapper for GCP API calls
#[derive(Clone)]
pub struct GcpHttpClient {
    client: Client,
}

impl GcpHttpClient {
    /// Create a new HTTP client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent("tgcp/0.1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client })
    }

    /// Make a GET request to a GCP API
    pub async fn get(&self, url: &str, token: &str) -> Result<Value> {
        tracing::debug!("GET {}", url);

        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to send request")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read response body")?;

        if !status.is_success() {
            tracing::error!("API error: {} - {}", status, body);
            return Err(anyhow::anyhow!("API request failed: {} - {}", status, body));
        }

        serde_json::from_str(&body).context("Failed to parse response JSON")
    }

    /// Make a POST request to a GCP API
    pub async fn post(&self, url: &str, token: &str, body: Option<&Value>) -> Result<Value> {
        tracing::debug!("POST {}", url);

        let mut request = self.client.post(url).bearer_auth(token);

        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await.context("Failed to send request")?;

        let status = response.status();
        let response_body = response
            .text()
            .await
            .context("Failed to read response body")?;

        if !status.is_success() {
            tracing::error!("API error: {} - {}", status, response_body);
            return Err(anyhow::anyhow!(
                "API request failed: {} - {}",
                status,
                response_body
            ));
        }

        // Handle empty response
        if response_body.is_empty() {
            return Ok(Value::Null);
        }

        serde_json::from_str(&response_body).context("Failed to parse response JSON")
    }

    /// Make a DELETE request to a GCP API
    pub async fn delete(&self, url: &str, token: &str) -> Result<Value> {
        tracing::debug!("DELETE {}", url);

        let response = self
            .client
            .delete(url)
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to send request")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read response body")?;

        if !status.is_success() {
            tracing::error!("API error: {} - {}", status, body);
            return Err(anyhow::anyhow!("API request failed: {} - {}", status, body));
        }

        // Handle empty response
        if body.is_empty() {
            return Ok(Value::Null);
        }

        serde_json::from_str(&body).context("Failed to parse response JSON")
    }
}

impl Default for GcpHttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default HTTP client")
    }
}

/// Format a GCP API error for display
pub fn format_gcp_error(error: &anyhow::Error) -> String {
    let error_str = error.to_string();

    // Try to extract meaningful message from JSON error response
    if let Ok(json) = serde_json::from_str::<Value>(&error_str) {
        if let Some(message) = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            return message.to_string();
        }
    }

    // Clean up common error patterns
    if error_str.contains("403") {
        return "Permission denied. Check your GCP IAM permissions.".to_string();
    }
    if error_str.contains("401") {
        return "Authentication failed. Run 'gcloud auth application-default login'.".to_string();
    }
    if error_str.contains("404") {
        return "Resource not found.".to_string();
    }
    if error_str.contains("429") {
        return "Rate limit exceeded. Please try again later.".to_string();
    }

    // Truncate long error messages
    if error_str.len() > 100 {
        format!("{}...", &error_str[..100])
    } else {
        error_str
    }
}
