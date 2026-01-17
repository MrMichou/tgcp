//! Resource Fetcher
//!
//! Handles fetching resources from GCP APIs based on resource definitions.
//! Supports both sequential and concurrent pagination for performance.

use super::registry::{get_resource, ResourceDef};
use super::sdk_dispatch;
use crate::gcp::client::GcpClient;
use anyhow::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Semaphore;

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
        let result =
            fetch_resources_paginated(resource_key, client, filters, page_token.as_deref()).await?;
        all_items.extend(result.items);

        if result.next_token.is_none() {
            break;
        }
        page_token = result.next_token;
    }

    Ok(all_items)
}

/// Fetch multiple resource types concurrently
/// Returns a vector of results in the same order as the input resource keys
#[allow(dead_code)]
pub async fn fetch_multiple_resources(
    resource_keys: &[&str],
    client: &GcpClient,
    max_concurrent: usize,
) -> Vec<Result<Vec<Value>>> {
    let semaphore = Arc::new(Semaphore::new(max_concurrent.max(1)));
    let mut futures = FuturesUnordered::new();

    for (idx, &resource_key) in resource_keys.iter().enumerate() {
        let sem = Arc::clone(&semaphore);
        let key = resource_key.to_string();
        let client = client.clone();

        futures.push(async move {
            let _permit = sem.acquire().await.unwrap();
            let result = fetch_resources(&key, &client, &[]).await;
            (idx, result)
        });
    }

    // Collect results preserving order
    let mut results: Vec<Option<Result<Vec<Value>>>> =
        (0..resource_keys.len()).map(|_| None).collect();

    while let Some((idx, result)) = futures.next().await {
        results[idx] = Some(result);
    }

    results.into_iter().map(|r| r.unwrap()).collect()
}

/// Fetch all pages concurrently with speculative fetching
/// Uses a sliding window approach: fetch first page, then speculatively fetch more
#[allow(dead_code)]
pub async fn fetch_resources_concurrent(
    resource_key: &str,
    client: &GcpClient,
    filters: &[ResourceFilter],
    max_concurrent: usize,
) -> Result<Vec<Value>> {
    // First, fetch initial page to see if there are more
    let first_result = fetch_resources_paginated(resource_key, client, filters, None).await?;

    let mut all_items = first_result.items;

    // If no more pages, return early
    let Some(first_next_token) = first_result.next_token else {
        return Ok(all_items);
    };

    // Concurrent fetch of remaining pages using a semaphore for rate limiting
    let semaphore = Arc::new(Semaphore::new(max_concurrent.max(1)));
    let mut page_tokens = vec![first_next_token];
    let mut page_results: Vec<Vec<Value>> = Vec::new();

    // Fetch pages in batches until no more tokens
    loop {
        if page_tokens.is_empty() {
            break;
        }

        let mut futures = FuturesUnordered::new();
        let current_tokens: Vec<String> = std::mem::take(&mut page_tokens);

        for (batch_idx, token) in current_tokens.into_iter().enumerate() {
            let sem = Arc::clone(&semaphore);
            let key = resource_key.to_string();
            let client = client.clone();
            let filters = filters.to_vec();

            futures.push(async move {
                let _permit = sem.acquire().await.unwrap();
                let result = fetch_resources_paginated(&key, &client, &filters, Some(&token)).await;
                (batch_idx, result)
            });
        }

        // Collect batch results
        let batch_count = futures.len();
        let mut batch_results: Vec<Option<Result<PaginatedResult>>> =
            (0..batch_count).map(|_| None).collect();

        while let Some((idx, result)) = futures.next().await {
            batch_results[idx] = Some(result);
        }

        // Process results in order
        for result_opt in batch_results {
            match result_opt.unwrap() {
                Ok(result) => {
                    page_results.push(result.items);
                    if let Some(next_token) = result.next_token {
                        page_tokens.push(next_token);
                    }
                },
                Err(e) => {
                    // Log error but continue with other pages
                    tracing::warn!("Error fetching page: {}", e);
                },
            }
        }
    }

    // Combine all results
    for items in page_results {
        all_items.extend(items);
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
            map.insert(
                filter.param.clone(),
                Value::Array(
                    filter
                        .values
                        .iter()
                        .map(|v| Value::String(v.clone()))
                        .collect(),
                ),
            );
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
    )
    .await?;

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
            map.insert(
                "machineType_short".to_string(),
                Value::String(short.clone()),
            );

            // Extract vCPUs from machine type name (e.g., n1-standard-4 -> 4)
            let vcpus = extract_vcpus_from_machine_type(&short);
            map.insert("vcpus".to_string(), Value::String(vcpus));
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
            map.insert(
                "users_count".to_string(),
                Value::String(users.len().to_string()),
            );
        }

        if let Some(subnets) = map.get("subnetworks").and_then(|v| v.as_array()) {
            map.insert(
                "subnetworks_count".to_string(),
                Value::String(subnets.len().to_string()),
            );
        }

        // Format booleans
        if let Some(auto_create) = map.get("autoCreateSubnetworks").and_then(|v| v.as_bool()) {
            let display = if auto_create { "Auto" } else { "Custom" };
            map.insert(
                "autoCreateSubnetworks_display".to_string(),
                Value::String(display.to_string()),
            );
        }

        // Firewall action display
        if map.contains_key("allowed") {
            map.insert(
                "action_display".to_string(),
                Value::String("ALLOW".to_string()),
            );
        } else if map.contains_key("denied") {
            map.insert(
                "action_display".to_string(),
                Value::String("DENY".to_string()),
            );
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
        if let Some(autopilot) = map
            .get("autopilot")
            .and_then(|v| v.get("enabled"))
            .and_then(|v| v.as_bool())
        {
            let display = if autopilot { "Autopilot" } else { "Standard" };
            map.insert(
                "autopilot_display".to_string(),
                Value::String(display.to_string()),
            );
        } else {
            map.insert(
                "autopilot_display".to_string(),
                Value::String("Standard".to_string()),
            );
        }

        if let Some(autoscaling) = map
            .get("autoscaling")
            .and_then(|v| v.get("enabled"))
            .and_then(|v| v.as_bool())
        {
            let display = if autoscaling { "Yes" } else { "No" };
            map.insert(
                "autoscaling_display".to_string(),
                Value::String(display.to_string()),
            );
        }

        // CDN / Load Balancing specific fields
        // enableCDN for backend services
        if let Some(enable_cdn) = map.get("enableCDN").and_then(|v| v.as_bool()) {
            let display = if enable_cdn { "Yes" } else { "No" };
            map.insert(
                "enableCDN_display".to_string(),
                Value::String(display.to_string()),
            );
        }

        // enableCdn for backend buckets (note different case)
        if let Some(enable_cdn) = map.get("enableCdn").and_then(|v| v.as_bool()) {
            let display = if enable_cdn { "Yes" } else { "No" };
            map.insert(
                "enableCdn_display".to_string(),
                Value::String(display.to_string()),
            );
        }

        // Count backends
        if let Some(backends) = map.get("backends").and_then(|v| v.as_array()) {
            map.insert(
                "backends_count".to_string(),
                Value::String(backends.len().to_string()),
            );
        }

        // Short name for health checks (take first one)
        if let Some(health_checks) = map.get("healthChecks").and_then(|v| v.as_array()) {
            let display = health_checks
                .first()
                .and_then(|v| v.as_str())
                .map(extract_short_name)
                .unwrap_or_else(|| "-".to_string());
            map.insert("healthChecks_short".to_string(), Value::String(display));
        }

        // Count host rules
        if let Some(host_rules) = map.get("hostRules").and_then(|v| v.as_array()) {
            map.insert(
                "hostRules_count".to_string(),
                Value::String(host_rules.len().to_string()),
            );
        }

        // Count path matchers
        if let Some(path_matchers) = map.get("pathMatchers").and_then(|v| v.as_array()) {
            map.insert(
                "pathMatchers_count".to_string(),
                Value::String(path_matchers.len().to_string()),
            );
        }

        // Short name for default service
        if let Some(default_service) = map.get("defaultService").and_then(|v| v.as_str()) {
            let short = extract_short_name(default_service);
            map.insert("defaultService_short".to_string(), Value::String(short));
        }

        // Short name for URL map
        if let Some(url_map) = map.get("urlMap").and_then(|v| v.as_str()) {
            let short = extract_short_name(url_map);
            map.insert("urlMap_short".to_string(), Value::String(short));
        }

        // Count SSL certificates
        if let Some(ssl_certs) = map.get("sslCertificates").and_then(|v| v.as_array()) {
            map.insert(
                "sslCertificates_count".to_string(),
                Value::String(ssl_certs.len().to_string()),
            );
        }

        // Short name for SSL policy
        if let Some(ssl_policy) = map.get("sslPolicy").and_then(|v| v.as_str()) {
            let short = extract_short_name(ssl_policy);
            map.insert("sslPolicy_short".to_string(), Value::String(short));
        }

        // Short name for target (forwarding rules)
        if let Some(target) = map.get("target").and_then(|v| v.as_str()) {
            let short = extract_short_name(target);
            map.insert("target_short".to_string(), Value::String(short));
        }

        // Display subject alternative names (first 3)
        if let Some(sans) = map
            .get("subjectAlternativeNames")
            .and_then(|v| v.as_array())
        {
            let display: Vec<&str> = sans.iter().filter_map(|v| v.as_str()).take(3).collect();
            let suffix = if sans.len() > 3 {
                format!(" +{}", sans.len() - 3)
            } else {
                String::new()
            };
            map.insert(
                "subjectAlternativeNames_display".to_string(),
                Value::String(format!("{}{}", display.join(", "), suffix)),
            );
        }

        // Short expire time
        if let Some(expire_time) = map.get("expireTime").and_then(|v| v.as_str()) {
            let short = format_timestamp_short(expire_time);
            map.insert("expireTime_short".to_string(), Value::String(short));
        }

        // Load Balancing specific fields
        // Health check port (extract from type-specific config)
        let port = map
            .get("httpHealthCheck")
            .or_else(|| map.get("httpsHealthCheck"))
            .or_else(|| map.get("tcpHealthCheck"))
            .or_else(|| map.get("sslHealthCheck"))
            .or_else(|| map.get("http2HealthCheck"))
            .or_else(|| map.get("grpcHealthCheck"))
            .and_then(|v| v.get("port"))
            .and_then(|v| v.as_i64())
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        map.insert("healthCheck_port".to_string(), Value::String(port));

        // Count instances in target pool
        if let Some(instances) = map.get("instances").and_then(|v| v.as_array()) {
            map.insert(
                "instances_count".to_string(),
                Value::String(instances.len().to_string()),
            );
        }

        // Short name for backup pool
        if let Some(backup_pool) = map.get("backupPool").and_then(|v| v.as_str()) {
            let short = extract_short_name(backup_pool);
            map.insert("backupPool_short".to_string(), Value::String(short));
        }

        // Short name for service (TCP/SSL proxies)
        if let Some(service) = map.get("service").and_then(|v| v.as_str()) {
            let short = extract_short_name(service);
            map.insert("service_short".to_string(), Value::String(short));
        }

        // Count enabled features in SSL policy
        if let Some(features) = map.get("enabledFeatures").and_then(|v| v.as_array()) {
            map.insert(
                "enabledFeatures_count".to_string(),
                Value::String(features.len().to_string()),
            );
        }

        // Count rules in security policy
        if let Some(rules) = map.get("rules").and_then(|v| v.as_array()) {
            map.insert(
                "rules_count".to_string(),
                Value::String(rules.len().to_string()),
            );
        }

        // Adaptive protection config display
        if let Some(adaptive) = map
            .get("adaptiveProtectionConfig")
            .and_then(|v| v.get("layer7DdosDefenseConfig"))
            .and_then(|v| v.get("enable"))
            .and_then(|v| v.as_bool())
        {
            let display = if adaptive { "Yes" } else { "No" };
            map.insert(
                "adaptiveProtectionConfig_display".to_string(),
                Value::String(display.to_string()),
            );
        } else {
            map.insert(
                "adaptiveProtectionConfig_display".to_string(),
                Value::String("-".to_string()),
            );
        }

        // VM Instance specific fields
        // Count attached disks
        if let Some(disks) = map.get("disks").and_then(|v| v.as_array()) {
            map.insert(
                "disks_count".to_string(),
                Value::String(disks.len().to_string()),
            );
        }

        // Preemptible/Spot status
        let provisioning_model = map
            .get("scheduling")
            .and_then(|v| v.get("provisioningModel"))
            .and_then(|v| v.as_str())
            .unwrap_or("STANDARD");
        let preemptible = map
            .get("scheduling")
            .and_then(|v| v.get("preemptible"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let scheduling_display = if provisioning_model == "SPOT" {
            "Spot"
        } else if preemptible {
            "Preempt"
        } else {
            "Standard"
        };
        map.insert(
            "scheduling_display".to_string(),
            Value::String(scheduling_display.to_string()),
        );

        // Creation timestamp
        if let Some(created) = map.get("creationTimestamp").and_then(|v| v.as_str()) {
            let short = format_timestamp_short(created);
            map.insert("creationTimestamp_short".to_string(), Value::String(short));
        }

        // Labels count
        if let Some(labels) = map.get("labels").and_then(|v| v.as_object()) {
            map.insert(
                "labels_count".to_string(),
                Value::String(labels.len().to_string()),
            );
        } else {
            map.insert("labels_count".to_string(), Value::String("0".to_string()));
        }
    }

    let _ = resource_def; // Silence unused warning
    item
}

/// Extract short name from GCP resource URL
/// e.g., `https://www.googleapis.com/compute/v1/projects/my-project/zones/us-central1-a` -> `us-central1-a`
fn extract_short_name(url: &str) -> String {
    url.rsplit('/').next().unwrap_or(url).to_string()
}

/// Extract vCPUs from machine type name
/// e.g., `n1-standard-4` -> `4`, `e2-medium` -> `1`, `c2-standard-60` -> `60`
fn extract_vcpus_from_machine_type(machine_type: &str) -> String {
    // Handle custom machine types: custom-N-M where N is vCPUs
    if machine_type.starts_with("custom-") || machine_type.starts_with("n1-custom-") {
        let parts: Vec<&str> = machine_type.split('-').collect();
        if parts.len() >= 2 {
            // For custom-N-M format, vCPUs is after "custom"
            if let Some(idx) = parts.iter().position(|&p| p == "custom") {
                if idx + 1 < parts.len() && parts[idx + 1].parse::<u32>().is_ok() {
                    return parts[idx + 1].to_string();
                }
            }
        }
    }

    // Handle shared-core machine types
    match machine_type {
        "f1-micro" => return "0.2".to_string(),
        "g1-small" => return "0.5".to_string(),
        "e2-micro" => return "0.25".to_string(),
        "e2-small" => return "0.5".to_string(),
        "e2-medium" => return "1".to_string(),
        _ => {},
    }

    // Standard format: family-type-N (e.g., n1-standard-4, c2-standard-60)
    let parts: Vec<&str> = machine_type.split('-').collect();
    if parts.len() >= 3 {
        if let Ok(vcpus) = parts[parts.len() - 1].parse::<u32>() {
            return vcpus.to_string();
        }
    }

    // If we can't determine, return "-"
    "-".to_string()
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
