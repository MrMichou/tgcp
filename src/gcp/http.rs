//! HTTP utilities for GCP REST API calls

use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::time::Duration;

/// Maximum length of response body to log (to avoid logging sensitive data)
const MAX_LOG_BODY_LENGTH: usize = 200;

/// Maximum number of retry attempts for transient errors
const MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff (milliseconds)
const BASE_DELAY_MS: u64 = 500;

/// Maximum delay cap (milliseconds)
const MAX_DELAY_MS: u64 = 10_000;

/// Check if a status code is retryable (transient error)
fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS       // 429
        | StatusCode::SERVICE_UNAVAILABLE   // 503
        | StatusCode::GATEWAY_TIMEOUT       // 504
        | StatusCode::BAD_GATEWAY // 502
    )
}

/// Calculate delay with exponential backoff and jitter
fn calculate_backoff_delay(attempt: u32) -> Duration {
    let base_delay = BASE_DELAY_MS * 2u64.pow(attempt);
    let capped_delay = base_delay.min(MAX_DELAY_MS);
    // Add jitter: random value between 0 and 50% of the delay
    let jitter = (capped_delay as f64 * rand_jitter()) as u64;
    Duration::from_millis(capped_delay + jitter)
}

/// Simple pseudo-random jitter factor (0.0 to 0.5)
/// Uses system time for simple randomness without external deps
fn rand_jitter() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 500) as f64 / 1000.0
}

/// Sanitize response body for logging
/// Truncates long responses and masks potentially sensitive patterns
fn sanitize_for_log(body: &str) -> String {
    // Truncate long responses
    let truncated = if body.len() > MAX_LOG_BODY_LENGTH {
        format!(
            "{}... [truncated, {} bytes total]",
            &body[..MAX_LOG_BODY_LENGTH],
            body.len()
        )
    } else {
        body.to_string()
    };

    // Mask patterns that might contain sensitive data
    // This is a basic implementation - could be expanded
    truncated.replace(|c: char| !c.is_ascii_graphic() && c != ' ', "")
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

    /// Make a GET request to a GCP API with retry logic for transient errors
    pub async fn get(&self, url: &str, token: &str) -> Result<Value> {
        tracing::debug!("GET {}", url);

        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
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

            if status.is_success() {
                return serde_json::from_str(&body).context("Failed to parse response JSON");
            }

            // Check if error is retryable
            if is_retryable_status(status) && attempt < MAX_RETRIES {
                let delay = calculate_backoff_delay(attempt);
                tracing::warn!(
                    "Transient error {} on GET {}, retrying in {:?} (attempt {}/{})",
                    status,
                    url,
                    delay,
                    attempt + 1,
                    MAX_RETRIES
                );
                tokio::time::sleep(delay).await;
                last_error = Some(anyhow::anyhow!("API request failed: {}", status));
                continue;
            }

            // Non-retryable error or max retries exceeded
            // Security: Only log sanitized/truncated error body to avoid leaking sensitive data
            tracing::error!("API error: {} - {}", status, sanitize_for_log(&body));
            return Err(anyhow::anyhow!("API request failed: {}", status));
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Request failed after retries")))
    }

    /// Make a POST request to a GCP API with retry logic for transient errors
    pub async fn post(&self, url: &str, token: &str, body: Option<&Value>) -> Result<Value> {
        tracing::debug!("POST {}", url);

        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
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

            if status.is_success() {
                // Handle empty response
                if response_body.is_empty() {
                    return Ok(Value::Null);
                }
                return serde_json::from_str(&response_body)
                    .context("Failed to parse response JSON");
            }

            // Check if error is retryable
            if is_retryable_status(status) && attempt < MAX_RETRIES {
                let delay = calculate_backoff_delay(attempt);
                tracing::warn!(
                    "Transient error {} on POST {}, retrying in {:?} (attempt {}/{})",
                    status,
                    url,
                    delay,
                    attempt + 1,
                    MAX_RETRIES
                );
                tokio::time::sleep(delay).await;
                last_error = Some(anyhow::anyhow!("API request failed: {}", status));
                continue;
            }

            // Non-retryable error or max retries exceeded
            // Security: Only log sanitized/truncated error body to avoid leaking sensitive data
            tracing::error!(
                "API error: {} - {}",
                status,
                sanitize_for_log(&response_body)
            );
            return Err(anyhow::anyhow!("API request failed: {}", status));
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Request failed after retries")))
    }

    /// Make a DELETE request to a GCP API with retry logic for transient errors
    pub async fn delete(&self, url: &str, token: &str) -> Result<Value> {
        tracing::debug!("DELETE {}", url);

        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
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

            if status.is_success() {
                // Handle empty response
                if body.is_empty() {
                    return Ok(Value::Null);
                }
                return serde_json::from_str(&body).context("Failed to parse response JSON");
            }

            // Check if error is retryable
            if is_retryable_status(status) && attempt < MAX_RETRIES {
                let delay = calculate_backoff_delay(attempt);
                tracing::warn!(
                    "Transient error {} on DELETE {}, retrying in {:?} (attempt {}/{})",
                    status,
                    url,
                    delay,
                    attempt + 1,
                    MAX_RETRIES
                );
                tokio::time::sleep(delay).await;
                last_error = Some(anyhow::anyhow!("API request failed: {}", status));
                continue;
            }

            // Non-retryable error or max retries exceeded
            // Security: Only log sanitized/truncated error body to avoid leaking sensitive data
            tracing::error!("API error: {} - {}", status, sanitize_for_log(&body));
            return Err(anyhow::anyhow!("API request failed: {}", status));
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Request failed after retries")))
    }
}

// Note: Default is intentionally not implemented for GcpHttpClient
// because new() can fail. Use GcpHttpClient::new() explicitly and handle errors.

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
