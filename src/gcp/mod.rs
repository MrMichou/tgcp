//! GCP API interaction module
//!
//! This module provides the core functionality for interacting with Google Cloud Platform
//! APIs, including authentication, HTTP client, and project management.
//!
//! # Module Structure
//!
//! - [`auth`] - GCP authentication using Application Default Credentials
//! - [`client`] - Main GCP client for making API requests
//! - [`http`] - HTTP utilities for REST API calls
//! - [`projects`] - Project listing and management
//!
//! # Example
//!
//! ```ignore
//! use crate::gcp::client::GcpClient;
//!
//! async fn example() -> anyhow::Result<()> {
//!     let client = GcpClient::new("my-project", "us-central1-a").await?;
//!     let instances = client.get(&client.compute_zonal_url("instances")).await?;
//!     Ok(())
//! }
//! ```

pub mod auth;
pub mod client;
pub mod http;
pub mod projects;
