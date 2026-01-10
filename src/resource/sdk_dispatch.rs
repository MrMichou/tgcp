//! SDK Dispatch
//!
//! Maps SDK method names to GCP REST API calls.

use crate::gcp::client::GcpClient;
use anyhow::{Context, Result};
use serde_json::Value;

/// Invoke a GCP SDK method
pub async fn invoke_sdk(
    service: &str,
    method: &str,
    client: &GcpClient,
    params: &Value,
) -> Result<Value> {
    tracing::debug!("invoke_sdk: service={}, method={}", service, method);

    match service {
        "compute" => invoke_compute(method, client, params).await,
        "storage" => invoke_storage(method, client, params).await,
        "container" => invoke_container(method, client, params).await,
        _ => Err(anyhow::anyhow!("Unknown service: {}", service)),
    }
}

/// Execute an action on a resource
pub async fn execute_action(
    service: &str,
    method: &str,
    client: &GcpClient,
    resource_id: &str,
    params: &Value,
) -> Result<Value> {
    tracing::info!("execute_action: service={}, method={}, resource={}", service, method, resource_id);

    match service {
        "compute" => execute_compute_action(method, client, resource_id, params).await,
        "storage" => execute_storage_action(method, client, resource_id, params).await,
        "container" => execute_container_action(method, client, resource_id, params).await,
        _ => Err(anyhow::anyhow!("Unknown service: {}", service)),
    }
}

/// Describe a single resource
pub async fn describe_resource(
    resource_key: &str,
    client: &GcpClient,
    resource_id: &str,
) -> Result<Value> {
    let Some(resource_def) = super::get_resource(resource_key) else {
        return Err(anyhow::anyhow!("Unknown resource: {}", resource_key));
    };

    // Build describe method name from list method
    let describe_method = resource_def
        .detail_sdk_method
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|| {
            // Derive from list method: list_instances -> get_instance
            if resource_def.sdk_method.starts_with("list_") {
                // Can't modify &str, so just use the original
                &resource_def.sdk_method
            } else {
                &resource_def.sdk_method
            }
        });

    let params = serde_json::json!({
        "name": resource_id
    });

    invoke_sdk(&resource_def.service, describe_method, client, &params).await
}

// =============================================================================
// Compute Engine
// =============================================================================

async fn invoke_compute(method: &str, client: &GcpClient, params: &Value) -> Result<Value> {
    match method {
        "list_instances" => {
            if client.zone == "all" {
                // Use aggregated API to get instances from all zones
                let url = client.compute_aggregated_url("instances");
                let url = add_query_params(&url, params);
                let response = client.get(&url).await?;
                Ok(flatten_aggregated_response(response))
            } else {
                let url = client.compute_zonal_url("instances");
                let url = add_query_params(&url, params);
                client.get(&url).await
            }
        }
        "list_disks" => {
            if client.zone == "all" {
                let url = client.compute_aggregated_url("disks");
                let url = add_query_params(&url, params);
                let response = client.get(&url).await?;
                Ok(flatten_aggregated_response(response))
            } else {
                let url = client.compute_zonal_url("disks");
                let url = add_query_params(&url, params);
                client.get(&url).await
            }
        }
        "list_networks" => {
            let url = client.compute_global_url("networks");
            let url = add_query_params(&url, params);
            client.get(&url).await
        }
        "list_subnetworks" => {
            let url = client.compute_regional_url("subnetworks");
            let url = add_query_params(&url, params);
            client.get(&url).await
        }
        "list_firewalls" => {
            let url = client.compute_global_url("firewalls");
            let url = add_query_params(&url, params);
            client.get(&url).await
        }
        "get_instance" => {
            let name = get_param_str(params, "name")?;
            let url = client.compute_zonal_url(&format!("instances/{}", name));
            client.get(&url).await
        }
        _ => Err(anyhow::anyhow!("Unknown compute method: {}", method)),
    }
}

async fn execute_compute_action(
    method: &str,
    client: &GcpClient,
    resource_id: &str,
    _params: &Value,
) -> Result<Value> {
    match method {
        "start_instance" => {
            let url = client.compute_zonal_url(&format!("instances/{}/start", resource_id));
            client.post(&url, None).await
        }
        "stop_instance" => {
            let url = client.compute_zonal_url(&format!("instances/{}/stop", resource_id));
            client.post(&url, None).await
        }
        "reset_instance" => {
            let url = client.compute_zonal_url(&format!("instances/{}/reset", resource_id));
            client.post(&url, None).await
        }
        "delete_instance" => {
            let url = client.compute_zonal_url(&format!("instances/{}", resource_id));
            client.delete(&url).await
        }
        "delete_disk" => {
            let url = client.compute_zonal_url(&format!("disks/{}", resource_id));
            client.delete(&url).await
        }
        "delete_firewall" => {
            let url = client.compute_global_url(&format!("firewalls/{}", resource_id));
            client.delete(&url).await
        }
        _ => Err(anyhow::anyhow!("Unknown compute action: {}", method)),
    }
}

// =============================================================================
// Cloud Storage
// =============================================================================

async fn invoke_storage(method: &str, client: &GcpClient, params: &Value) -> Result<Value> {
    match method {
        "list_buckets" => {
            let url = format!(
                "{}?project={}",
                client.storage_url("b"),
                client.project_id
            );
            let url = add_query_params(&url, params);
            client.get(&url).await
        }
        "list_objects" => {
            let bucket = get_param_str(params, "bucket")?;
            let url = client.storage_objects_url(&bucket);
            let url = add_query_params(&url, params);
            client.get(&url).await
        }
        _ => Err(anyhow::anyhow!("Unknown storage method: {}", method)),
    }
}

async fn execute_storage_action(
    method: &str,
    client: &GcpClient,
    resource_id: &str,
    params: &Value,
) -> Result<Value> {
    match method {
        "delete_bucket" => {
            let url = client.storage_bucket_url(resource_id);
            client.delete(&url).await
        }
        "delete_object" => {
            let bucket = get_param_str(params, "bucket")?;
            let url = format!("{}/{}", client.storage_objects_url(&bucket), urlencoding::encode(resource_id));
            client.delete(&url).await
        }
        _ => Err(anyhow::anyhow!("Unknown storage action: {}", method)),
    }
}

// =============================================================================
// GKE (Container)
// =============================================================================

async fn invoke_container(method: &str, client: &GcpClient, params: &Value) -> Result<Value> {
    match method {
        "list_clusters" => {
            // List all clusters in all locations
            let url = client.container_location_url("-", "clusters");
            let url = add_query_params(&url, params);
            client.get(&url).await
        }
        "list_nodepools" => {
            let cluster = get_param_str(params, "cluster")?;
            let location = get_param_str_opt(params, "location")
                .unwrap_or_else(|| client.zone.clone());
            let url = client.container_location_url(&location, &format!("clusters/{}/nodePools", cluster));
            client.get(&url).await
        }
        _ => Err(anyhow::anyhow!("Unknown container method: {}", method)),
    }
}

async fn execute_container_action(
    method: &str,
    _client: &GcpClient,
    _resource_id: &str,
    _params: &Value,
) -> Result<Value> {
    Err(anyhow::anyhow!("Unknown container action: {}", method))
}

// =============================================================================
// Helpers
// =============================================================================

fn get_param_str(params: &Value, key: &str) -> Result<String> {
    params
        .get(key)
        .and_then(|v| {
            if let Value::Array(arr) = v {
                arr.first().and_then(|v| v.as_str())
            } else {
                v.as_str()
            }
        })
        .map(|s| s.to_string())
        .context(format!("Missing required parameter: {}", key))
}

fn get_param_str_opt(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(|v| {
            if let Value::Array(arr) = v {
                arr.first().and_then(|v| v.as_str())
            } else {
                v.as_str()
            }
        })
        .map(|s| s.to_string())
}

fn add_query_params(url: &str, params: &Value) -> String {
    let Value::Object(map) = params else {
        return url.to_string();
    };

    let mut query_parts: Vec<String> = Vec::new();

    for (key, value) in map {
        // Skip internal params
        if key == "bucket" || key == "cluster" || key == "location" || key == "name" {
            continue;
        }

        match value {
            Value::String(s) => {
                query_parts.push(format!("{}={}", key, urlencoding::encode(s)));
            }
            Value::Array(arr) => {
                for item in arr {
                    if let Value::String(s) = item {
                        query_parts.push(format!("{}={}", key, urlencoding::encode(s)));
                    }
                }
            }
            _ => {}
        }
    }

    if query_parts.is_empty() {
        url.to_string()
    } else if url.contains('?') {
        format!("{}&{}", url, query_parts.join("&"))
    } else {
        format!("{}?{}", url, query_parts.join("&"))
    }
}

/// Flatten an aggregated API response into a standard list response.
/// Aggregated responses have format: { "items": { "zones/us-central1-a": { "instances": [...] }, ... } }
/// We flatten to: { "items": [...all instances...] }
fn flatten_aggregated_response(response: Value) -> Value {
    let Some(items) = response.get("items").and_then(|v| v.as_object()) else {
        return serde_json::json!({ "items": [] });
    };

    let mut all_items: Vec<Value> = Vec::new();

    for (_zone_key, zone_data) in items {
        // Each zone entry may have "instances", "disks", etc.
        // Look for any array field that contains the actual resources
        if let Some(obj) = zone_data.as_object() {
            for (key, value) in obj {
                // Skip warning field and other metadata
                if key == "warning" {
                    continue;
                }
                if let Some(arr) = value.as_array() {
                    all_items.extend(arr.iter().cloned());
                }
            }
        }
    }

    serde_json::json!({ "items": all_items })
}
