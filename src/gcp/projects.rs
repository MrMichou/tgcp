//! GCP Projects
//!
//! Functions for listing and managing GCP projects.

use super::client::GcpClient;
use anyhow::Result;
use serde_json::Value;

/// Project information
#[derive(Debug, Clone)]
pub struct Project {
    pub project_id: String,
}

impl From<&Value> for Project {
    fn from(value: &Value) -> Self {
        Self {
            project_id: value
                .get("projectId")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .to_string(),
        }
    }
}

/// List all accessible GCP projects (sorted alphabetically by project_id)
///
/// Paginates through all pages using `nextPageToken` to handle large organizations.
pub async fn list_projects(client: &GcpClient) -> Result<Vec<Project>> {
    let mut all_projects: Vec<Project> = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let url = if let Some(ref token) = page_token {
            format!(
                "{}?pageToken={}",
                client.resourcemanager_url("projects"),
                token
            )
        } else {
            client.resourcemanager_url("projects")
        };

        let response = client.get(&url).await?;

        let page_projects: Vec<Project> = response
            .get("projects")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|p| {
                        p.get("lifecycleState")
                            .and_then(|v| v.as_str())
                            .map(|s| s == "ACTIVE")
                            .unwrap_or(false)
                    })
                    .map(Project::from)
                    .collect()
            })
            .unwrap_or_default();

        all_projects.extend(page_projects);

        // Check for next page
        match response.get("nextPageToken").and_then(|v| v.as_str()) {
            Some(token) if !token.is_empty() => {
                page_token = Some(token.to_string());
            },
            _ => break,
        }
    }

    // Sort alphabetically by project_id
    all_projects.sort_by(|a, b| {
        a.project_id
            .to_lowercase()
            .cmp(&b.project_id.to_lowercase())
    });

    Ok(all_projects)
}

/// Get project IDs as a simple list
pub async fn list_project_ids(client: &GcpClient) -> Result<Vec<String>> {
    let projects = list_projects(client).await?;
    Ok(projects.into_iter().map(|p| p.project_id).collect())
}
