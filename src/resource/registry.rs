//! Resource Registry - Load resource definitions from JSON
//!
//! This module loads all GCP resource definitions from embedded JSON files
//! and provides lookup functions for the rest of the application.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Embedded resource JSON files (compiled into the binary)
const RESOURCE_FILES: &[&str] = &[
    include_str!("../resources/common.json"),
    include_str!("../resources/compute.json"),
    include_str!("../resources/storage.json"),
    include_str!("../resources/gke.json"),
];

/// Color definition from JSON
#[derive(Debug, Clone, Deserialize)]
pub struct ColorDef {
    pub value: String,
    pub color: [u8; 3],
}

/// Column definition from JSON
#[derive(Debug, Clone, Deserialize)]
pub struct ColumnDef {
    pub header: String,
    pub json_path: String,
    pub width: u16,
    #[serde(default)]
    pub color_map: Option<String>,
}

/// Sub-resource definition from JSON
#[derive(Debug, Clone, Deserialize)]
pub struct SubResourceDef {
    pub resource_key: String,
    pub display_name: String,
    pub shortcut: String,
    pub parent_id_field: String,
    pub filter_param: String,
}

/// Confirmation config for actions
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfirmConfig {
    /// Message to show in confirmation dialog
    #[serde(default)]
    pub message: Option<String>,
    /// If true, default selection is Yes; if false, default is No
    #[serde(default)]
    pub default_yes: bool,
    /// If true, action is destructive (shown in red)
    #[serde(default)]
    pub destructive: bool,
}

/// Action definition from JSON
#[derive(Debug, Clone, Deserialize)]
pub struct ActionDef {
    /// Key identifier for the action
    #[allow(dead_code)]
    pub key: String,
    pub display_name: String,
    #[serde(default)]
    pub shortcut: Option<String>,
    pub sdk_method: String,
    /// Parameter name for the resource ID
    #[serde(default)]
    #[allow(dead_code)]
    pub id_param: Option<String>,
    /// Legacy field - use `confirm` instead
    #[serde(default)]
    pub needs_confirm: bool,
    /// Confirmation configuration
    #[serde(default)]
    pub confirm: Option<ConfirmConfig>,
}

impl ActionDef {
    /// Check if this action requires confirmation
    pub fn requires_confirm(&self) -> bool {
        self.confirm.is_some() || self.needs_confirm
    }

    /// Get the confirmation config (with defaults)
    pub fn get_confirm_config(&self) -> Option<ConfirmConfig> {
        if let Some(ref config) = self.confirm {
            Some(config.clone())
        } else if self.needs_confirm {
            Some(ConfirmConfig {
                message: Some(self.display_name.clone()),
                default_yes: false,
                destructive: false,
            })
        } else {
            None
        }
    }
}

/// Resource definition from JSON
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ResourceDef {
    pub display_name: String,
    pub service: String,
    pub sdk_method: String,
    #[serde(default)]
    pub sdk_method_params: Value,
    pub response_path: String,
    pub id_field: String,
    pub name_field: String,
    #[serde(default)]
    pub is_global: bool,
    #[serde(default)]
    pub is_regional: bool,
    pub columns: Vec<ColumnDef>,
    #[serde(default)]
    pub sub_resources: Vec<SubResourceDef>,
    #[serde(default)]
    pub actions: Vec<ActionDef>,
    /// SDK method to call when fetching details for a single resource
    #[serde(default)]
    pub detail_sdk_method: Option<String>,
    /// Parameters for detail_sdk_method
    #[serde(default)]
    pub detail_sdk_method_params: Value,
}

/// Root structure of resources/*.json
#[derive(Debug, Clone, Deserialize)]
pub struct ResourceConfig {
    #[serde(default)]
    pub color_maps: HashMap<String, Vec<ColorDef>>,
    #[serde(default)]
    pub resources: HashMap<String, ResourceDef>,
}

/// Global registry loaded from JSON
static REGISTRY: OnceLock<ResourceConfig> = OnceLock::new();

/// Get the resource registry (loads from embedded JSON on first access)
pub fn get_registry() -> &'static ResourceConfig {
    REGISTRY.get_or_init(|| {
        let mut final_config = ResourceConfig {
            color_maps: HashMap::new(),
            resources: HashMap::new(),
        };

        for content in RESOURCE_FILES {
            let partial: ResourceConfig = serde_json::from_str(content)
                .unwrap_or_else(|e| panic!("Failed to parse embedded resource JSON: {}", e));
            final_config.color_maps.extend(partial.color_maps);
            final_config.resources.extend(partial.resources);
        }

        final_config
    })
}

/// Get a resource definition by key
pub fn get_resource(key: &str) -> Option<&'static ResourceDef> {
    get_registry().resources.get(key)
}

/// Get all resource keys (for autocomplete)
pub fn get_all_resource_keys() -> Vec<&'static str> {
    get_registry()
        .resources
        .keys()
        .map(|s| s.as_str())
        .collect()
}

/// Get a color map by name
pub fn get_color_map(name: &str) -> Option<&'static Vec<ColorDef>> {
    get_registry().color_maps.get(name)
}

/// Get color for a value based on color map name
pub fn get_color_for_value(color_map_name: &str, value: &str) -> Option<[u8; 3]> {
    get_color_map(color_map_name)?
        .iter()
        .find(|c| c.value == value)
        .map(|c| c.color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_loads_successfully() {
        let registry = get_registry();
        assert!(
            !registry.resources.is_empty(),
            "Registry should have resources"
        );
    }

    #[test]
    fn test_compute_instances_resource_exists() {
        let resource = get_resource("compute-instances");
        assert!(
            resource.is_some(),
            "Compute instances resource should exist"
        );

        let resource = resource.unwrap();
        assert_eq!(resource.display_name, "VM Instances");
        assert_eq!(resource.service, "compute");
    }

    #[test]
    fn test_get_all_resource_keys() {
        let keys = get_all_resource_keys();
        assert!(!keys.is_empty(), "Should have resource types");
        assert!(
            keys.contains(&"compute-instances"),
            "Should contain compute-instances"
        );
    }

    #[test]
    fn test_common_color_maps_exist() {
        let state_map = get_color_map("status");
        assert!(state_map.is_some(), "Status color map should exist");
    }
}
