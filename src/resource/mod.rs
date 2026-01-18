//! Resource abstraction layer
//!
//! This module provides a data-driven approach to managing GCP resources.
//! Resource definitions are loaded from JSON files at compile time, allowing
//! new resource types to be added without code changes.
//!
//! # Architecture
//!
//! - [`registry`] - Loads and caches resource definitions from embedded JSON
//! - [`fetcher`] - Fetches resources from GCP APIs with pagination support
//! - [`sdk_dispatch`] - Maps abstract SDK method names to concrete REST API calls
//!
//! # Resource Definitions
//!
//! Resources are defined in JSON files under `src/resources/`:
//! - `compute.json` - Compute Engine resources (VMs, disks, networks)
//! - `storage.json` - Cloud Storage resources (buckets, objects)
//! - `gke.json` - GKE resources (clusters, node pools)
//!
//! # Example
//!
//! ```ignore
//! use crate::resource::{get_resource, fetch_resources};
//! use crate::gcp::client::GcpClient;
//!
//! async fn list_vms(client: &GcpClient) -> anyhow::Result<Vec<serde_json::Value>> {
//!     let resource = get_resource("compute-instances").unwrap();
//!     fetch_resources(client, &resource, None).await
//! }
//! ```

mod fetcher;
mod registry;
pub mod sdk_dispatch;

#[allow(unused_imports)]
pub use fetcher::{
    extract_json_value, fetch_multiple_resources, fetch_resources, fetch_resources_concurrent,
    fetch_resources_paginated, ResourceFilter,
};
pub use registry::*;
pub use sdk_dispatch::execute_action;
