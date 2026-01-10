//! GCP Projects
//!
//! Functions for listing and managing GCP projects.

use super::client::GcpClient;
use anyhow::Result;
use serde_json::Value;

/// Project information
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Project {
    pub project_id: String,
    pub name: String,
    pub project_number: String,
    pub lifecycle_state: String,
}

impl From<&Value> for Project {
    fn from(value: &Value) -> Self {
        Self {
            project_id: value
                .get("projectId")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .to_string(),
            name: value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .to_string(),
            project_number: value
                .get("projectNumber")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .to_string(),
            lifecycle_state: value
                .get("lifecycleState")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .to_string(),
        }
    }
}

/// List all accessible GCP projects
pub async fn list_projects(client: &GcpClient) -> Result<Vec<Project>> {
    let url = client.resourcemanager_url("projects");
    let response = client.get(&url).await?;

    let projects = response
        .get("projects")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|p| {
                    // Only include active projects
                    p.get("lifecycleState")
                        .and_then(|v| v.as_str())
                        .map(|s| s == "ACTIVE")
                        .unwrap_or(false)
                })
                .map(Project::from)
                .collect()
        })
        .unwrap_or_default();

    Ok(projects)
}

/// Get project IDs as a simple list
pub async fn list_project_ids(client: &GcpClient) -> Result<Vec<String>> {
    let projects = list_projects(client).await?;
    Ok(projects.into_iter().map(|p| p.project_id).collect())
}
