//! Resource Fetcher
//!
//! Handles fetching resources from GCP APIs based on resource definitions.

use super::registry::{get_resource, ResourceDef};
use super::sdk_dispatch;
use crate::gcp::client::GcpClient;
use anyhow::Result;
use serde_json::Value;

/// Filter for resources
#[derive(Debug, Clone)]
pub struct ResourceFilter {
    pub param: String,
    pub values: Vec<String>,
}

impl ResourceFilter {
    pub fn new(param: &str, values: Vec<String>) -> Self {
        Self {
            param: param.to_string(),
            values,
        }
    }
}

/// Result of paginated fetch
pub struct PaginatedResult {
    pub items: Vec<Value>,
    pub next_token: Option<String>,
}

/// Fetch all resources (auto-paginate)
pub async fn fetch_resources(
    resource_key: &str,
    client: &GcpClient,
    filters: &[ResourceFilter],
) -> Result<Vec<Value>> {
    let mut all_items = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let result = fetch_resources_paginated(resource_key, client, filters, page_token.as_deref()).await?;
        all_items.extend(result.items);

        if result.next_token.is_none() {
            break;
        }
        page_token = result.next_token;
    }

    Ok(all_items)
}

/// Fetch one page of resources
pub async fn fetch_resources_paginated(
    resource_key: &str,
    client: &GcpClient,
    filters: &[ResourceFilter],
    page_token: Option<&str>,
) -> Result<PaginatedResult> {
    let Some(resource_def) = get_resource(resource_key) else {
        return Err(anyhow::anyhow!("Unknown resource: {}", resource_key));
    };

    // Build params
    let mut params = resource_def.sdk_method_params.clone();
    if params.is_null() {
        params = Value::Object(serde_json::Map::new());
    }

    // Add filters
    if let Value::Object(ref mut map) = params {
        for filter in filters {
            map.insert(filter.param.clone(), Value::Array(
                filter.values.iter().map(|v| Value::String(v.clone())).collect()
            ));
        }

        // Add page token
        if let Some(token) = page_token {
            map.insert("pageToken".to_string(), Value::String(token.to_string()));
        }
    }

    // Invoke SDK method
    let response = sdk_dispatch::invoke_sdk(
        &resource_def.service,
        &resource_def.sdk_method,
        client,
        &params,
    ).await?;

    // Extract items from response path
    let items = extract_items(&response, &resource_def.response_path, resource_def);

    // Get next page token
    let next_token = response
        .get("nextPageToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(PaginatedResult { items, next_token })
}

/// Extract items from response using the response_path
fn extract_items(response: &Value, path: &str, resource_def: &ResourceDef) -> Vec<Value> {
    let raw_items = if path.is_empty() {
        if let Some(arr) = response.as_array() {
            arr.clone()
        } else {
            vec![]
        }
    } else {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = response;

        for part in parts {
            current = match current.get(part) {
                Some(v) => v,
                None => return vec![],
            };
        }

        current.as_array().cloned().unwrap_or_default()
    };

    // Post-process items to add computed fields
    raw_items
        .into_iter()
        .map(|item| post_process_item(item, resource_def))
        .collect()
}

/// Post-process an item to add computed/derived fields
fn post_process_item(mut item: Value, resource_def: &ResourceDef) -> Value {
    if let Value::Object(ref mut map) = item {
        // Extract short names from full URLs
        if let Some(zone) = map.get("zone").and_then(|v| v.as_str()) {
            let short = extract_short_name(zone);
            map.insert("zone_short".to_string(), Value::String(short));
        }

        if let Some(region) = map.get("region").and_then(|v| v.as_str()) {
            let short = extract_short_name(region);
            map.insert("region_short".to_string(), Value::String(short));
        }

        if let Some(machine_type) = map.get("machineType").and_then(|v| v.as_str()) {
            let short = extract_short_name(machine_type);
            map.insert("machineType_short".to_string(), Value::String(short));
        }

        if let Some(disk_type) = map.get("type").and_then(|v| v.as_str()) {
            let short = extract_short_name(disk_type);
            map.insert("type_short".to_string(), Value::String(short));
        }

        if let Some(network) = map.get("network").and_then(|v| v.as_str()) {
            let short = extract_short_name(network);
            map.insert("network_short".to_string(), Value::String(short));
        }

        // Count arrays
        if let Some(users) = map.get("users").and_then(|v| v.as_array()) {
            map.insert("users_count".to_string(), Value::String(users.len().to_string()));
        }

        if let Some(subnets) = map.get("subnetworks").and_then(|v| v.as_array()) {
            map.insert("subnetworks_count".to_string(), Value::String(subnets.len().to_string()));
        }

        // Format booleans
        if let Some(auto_create) = map.get("autoCreateSubnetworks").and_then(|v| v.as_bool()) {
            let display = if auto_create { "Auto" } else { "Custom" };
            map.insert("autoCreateSubnetworks_display".to_string(), Value::String(display.to_string()));
        }

        // Firewall action display
        if map.contains_key("allowed") {
            map.insert("action_display".to_string(), Value::String("ALLOW".to_string()));
        } else if map.contains_key("denied") {
            map.insert("action_display".to_string(), Value::String("DENY".to_string()));
        }

        // Format timestamps
        if let Some(created) = map.get("timeCreated").and_then(|v| v.as_str()) {
            let short = format_timestamp_short(created);
            map.insert("timeCreated_short".to_string(), Value::String(short));
        }

        if let Some(updated) = map.get("updated").and_then(|v| v.as_str()) {
            let short = format_timestamp_short(updated);
            map.insert("updated_short".to_string(), Value::String(short));
        }

        // Format size
        if let Some(size) = map.get("size").and_then(|v| v.as_str()) {
            let display = format_bytes(size.parse().unwrap_or(0));
            map.insert("size_display".to_string(), Value::String(display));
        }

        // GKE specific
        if let Some(autopilot) = map.get("autopilot").and_then(|v| v.get("enabled")).and_then(|v| v.as_bool()) {
            let display = if autopilot { "Autopilot" } else { "Standard" };
            map.insert("autopilot_display".to_string(), Value::String(display.to_string()));
        } else {
            map.insert("autopilot_display".to_string(), Value::String("Standard".to_string()));
        }

        if let Some(autoscaling) = map.get("autoscaling").and_then(|v| v.get("enabled")).and_then(|v| v.as_bool()) {
            let display = if autoscaling { "Yes" } else { "No" };
            map.insert("autoscaling_display".to_string(), Value::String(display.to_string()));
        }
    }

    let _ = resource_def; // Silence unused warning
    item
}

/// Extract short name from GCP resource URL
/// e.g., "https://www.googleapis.com/compute/v1/projects/my-project/zones/us-central1-a" -> "us-central1-a"
fn extract_short_name(url: &str) -> String {
    url.rsplit('/').next().unwrap_or(url).to_string()
}

/// Format timestamp to short form
fn format_timestamp_short(timestamp: &str) -> String {
    // RFC3339 format: 2023-01-15T10:30:00.000Z
    if timestamp.len() >= 10 {
        timestamp[..10].to_string()
    } else {
        timestamp.to_string()
    }
}

/// Format bytes to human readable
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Extract a value from JSON using a dot-notation path
pub fn extract_json_value(item: &Value, path: &str) -> String {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = item;

    for part in parts {
        // Handle array index
        if let Ok(idx) = part.parse::<usize>() {
            current = match current.get(idx) {
                Some(v) => v,
                None => return "-".to_string(),
            };
        } else {
            current = match current.get(part) {
                Some(v) => v,
                None => return "-".to_string(),
            };
        }
    }

    match current {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "-".to_string(),
        Value::Array(arr) => format!("[{} items]", arr.len()),
        Value::Object(_) => "[object]".to_string(),
    }
}
