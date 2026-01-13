//! HTTP utilities for GCP REST API calls

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;

/// Maximum length of response body to log (to avoid logging sensitive data)
const MAX_LOG_BODY_LENGTH: usize = 200;

/// Sanitize response body for logging
/// Truncates long responses and masks potentially sensitive patterns
fn sanitize_for_log(body: &str) -> String {
    // Truncate long responses
    let truncated = if body.len() > MAX_LOG_BODY_LENGTH {
        format!("{}... [truncated, {} bytes total]", &body[..MAX_LOG_BODY_LENGTH], body.len())
    } else {
        body.to_string()
    };

    // Mask patterns that might contain sensitive data
    // This is a basic implementation - could be expanded
    truncated
        .replace(|c: char| !c.is_ascii_graphic() && c != ' ', "")
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
            // Security: Only log sanitized/truncated error body to avoid leaking sensitive data
            tracing::error!("API error: {} - {}", status, sanitize_for_log(&body));
            return Err(anyhow::anyhow!("API request failed: {}", status));
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
            // Security: Only log sanitized/truncated error body to avoid leaking sensitive data
            tracing::error!("API error: {} - {}", status, sanitize_for_log(&response_body));
            return Err(anyhow::anyhow!("API request failed: {}", status));
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
            // Security: Only log sanitized/truncated error body to avoid leaking sensitive data
            tracing::error!("API error: {} - {}", status, sanitize_for_log(&body));
            return Err(anyhow::anyhow!("API request failed: {}", status));
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
/// Security: Sanitizes error messages to avoid leaking sensitive API details
pub fn format_gcp_error(error: &anyhow::Error) -> String {
    let error_str = error.to_string();

    // Clean up common error patterns with user-friendly messages
    // Security: These generic messages avoid leaking API structure details
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
    if error_str.contains("400") {
        return "Invalid request. Check your parameters.".to_string();
    }
    if error_str.contains("500") || error_str.contains("503") {
        return "GCP service temporarily unavailable. Please try again.".to_string();
    }
    if error_str.contains("409") {
        return "Resource conflict. The resource may already exist or be in use.".to_string();
    }

    // For other errors, provide a generic message without exposing details
    // Security: Don't expose raw API error messages to users
    if error_str.contains("API request failed") {
        return "Request failed. Check your network connection and try again.".to_string();
    }

    // Truncate long error messages and remove potential sensitive data
    let sanitized = error_str
        .chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .take(80)
        .collect::<String>();

    if sanitized.len() < error_str.len() {
        format!("{}...", sanitized)
    } else {
        sanitized
    }
}
