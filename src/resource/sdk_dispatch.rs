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
        "billing" => invoke_billing(method, client, params).await,
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
    tracing::info!(
        "execute_action: service={}, method={}, resource={}",
        service,
        method,
        resource_id
    );

    match service {
        "compute" => execute_compute_action(method, client, resource_id, params).await,
        "storage" => execute_storage_action(method, client, resource_id, params).await,
        "container" => execute_container_action(method, client, resource_id, params).await,
        "billing" => execute_billing_action(method, client, resource_id, params).await,
        _ => Err(anyhow::anyhow!("Unknown service: {}", service)),
    }
}

/// Describe a single resource
#[allow(dead_code)]
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
        .as_deref()
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
        },
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
        },
        "list_networks" => {
            let url = client.compute_global_url("networks");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_subnetworks" => {
            let url = client.compute_regional_url("subnetworks");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_firewalls" => {
            let url = client.compute_global_url("firewalls");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "get_instance" => {
            let name = get_param_str(params, "name")?;
            let url = client.compute_zonal_url(&format!("instances/{}", name));
            client.get(&url).await
        },
        // CDN / Load Balancing resources
        "list_backend_services" => {
            let url = client.compute_global_url("backendServices");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_backend_buckets" => {
            let url = client.compute_global_url("backendBuckets");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_url_maps" => {
            let url = client.compute_global_url("urlMaps");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_target_http_proxies" => {
            let url = client.compute_global_url("targetHttpProxies");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_target_https_proxies" => {
            let url = client.compute_global_url("targetHttpsProxies");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_global_forwarding_rules" => {
            let url = client.compute_global_url("globalForwardingRules");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_ssl_certificates" => {
            let url = client.compute_global_url("sslCertificates");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        // Load Balancing resources
        "list_health_checks" => {
            let url = client.compute_global_url("healthChecks");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_target_pools" => {
            let url = client.compute_regional_url("targetPools");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_target_tcp_proxies" => {
            let url = client.compute_global_url("targetTcpProxies");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_target_ssl_proxies" => {
            let url = client.compute_global_url("targetSslProxies");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_target_grpc_proxies" => {
            let url = client.compute_global_url("targetGrpcProxies");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_ssl_policies" => {
            let url = client.compute_global_url("sslPolicies");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_security_policies" => {
            let url = client.compute_global_url("securityPolicies");
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_network_endpoint_groups" => {
            if client.zone == "all" {
                let url = client.compute_aggregated_url("networkEndpointGroups");
                let url = add_query_params(&url, params);
                let response = client.get(&url).await?;
                Ok(flatten_aggregated_response(response))
            } else {
                let url = client.compute_zonal_url("networkEndpointGroups");
                let url = add_query_params(&url, params);
                client.get(&url).await
            }
        },
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
        },
        "stop_instance" => {
            let url = client.compute_zonal_url(&format!("instances/{}/stop", resource_id));
            client.post(&url, None).await
        },
        "reset_instance" => {
            let url = client.compute_zonal_url(&format!("instances/{}/reset", resource_id));
            client.post(&url, None).await
        },
        "delete_instance" => {
            let url = client.compute_zonal_url(&format!("instances/{}", resource_id));
            client.delete(&url).await
        },
        "delete_disk" => {
            let url = client.compute_zonal_url(&format!("disks/{}", resource_id));
            client.delete(&url).await
        },
        "delete_firewall" => {
            let url = client.compute_global_url(&format!("firewalls/{}", resource_id));
            client.delete(&url).await
        },
        // CDN / Load Balancing delete actions
        "delete_backend_service" => {
            let url = client.compute_global_url(&format!("backendServices/{}", resource_id));
            client.delete(&url).await
        },
        "delete_backend_bucket" => {
            let url = client.compute_global_url(&format!("backendBuckets/{}", resource_id));
            client.delete(&url).await
        },
        "delete_url_map" => {
            let url = client.compute_global_url(&format!("urlMaps/{}", resource_id));
            client.delete(&url).await
        },
        "delete_target_http_proxy" => {
            let url = client.compute_global_url(&format!("targetHttpProxies/{}", resource_id));
            client.delete(&url).await
        },
        "delete_target_https_proxy" => {
            let url = client.compute_global_url(&format!("targetHttpsProxies/{}", resource_id));
            client.delete(&url).await
        },
        "delete_global_forwarding_rule" => {
            let url = client.compute_global_url(&format!("globalForwardingRules/{}", resource_id));
            client.delete(&url).await
        },
        "delete_ssl_certificate" => {
            let url = client.compute_global_url(&format!("sslCertificates/{}", resource_id));
            client.delete(&url).await
        },
        // Load Balancing delete actions
        "delete_health_check" => {
            let url = client.compute_global_url(&format!("healthChecks/{}", resource_id));
            client.delete(&url).await
        },
        "delete_target_pool" => {
            let url = client.compute_regional_url(&format!("targetPools/{}", resource_id));
            client.delete(&url).await
        },
        "delete_target_tcp_proxy" => {
            let url = client.compute_global_url(&format!("targetTcpProxies/{}", resource_id));
            client.delete(&url).await
        },
        "delete_target_ssl_proxy" => {
            let url = client.compute_global_url(&format!("targetSslProxies/{}", resource_id));
            client.delete(&url).await
        },
        "delete_target_grpc_proxy" => {
            let url = client.compute_global_url(&format!("targetGrpcProxies/{}", resource_id));
            client.delete(&url).await
        },
        "delete_ssl_policy" => {
            let url = client.compute_global_url(&format!("sslPolicies/{}", resource_id));
            client.delete(&url).await
        },
        "delete_security_policy" => {
            let url = client.compute_global_url(&format!("securityPolicies/{}", resource_id));
            client.delete(&url).await
        },
        "delete_network_endpoint_group" => {
            let url = client.compute_zonal_url(&format!("networkEndpointGroups/{}", resource_id));
            client.delete(&url).await
        },
        _ => Err(anyhow::anyhow!("Unknown compute action: {}", method)),
    }
}

// =============================================================================
// Cloud Storage
// =============================================================================

async fn invoke_storage(method: &str, client: &GcpClient, params: &Value) -> Result<Value> {
    match method {
        "list_buckets" => {
            let url = format!("{}?project={}", client.storage_url("b"), client.project_id);
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
        "list_objects" => {
            let bucket = get_param_str(params, "bucket")?;
            let url = client.storage_objects_url(&bucket);
            let url = add_query_params(&url, params);
            client.get(&url).await
        },
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
        },
        "delete_object" => {
            let bucket = get_param_str(params, "bucket")?;
            let url = format!(
                "{}/{}",
                client.storage_objects_url(&bucket),
                urlencoding::encode(resource_id)
            );
            client.delete(&url).await
        },
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
        },
        "list_nodepools" => {
            let cluster = get_param_str(params, "cluster")?;
            let location =
                get_param_str_opt(params, "location").unwrap_or_else(|| client.zone.clone());
            let url = client
                .container_location_url(&location, &format!("clusters/{}/nodePools", cluster));
            client.get(&url).await
        },
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
// Cloud Billing
// =============================================================================

async fn invoke_billing(method: &str, client: &GcpClient, params: &Value) -> Result<Value> {
    match method {
        "list_billing_accounts" => {
            // List all billing accounts accessible to the user
            let url = client.billing_url("billingAccounts");
            let url = add_query_params(&url, params);
            let response = client.get(&url).await?;
            // Add computed fields for display
            Ok(enrich_billing_accounts(response))
        },
        "list_budgets" => {
            // List budgets for a billing account
            let billing_account = get_param_str(params, "billingAccount")?;
            let url = client.billing_budgets_url(&billing_account, "budgets");
            let url = add_query_params(&url, params);
            let response = client.get(&url).await?;
            // Add computed fields for display
            Ok(enrich_budgets(response))
        },
        "get_project_billing_info" => {
            // Get billing info for the current project
            let url = client.billing_url(&format!("projects/{}/billingInfo", client.project_id));
            let response = client.get(&url).await?;
            // Wrap in array for consistent handling
            Ok(enrich_project_billing_info(response))
        },
        "list_services" => {
            // List all GCP services with pricing info
            let url = client.billing_url("services");
            let url = add_query_params(&url, params);
            let response = client.get(&url).await?;
            Ok(enrich_services(response))
        },
        "list_skus" => {
            // List SKUs (prices) for a service
            let parent = get_param_str(params, "parent")?;
            let url = client.billing_url(&format!("{}/skus", parent));
            let url = add_query_params(&url, params);
            let response = client.get(&url).await?;
            Ok(enrich_skus(response))
        },
        _ => Err(anyhow::anyhow!("Unknown billing method: {}", method)),
    }
}

async fn execute_billing_action(
    method: &str,
    _client: &GcpClient,
    _resource_id: &str,
    _params: &Value,
) -> Result<Value> {
    // Billing resources are read-only in this MVP
    Err(anyhow::anyhow!("Unknown billing action: {}", method))
}

/// Enrich billing accounts with computed display fields
fn enrich_billing_accounts(mut response: Value) -> Value {
    if let Some(accounts) = response
        .get_mut("billingAccounts")
        .and_then(|v| v.as_array_mut())
    {
        for account in accounts {
            if let Some(obj) = account.as_object_mut() {
                // name_short: "billingAccounts/XXXXX-XXXXX-XXXXX" -> "XXXXX-XXXXX-XXXXX"
                if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                    let short = name.strip_prefix("billingAccounts/").unwrap_or(name);
                    obj.insert("name_short".to_string(), Value::String(short.to_string()));
                }

                // open_display: true -> "OPEN", false -> "CLOSED"
                if let Some(open) = obj.get("open").and_then(|v| v.as_bool()) {
                    let status = if open { "OPEN" } else { "CLOSED" };
                    obj.insert(
                        "open_display".to_string(),
                        Value::String(status.to_string()),
                    );
                }

                // masterBillingAccount_short
                if let Some(master) = obj.get("masterBillingAccount").and_then(|v| v.as_str()) {
                    let short = master.strip_prefix("billingAccounts/").unwrap_or(master);
                    obj.insert(
                        "masterBillingAccount_short".to_string(),
                        Value::String(short.to_string()),
                    );
                } else {
                    obj.insert(
                        "masterBillingAccount_short".to_string(),
                        Value::String("-".to_string()),
                    );
                }
            }
        }
    }
    response
}

/// Enrich budgets with computed display fields
fn enrich_budgets(mut response: Value) -> Value {
    if let Some(budgets) = response.get_mut("budgets").and_then(|v| v.as_array_mut()) {
        for budget in budgets {
            if let Some(obj) = budget.as_object_mut() {
                // Parse budget amount
                let budget_amount = parse_budget_amount(obj.get("amount"));
                obj.insert(
                    "amount_display".to_string(),
                    Value::String(format_currency(budget_amount)),
                );

                // Parse current spend (from etag or we need to handle differently)
                // Note: The Budget API doesn't directly return current spend in the list response
                // We'll show the budget amount and threshold rules for now
                obj.insert("spent_display".to_string(), Value::String("-".to_string()));
                obj.insert(
                    "percent_display".to_string(),
                    Value::String("-".to_string()),
                );

                // Budget status based on threshold rules
                obj.insert("budget_status".to_string(), Value::String("OK".to_string()));

                // Count threshold rules
                let rules_count = obj
                    .get("thresholdRules")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.len())
                    .unwrap_or(0);
                obj.insert(
                    "thresholdRules_count".to_string(),
                    Value::String(rules_count.to_string()),
                );
            }
        }
    }
    response
}

/// Enrich project billing info with computed display fields
fn enrich_project_billing_info(response: Value) -> Value {
    let mut result = response.clone();
    if let Some(obj) = result.as_object_mut() {
        // billingAccountName_short
        if let Some(account) = obj.get("billingAccountName").and_then(|v| v.as_str()) {
            let short = account.strip_prefix("billingAccounts/").unwrap_or(account);
            obj.insert(
                "billingAccountName_short".to_string(),
                Value::String(short.to_string()),
            );
        } else {
            obj.insert(
                "billingAccountName_short".to_string(),
                Value::String("-".to_string()),
            );
        }
    }
    // Wrap single item in _self response for consistent handling
    serde_json::json!({ "_self": [result] })
}

/// Parse budget amount from the amount object
fn parse_budget_amount(amount: Option<&Value>) -> f64 {
    let Some(amount) = amount else {
        return 0.0;
    };

    // Check for specifiedAmount first
    if let Some(specified) = amount.get("specifiedAmount") {
        return parse_money(specified);
    }

    // Check for lastPeriodAmount (uses last period's spend as budget)
    if amount.get("lastPeriodAmount").is_some() {
        return -1.0; // Indicates "last period" budget
    }

    0.0
}

/// Parse a Money object (units + nanos)
fn parse_money(money: &Value) -> f64 {
    let units = money
        .get("units")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let nanos = money.get("nanos").and_then(|v| v.as_i64()).unwrap_or(0) as f64;

    units + (nanos / 1_000_000_000.0)
}

/// Format a currency value for display
fn format_currency(amount: f64) -> String {
    if amount < 0.0 {
        "Last Period".to_string()
    } else if amount >= 1_000_000.0 {
        format!("${:.1}M", amount / 1_000_000.0)
    } else if amount >= 1_000.0 {
        format!("${:.1}K", amount / 1_000.0)
    } else {
        format!("${:.2}", amount)
    }
}

/// Enrich services with computed display fields
fn enrich_services(mut response: Value) -> Value {
    if let Some(services) = response.get_mut("services").and_then(|v| v.as_array_mut()) {
        for service in services {
            if let Some(obj) = service.as_object_mut() {
                // businessEntityName_short: "businessEntities/GCP" -> "GCP"
                if let Some(entity) = obj.get("businessEntityName").and_then(|v| v.as_str()) {
                    let short = entity.strip_prefix("businessEntities/").unwrap_or(entity);
                    obj.insert(
                        "businessEntityName_short".to_string(),
                        Value::String(short.to_string()),
                    );
                }
            }
        }
    }
    response
}

/// Enrich SKUs with computed display fields (prices)
fn enrich_skus(mut response: Value) -> Value {
    if let Some(skus) = response.get_mut("skus").and_then(|v| v.as_array_mut()) {
        for sku in skus {
            if let Some(obj) = sku.as_object_mut() {
                // Extract price from pricingInfo
                let (price, unit) = extract_sku_price(obj);
                obj.insert("price_display".to_string(), Value::String(price));
                obj.insert("usage_unit".to_string(), Value::String(unit));
            }
        }
    }
    response
}

/// Extract price and unit from SKU pricing info
fn extract_sku_price(sku: &serde_json::Map<String, Value>) -> (String, String) {
    let pricing_info = sku.get("pricingInfo").and_then(|v| v.as_array());

    let Some(pricing_info) = pricing_info else {
        return ("-".to_string(), "-".to_string());
    };

    let Some(first_pricing) = pricing_info.first() else {
        return ("-".to_string(), "-".to_string());
    };

    let pricing_expr = first_pricing.get("pricingExpression");

    let Some(pricing_expr) = pricing_expr else {
        return ("-".to_string(), "-".to_string());
    };

    // Get usage unit
    let unit = pricing_expr
        .get("usageUnit")
        .and_then(|v| v.as_str())
        .unwrap_or("-")
        .to_string();

    // Get price from tiered rates
    let tiered_rates = pricing_expr.get("tieredRates").and_then(|v| v.as_array());

    let price = if let Some(rates) = tiered_rates {
        if let Some(first_rate) = rates.first() {
            if let Some(unit_price) = first_rate.get("unitPrice") {
                let amount = parse_money(unit_price);
                if amount == 0.0 {
                    "Free".to_string()
                } else if amount < 0.0001 {
                    format!("${:.6}", amount)
                } else {
                    format!("${:.4}", amount)
                }
            } else {
                "-".to_string()
            }
        } else {
            "-".to_string()
        }
    } else {
        "-".to_string()
    };

    (price, unit)
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
            },
            Value::Array(arr) => {
                for item in arr {
                    if let Value::String(s) = item {
                        query_parts.push(format!("{}={}", key, urlencoding::encode(s)));
                    }
                }
            },
            _ => {},
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
