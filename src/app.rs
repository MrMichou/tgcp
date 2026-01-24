//! Application State
//!
//! Central application state management for tgcp.

use crate::config::Config;
use crate::gcp::client::{GcpClient, OperationStatus};
use crate::notification::{DetailLevel, NotificationManager, OperationType, SoundConfig};
use crate::resource::{
    enrich_with_metrics, extract_json_value, fetch_resources_paginated, get_all_resource_keys,
    get_resource, MetricsHistory, ResourceDef, ResourceFilter,
};
use crate::theme::ThemeManager;
use anyhow::Result;
use crossterm::event::KeyCode;
use serde_json::Value;
use std::collections::HashSet;
use std::ops::Range;
use std::time::Duration;
use uuid::Uuid;

// =========================================================================
// Configuration Constants
// =========================================================================

/// Default viewport height (will be updated during render based on terminal size)
const DEFAULT_VIEWPORT_HEIGHT: usize = 20;

/// Application modes
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,        // Viewing list
    Command,       // : command input
    Help,          // ? help popup
    Confirm,       // Confirmation dialog
    Warning,       // Warning/info dialog (OK only)
    Projects,      // Project selection
    Zones,         // Zone selection
    Describe,      // Viewing JSON details of selected item
    Notifications, // Notifications history panel
    ColumnConfig,  // Column visibility configuration
}

/// State for column configuration overlay
#[derive(Debug, Clone)]
pub struct ColumnConfigState {
    /// List of columns with visibility status
    pub columns: Vec<ColumnConfigItem>,
    /// Currently selected column index
    pub selected: usize,
}

/// Single column configuration item
#[derive(Debug, Clone)]
pub struct ColumnConfigItem {
    /// Column header name
    pub header: String,
    /// Whether the column is visible
    pub visible: bool,
}

/// Pending action that requires confirmation
#[derive(Debug, Clone)]
pub struct PendingAction {
    pub service: String,
    pub sdk_method: String,
    pub resource_id: String,
    pub message: String,
    pub destructive: bool,
    pub selected_yes: bool,
}

/// Parent context for hierarchical navigation
#[derive(Debug, Clone)]
pub struct ParentContext {
    pub resource_key: String,
    pub item: Value,
    pub display_name: String,
}

/// Pagination state
#[derive(Debug, Clone, Default)]
pub struct PaginationState {
    pub next_token: Option<String>,
    pub token_stack: Vec<Option<String>>,
    pub current_page: usize,
    pub has_more: bool,
}

/// Main application state
pub struct App {
    // GCP Client
    pub client: GcpClient,

    // Current resource being viewed
    pub current_resource_key: String,

    // Dynamic data storage (JSON)
    pub items: Vec<Value>,
    pub filtered_items: Vec<Value>,

    // Navigation state
    pub selected: usize,
    pub mode: Mode,
    pub filter_text: String,
    pub filter_active: bool,

    // Hierarchical navigation
    pub parent_context: Option<ParentContext>,
    pub navigation_stack: Vec<ParentContext>,

    // Command input
    pub command_text: String,
    pub command_suggestions: Vec<String>,
    pub command_suggestion_selected: usize,
    pub command_preview: Option<String>,

    // Project/Zone
    pub project: String,
    pub zone: String,
    pub available_projects: Vec<String>,
    pub available_zones: Vec<String>,
    pub projects_selected: usize,
    pub zones_selected: usize,
    // Search in selectors
    pub projects_search_text: String,
    pub projects_filtered: Vec<String>,
    pub zones_search_text: String,
    pub zones_filtered: Vec<String>,

    // Sorting
    pub sort_column: Option<usize>,
    pub sort_ascending: bool,

    // Confirmation
    pub pending_action: Option<PendingAction>,

    // UI state
    pub loading: bool,
    pub error_message: Option<String>,
    pub describe_scroll: usize,
    pub describe_data: Option<Value>,

    // Auto-refresh
    pub last_refresh: std::time::Instant,

    // Persistent configuration
    pub config: Config,

    // Key press tracking
    pub last_key_press: Option<(KeyCode, std::time::Instant)>,

    // Read-only mode
    pub readonly: bool,

    // Warning message
    pub warning_message: Option<String>,

    // Pagination
    pub pagination: PaginationState,

    // Theme
    pub theme_manager: ThemeManager,

    // Notifications
    pub notification_manager: NotificationManager,
    pub notifications_selected: usize,

    // Virtual scrolling
    pub viewport_height: usize,
    pub scroll_offset: usize,

    // Multi-selection (bulk operations)
    pub selected_indices: HashSet<usize>,
    pub visual_mode: bool,

    // Metrics history for trend calculation
    pub metrics_history: MetricsHistory,

    // Column configuration state
    pub column_config_state: Option<ColumnConfigState>,
}

impl App {
    /// Create App from pre-initialized components
    #[allow(clippy::too_many_arguments)]
    pub fn from_initialized(
        client: GcpClient,
        project: String,
        zone: String,
        available_projects: Vec<String>,
        available_zones: Vec<String>,
        initial_items: Vec<Value>,
        config: Config,
        readonly: bool,
    ) -> Self {
        let filtered_items = initial_items.clone();

        // Initialize theme manager and apply project-specific theme
        let mut theme_manager = ThemeManager::load();

        // Apply theme from config or project-specific setting
        let theme_name = config.effective_theme(&project);
        theme_manager.set_theme(&theme_name);

        // Initialize notification manager with config settings
        let mut notification_manager = NotificationManager::new();
        notification_manager.detail_level =
            DetailLevel::from_str(&config.notifications.detail_level);
        notification_manager.toast_duration =
            Duration::from_secs(config.notifications.toast_duration_secs);
        notification_manager.max_history = config.notifications.max_history;
        notification_manager.poll_interval =
            Duration::from_millis(config.notifications.poll_interval_ms);
        notification_manager.auto_poll = config.notifications.auto_poll;
        notification_manager.sound_config = SoundConfig::from_str(&config.notifications.sound);

        Self {
            client,
            current_resource_key: "compute-instances".to_string(),
            items: initial_items,
            filtered_items,
            selected: 0,
            mode: Mode::Normal,
            filter_text: String::new(),
            filter_active: false,
            parent_context: None,
            navigation_stack: Vec::new(),
            command_text: String::new(),
            command_suggestions: Vec::new(),
            command_suggestion_selected: 0,
            command_preview: None,
            project,
            zone,
            available_projects: available_projects.clone(),
            available_zones: available_zones.clone(),
            projects_selected: 0,
            zones_selected: 0,
            projects_search_text: String::new(),
            projects_filtered: available_projects,
            zones_search_text: String::new(),
            zones_filtered: available_zones,
            sort_column: None,
            sort_ascending: true,
            pending_action: None,
            loading: false,
            error_message: None,
            describe_scroll: 0,
            describe_data: None,
            last_refresh: std::time::Instant::now(),
            config,
            last_key_press: None,
            readonly,
            warning_message: None,
            pagination: PaginationState::default(),
            theme_manager,
            notification_manager,
            notifications_selected: 0,
            // Virtual scrolling
            viewport_height: DEFAULT_VIEWPORT_HEIGHT,
            scroll_offset: 0,
            // Multi-selection
            selected_indices: HashSet::new(),
            visual_mode: false,
            // Metrics history
            metrics_history: MetricsHistory::default(),
            // Column configuration
            column_config_state: None,
        }
    }

    /// Check if auto-refresh is needed (disabled)
    pub fn needs_refresh(&self) -> bool {
        false
    }

    /// Reset refresh timer
    pub fn mark_refreshed(&mut self) {
        self.last_refresh = std::time::Instant::now();
    }

    // =========================================================================
    // Resource Definition Access
    // =========================================================================

    pub fn current_resource(&self) -> Option<&'static ResourceDef> {
        get_resource(&self.current_resource_key)
    }

    pub fn get_available_commands(&self) -> Vec<String> {
        let mut commands: Vec<String> = get_all_resource_keys()
            .iter()
            .map(|s| s.to_string())
            .collect();

        // Add built-in commands
        commands.push("projects".to_string());
        commands.push("zones".to_string());
        commands.push("notifications".to_string());
        commands.push("notifications clear".to_string());

        // Add theme commands
        commands.push("theme".to_string());
        for theme in ThemeManager::list_available() {
            commands.push(format!("theme {}", theme));
        }

        // Add aliases
        for alias in self.config.aliases.keys() {
            if !commands.contains(alias) {
                commands.push(alias.clone());
            }
        }

        commands.sort();
        commands
    }

    // =========================================================================
    // Data Fetching
    // =========================================================================

    pub async fn refresh_current(&mut self) -> Result<()> {
        self.fetch_page(self.pagination.next_token.clone()).await
    }

    async fn fetch_page(&mut self, page_token: Option<String>) -> Result<()> {
        if self.current_resource().is_none() {
            self.error_message = Some(format!("Unknown resource: {}", self.current_resource_key));
            return Ok(());
        }

        self.loading = true;
        self.error_message = None;

        let filters = self.build_filters_from_context();

        match fetch_resources_paginated(
            &self.current_resource_key,
            &self.client,
            &filters,
            page_token.as_deref(),
        )
        .await
        {
            Ok(result) => {
                let prev_selected = self.selected;
                self.items = result.items;

                // Enrich VM instances with monitoring metrics
                if self.current_resource_key == "compute-instances" {
                    if let Err(e) = enrich_with_metrics(
                        &mut self.items,
                        &self.client,
                        &mut self.metrics_history,
                    )
                    .await
                    {
                        tracing::debug!("Failed to enrich with metrics: {}", e);
                    }
                }

                self.apply_filter();

                self.pagination.has_more = result.next_token.is_some();
                self.pagination.next_token = result.next_token;

                if prev_selected < self.filtered_items.len() {
                    self.selected = prev_selected;
                } else {
                    self.selected = 0;
                }
            },
            Err(e) => {
                self.error_message = Some(crate::gcp::client::format_gcp_error(&e));
                self.items.clear();
                self.filtered_items.clear();
                self.selected = 0;
                self.pagination = PaginationState::default();
            },
        }

        self.loading = false;
        self.mark_refreshed();
        Ok(())
    }

    pub async fn next_page(&mut self) -> Result<()> {
        if !self.pagination.has_more {
            return Ok(());
        }

        let current_token = self.pagination.next_token.clone();
        self.pagination.token_stack.push(current_token.clone());
        self.pagination.current_page += 1;

        self.fetch_page(current_token).await
    }

    pub async fn prev_page(&mut self) -> Result<()> {
        if self.pagination.current_page <= 1 {
            return Ok(());
        }

        self.pagination.token_stack.pop();
        let prev_token = self.pagination.token_stack.pop().flatten();
        self.pagination.current_page -= 1;

        self.fetch_page(prev_token).await
    }

    pub fn reset_pagination(&mut self) {
        self.pagination = PaginationState::default();
    }

    fn build_filters_from_context(&self) -> Vec<ResourceFilter> {
        let Some(parent) = &self.parent_context else {
            return Vec::new();
        };

        if let Some(parent_resource) = get_resource(&parent.resource_key) {
            for sub in &parent_resource.sub_resources {
                if sub.resource_key == self.current_resource_key {
                    let parent_id = extract_json_value(&parent.item, &sub.parent_id_field);
                    if parent_id != "-" {
                        return vec![ResourceFilter::new(&sub.filter_param, vec![parent_id])];
                    }
                }
            }
        }

        Vec::new()
    }

    // =========================================================================
    // Filtering
    // =========================================================================

    // TODO: Performance optimization opportunity
    // Currently clones all items into filtered_items. For large datasets, consider:
    // 1. Using Vec<usize> indices instead of cloning items
    // 2. Using Cow<[Value]> for copy-on-write semantics
    // This would require updating all 40+ usages of filtered_items
    pub fn apply_filter(&mut self) {
        let filter = self.filter_text.to_lowercase();

        if filter.is_empty() {
            self.filtered_items = self.items.clone();
        } else {
            let resource = self.current_resource();
            self.filtered_items = self
                .items
                .iter()
                .filter(|item| {
                    if let Some(res) = resource {
                        // Search ALL columns, not just name/id
                        res.columns.iter().any(|col| {
                            let value = extract_json_value(item, &col.json_path).to_lowercase();
                            value.contains(&filter)
                        })
                    } else {
                        item.to_string().to_lowercase().contains(&filter)
                    }
                })
                .cloned()
                .collect();
        }

        if self.selected >= self.filtered_items.len() && !self.filtered_items.is_empty() {
            self.selected = self.filtered_items.len() - 1;
        }

        // Clear selection when filter changes (indices become invalid)
        self.selected_indices.clear();
        self.scroll_offset = 0;

        // Re-apply sort if active
        if self.sort_column.is_some() {
            self.apply_sort();
        }
    }

    pub fn clear_filter(&mut self) {
        self.filter_text.clear();
        self.filter_active = false;
        self.apply_filter();
    }

    // =========================================================================
    // Navigation
    // =========================================================================

    pub fn selected_item(&self) -> Option<&Value> {
        self.filtered_items.get(self.selected)
    }

    pub fn selected_item_json(&self) -> Option<String> {
        if let Some(ref data) = self.describe_data {
            return Some(serde_json::to_string_pretty(data).unwrap_or_default());
        }
        self.selected_item()
            .map(|item| serde_json::to_string_pretty(item).unwrap_or_default())
    }

    pub fn describe_line_count(&self) -> usize {
        self.selected_item_json()
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }

    pub fn describe_scroll_to_bottom(&mut self, visible_lines: usize) {
        let total = self.describe_line_count();
        self.describe_scroll = total.saturating_sub(visible_lines);
    }

    pub fn next(&mut self) {
        match self.mode {
            Mode::Projects => {
                if !self.projects_filtered.is_empty() {
                    self.projects_selected =
                        (self.projects_selected + 1).min(self.projects_filtered.len() - 1);
                }
            },
            Mode::Zones => {
                if !self.zones_filtered.is_empty() {
                    self.zones_selected =
                        (self.zones_selected + 1).min(self.zones_filtered.len() - 1);
                }
            },
            _ => {
                if !self.filtered_items.is_empty() {
                    self.selected = (self.selected + 1).min(self.filtered_items.len() - 1);
                }
            },
        }
    }

    pub fn previous(&mut self) {
        match self.mode {
            Mode::Projects => {
                self.projects_selected = self.projects_selected.saturating_sub(1);
            },
            Mode::Zones => {
                self.zones_selected = self.zones_selected.saturating_sub(1);
            },
            _ => {
                self.selected = self.selected.saturating_sub(1);
            },
        }
    }

    pub fn go_to_top(&mut self) {
        match self.mode {
            Mode::Projects => self.projects_selected = 0,
            Mode::Zones => self.zones_selected = 0,
            _ => self.selected = 0,
        }
    }

    pub fn go_to_bottom(&mut self) {
        match self.mode {
            Mode::Projects => {
                if !self.projects_filtered.is_empty() {
                    self.projects_selected = self.projects_filtered.len() - 1;
                }
            },
            Mode::Zones => {
                if !self.zones_filtered.is_empty() {
                    self.zones_selected = self.zones_filtered.len() - 1;
                }
            },
            _ => {
                if !self.filtered_items.is_empty() {
                    self.selected = self.filtered_items.len() - 1;
                }
            },
        }
    }

    pub fn page_down(&mut self, page_size: usize) {
        match self.mode {
            Mode::Projects => {
                if !self.projects_filtered.is_empty() {
                    self.projects_selected =
                        (self.projects_selected + page_size).min(self.projects_filtered.len() - 1);
                }
            },
            Mode::Zones => {
                if !self.zones_filtered.is_empty() {
                    self.zones_selected =
                        (self.zones_selected + page_size).min(self.zones_filtered.len() - 1);
                }
            },
            _ => {
                if !self.filtered_items.is_empty() {
                    self.selected = (self.selected + page_size).min(self.filtered_items.len() - 1);
                }
            },
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        match self.mode {
            Mode::Projects => {
                self.projects_selected = self.projects_selected.saturating_sub(page_size);
            },
            Mode::Zones => {
                self.zones_selected = self.zones_selected.saturating_sub(page_size);
            },
            _ => {
                self.selected = self.selected.saturating_sub(page_size);
            },
        }
    }

    // =========================================================================
    // Mode Transitions
    // =========================================================================

    pub fn enter_command_mode(&mut self) {
        self.mode = Mode::Command;
        self.command_text.clear();
        self.command_suggestions = self.get_available_commands();
        self.command_suggestion_selected = 0;
        self.command_preview = None;
    }

    pub fn update_command_suggestions(&mut self) {
        let input = self.command_text.to_lowercase();
        let all_commands = self.get_available_commands();

        if input.is_empty() {
            self.command_suggestions = all_commands;
        } else {
            self.command_suggestions = all_commands
                .into_iter()
                .filter(|cmd| cmd.contains(&input))
                .collect();
        }

        if self.command_suggestion_selected >= self.command_suggestions.len() {
            self.command_suggestion_selected = 0;
        }

        self.update_preview();
    }

    fn update_preview(&mut self) {
        if self.command_suggestions.is_empty() {
            self.command_preview = None;
        } else {
            self.command_preview = self
                .command_suggestions
                .get(self.command_suggestion_selected)
                .cloned();
        }
    }

    pub fn next_suggestion(&mut self) {
        if !self.command_suggestions.is_empty() {
            self.command_suggestion_selected =
                (self.command_suggestion_selected + 1) % self.command_suggestions.len();
            self.update_preview();
        }
    }

    pub fn prev_suggestion(&mut self) {
        if !self.command_suggestions.is_empty() {
            if self.command_suggestion_selected == 0 {
                self.command_suggestion_selected = self.command_suggestions.len() - 1;
            } else {
                self.command_suggestion_selected -= 1;
            }
            self.update_preview();
        }
    }

    pub fn apply_suggestion(&mut self) {
        if let Some(preview) = &self.command_preview {
            self.command_text = preview.clone();
            self.update_command_suggestions();
        }
    }

    pub fn enter_help_mode(&mut self) {
        self.mode = Mode::Help;
    }

    pub async fn enter_describe_mode(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }

        self.mode = Mode::Describe;
        self.describe_scroll = 0;
        self.describe_data = None;

        // For now, just show the list data
        // TODO: Fetch detailed data via describe API
        if let Some(item) = self.selected_item().cloned() {
            self.describe_data = Some(item);
        }
    }

    pub fn enter_confirm_mode(&mut self, pending: PendingAction) {
        self.pending_action = Some(pending);
        self.mode = Mode::Confirm;
    }

    pub fn show_warning(&mut self, message: &str) {
        self.warning_message = Some(message.to_string());
        self.mode = Mode::Warning;
    }

    pub fn create_pending_action(
        &self,
        action: &crate::resource::ActionDef,
        resource_id: &str,
    ) -> Option<PendingAction> {
        let config = action.get_confirm_config()?;
        let resource_name = self
            .selected_item()
            .and_then(|item| {
                if let Some(resource_def) = self.current_resource() {
                    let name = extract_json_value(item, &resource_def.name_field);
                    if name != "-" && !name.is_empty() {
                        return Some(name);
                    }
                }
                None
            })
            .unwrap_or_else(|| resource_id.to_string());

        let message = config
            .message
            .unwrap_or_else(|| action.display_name.clone());

        Some(PendingAction {
            service: self.current_resource()?.service.clone(),
            sdk_method: action.sdk_method.clone(),
            resource_id: resource_id.to_string(),
            message: format!("{} '{}'?", message, resource_name),
            destructive: config.destructive,
            selected_yes: config.default_yes,
        })
    }

    pub fn enter_projects_mode(&mut self) {
        self.projects_search_text.clear();
        self.projects_filtered = self.available_projects.clone();
        self.projects_selected = self
            .projects_filtered
            .iter()
            .position(|p| p == &self.project)
            .unwrap_or(0);
        self.mode = Mode::Projects;
    }

    pub fn enter_zones_mode(&mut self) {
        self.zones_search_text.clear();
        self.zones_filtered = self.available_zones.clone();
        self.zones_selected = self
            .zones_filtered
            .iter()
            .position(|z| z == &self.zone)
            .unwrap_or(0);
        self.mode = Mode::Zones;
    }

    pub fn enter_notifications_mode(&mut self) {
        self.notifications_selected = 0;
        self.mode = Mode::Notifications;
    }

    pub fn enter_column_config_mode(&mut self) {
        let Some(resource) = self.current_resource() else {
            return;
        };

        // Get currently hidden columns for this resource
        let hidden = self.config.get_hidden_columns(&self.current_resource_key);

        // Build column list with visibility status
        let columns: Vec<ColumnConfigItem> = resource
            .columns
            .iter()
            .map(|col| ColumnConfigItem {
                header: col.header.clone(),
                visible: !hidden.contains(&col.header),
            })
            .collect();

        self.column_config_state = Some(ColumnConfigState {
            columns,
            selected: 0,
        });
        self.mode = Mode::ColumnConfig;
    }

    /// Toggle visibility of the currently selected column in column config mode
    pub fn toggle_column_visibility(&mut self) {
        if let Some(ref mut state) = self.column_config_state {
            // Count currently visible columns first
            let visible_count = state.columns.iter().filter(|c| c.visible).count();
            let selected_idx = state.selected;

            if let Some(col) = state.columns.get_mut(selected_idx) {
                // Only allow toggling off if more than one column is visible
                if col.visible && visible_count <= 1 {
                    // Can't hide the last visible column
                    return;
                }

                col.visible = !col.visible;
            }
        }
    }

    /// Apply column configuration and save to config
    pub fn apply_column_config(&mut self) {
        if let Some(state) = self.column_config_state.take() {
            // Collect hidden column headers
            let hidden: std::collections::HashSet<String> = state
                .columns
                .iter()
                .filter(|col| !col.visible)
                .map(|col| col.header.clone())
                .collect();

            // Save to config
            if let Err(e) = self
                .config
                .set_hidden_columns(&self.current_resource_key, hidden)
            {
                tracing::warn!("Failed to save column config: {}", e);
            }
        }
        self.mode = Mode::Normal;
    }

    /// Cancel column config without saving
    pub fn cancel_column_config(&mut self) {
        self.column_config_state = None;
        self.mode = Mode::Normal;
    }

    // =========================================================================
    // Notifications
    // =========================================================================

    /// Create a notification for an operation and return its ID
    pub fn create_operation_notification(
        &mut self,
        sdk_method: &str,
        resource_type: &str,
        resource_id: &str,
    ) -> Uuid {
        if !self.config.notifications.enabled {
            return Uuid::nil();
        }

        let op_type = OperationType::from_method(sdk_method);
        self.notification_manager.create_notification(
            op_type,
            resource_type.to_string(),
            resource_id.to_string(),
        )
    }

    /// Mark a notification as in progress with optional operation URL
    pub fn mark_notification_in_progress(&mut self, id: Uuid, operation_url: Option<String>) {
        if !id.is_nil() {
            self.notification_manager
                .mark_in_progress(id, operation_url);
        }
    }

    /// Mark a notification as successful
    pub fn mark_notification_success(&mut self, id: Uuid) {
        if !id.is_nil() {
            self.notification_manager.mark_success(id);
        }
    }

    /// Mark a notification as failed
    pub fn mark_notification_error(&mut self, id: Uuid, error: String) {
        if !id.is_nil() {
            self.notification_manager.mark_error(id, error);
        }
    }

    /// Poll pending operations and update their status
    pub async fn poll_pending_operations(&mut self) -> Result<()> {
        if !self.config.notifications.enabled || !self.notification_manager.auto_poll {
            return Ok(());
        }

        // Get operations that need polling
        let ops_to_poll = self.notification_manager.operations_to_poll();

        for (notification_id, operation_url) in ops_to_poll {
            match self.client.poll_operation(&operation_url).await {
                Ok(status) => match status {
                    OperationStatus::Done => {
                        self.notification_manager.mark_success(notification_id);
                        // Refresh current view to show updated state
                        let _ = self.refresh_current().await;
                    },
                    OperationStatus::Failed(error) => {
                        self.notification_manager.mark_error(notification_id, error);
                    },
                    OperationStatus::Running => {
                        // Still running, will poll again
                    },
                    OperationStatus::Unknown(s) => {
                        tracing::warn!("Unknown operation status: {}", s);
                    },
                },
                Err(e) => {
                    tracing::warn!("Failed to poll operation: {}", e);
                    // Don't mark as error, might be transient
                },
            }
        }

        Ok(())
    }

    /// Clear all notifications
    pub fn clear_notifications(&mut self) {
        self.notification_manager.clear();
    }

    // =========================================================================
    // Selector Filtering
    // =========================================================================

    pub fn apply_projects_filter(&mut self) {
        let filter = self.projects_search_text.to_lowercase();
        if filter.is_empty() {
            self.projects_filtered = self.available_projects.clone();
        } else {
            self.projects_filtered = self
                .available_projects
                .iter()
                .filter(|p| p.to_lowercase().contains(&filter))
                .cloned()
                .collect();
        }
        // Reset selection if out of bounds
        if self.projects_selected >= self.projects_filtered.len() {
            self.projects_selected = 0;
        }
    }

    pub fn apply_zones_filter(&mut self) {
        let filter = self.zones_search_text.to_lowercase();
        if filter.is_empty() {
            self.zones_filtered = self.available_zones.clone();
        } else {
            self.zones_filtered = self
                .available_zones
                .iter()
                .filter(|z| z.to_lowercase().contains(&filter))
                .cloned()
                .collect();
        }
        // Reset selection if out of bounds
        if self.zones_selected >= self.zones_filtered.len() {
            self.zones_selected = 0;
        }
    }

    // =========================================================================
    // Sorting
    // =========================================================================

    pub fn sort_by_column(&mut self, column_index: usize) {
        if let Some(current) = self.sort_column {
            if current == column_index {
                // Toggle direction
                self.sort_ascending = !self.sort_ascending;
            } else {
                self.sort_column = Some(column_index);
                self.sort_ascending = true;
            }
        } else {
            self.sort_column = Some(column_index);
            self.sort_ascending = true;
        }
        self.apply_sort();
    }

    pub fn apply_sort(&mut self) {
        let Some(col_idx) = self.sort_column else {
            return;
        };
        let Some(resource) = self.current_resource() else {
            return;
        };
        let Some(column) = resource.columns.get(col_idx) else {
            return;
        };

        let json_path = column.json_path.clone();
        let ascending = self.sort_ascending;

        self.filtered_items.sort_by(|a, b| {
            let val_a = extract_json_value(a, &json_path);
            let val_b = extract_json_value(b, &json_path);

            // Try numeric comparison first
            let cmp = match (val_a.parse::<f64>(), val_b.parse::<f64>()) {
                (Ok(na), Ok(nb)) => na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal),
                _ => val_a.cmp(&val_b),
            };

            if ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });
    }

    pub fn clear_sort(&mut self) {
        self.sort_column = None;
        self.apply_filter(); // Re-apply filter to restore original order
    }

    pub fn exit_mode(&mut self) {
        self.mode = Mode::Normal;
        self.pending_action = None;
        self.describe_data = None;
    }

    // =========================================================================
    // Resource Navigation
    // =========================================================================

    pub async fn navigate_to_resource(&mut self, resource_key: &str) -> Result<()> {
        if get_resource(resource_key).is_none() {
            self.error_message = Some(format!("Unknown resource: {}", resource_key));
            return Ok(());
        }

        self.parent_context = None;
        self.navigation_stack.clear();
        self.current_resource_key = resource_key.to_string();
        self.selected = 0;
        self.filter_text.clear();
        self.filter_active = false;
        self.mode = Mode::Normal;
        // Clear selection and scroll state
        self.selected_indices.clear();
        self.visual_mode = false;
        self.scroll_offset = 0;

        self.reset_pagination();
        self.refresh_current().await?;
        Ok(())
    }

    pub async fn navigate_to_sub_resource(&mut self, sub_resource_key: &str) -> Result<()> {
        let Some(selected_item) = self.selected_item().cloned() else {
            return Ok(());
        };

        let Some(current_resource) = self.current_resource() else {
            return Ok(());
        };

        let is_valid = current_resource
            .sub_resources
            .iter()
            .any(|s| s.resource_key == sub_resource_key);

        if !is_valid {
            self.error_message = Some(format!(
                "{} is not a sub-resource of {}",
                sub_resource_key, self.current_resource_key
            ));
            return Ok(());
        }

        let display_name = extract_json_value(&selected_item, &current_resource.name_field);
        let id = extract_json_value(&selected_item, &current_resource.id_field);
        let display = if display_name != "-" {
            display_name
        } else {
            id
        };

        if let Some(ctx) = self.parent_context.take() {
            self.navigation_stack.push(ctx);
        }

        self.parent_context = Some(ParentContext {
            resource_key: self.current_resource_key.clone(),
            item: selected_item,
            display_name: display,
        });

        self.current_resource_key = sub_resource_key.to_string();
        self.selected = 0;
        self.filter_text.clear();
        self.filter_active = false;
        // Clear selection and scroll state
        self.selected_indices.clear();
        self.visual_mode = false;
        self.scroll_offset = 0;

        self.reset_pagination();
        self.refresh_current().await?;
        Ok(())
    }

    pub async fn navigate_back(&mut self) -> Result<()> {
        if let Some(parent) = self.parent_context.take() {
            self.parent_context = self.navigation_stack.pop();
            self.current_resource_key = parent.resource_key;
            self.selected = 0;
            self.filter_text.clear();
            self.filter_active = false;
            // Clear selection and scroll state
            self.selected_indices.clear();
            self.visual_mode = false;
            self.scroll_offset = 0;

            self.reset_pagination();
            self.refresh_current().await?;
        }
        Ok(())
    }

    pub fn get_breadcrumb(&self) -> Vec<String> {
        let mut path = Vec::new();

        for ctx in &self.navigation_stack {
            path.push(format!("{}:{}", ctx.resource_key, ctx.display_name));
        }

        if let Some(ctx) = &self.parent_context {
            path.push(format!("{}:{}", ctx.resource_key, ctx.display_name));
        }

        path.push(self.current_resource_key.clone());
        path
    }

    // =========================================================================
    // Project/Zone Switching
    // =========================================================================

    pub async fn switch_zone(&mut self, zone: &str) -> Result<()> {
        self.client.switch_zone(zone);
        self.zone = zone.to_string();

        if let Err(e) = self.config.set_zone(zone) {
            tracing::warn!("Failed to save zone to config: {}", e);
        }

        Ok(())
    }

    pub async fn switch_project(&mut self, project: &str) -> Result<()> {
        self.client.switch_project(project).await?;
        self.project = project.to_string();

        if let Err(e) = self.config.set_project(project) {
            tracing::warn!("Failed to save project to config: {}", e);
        }

        // Apply project-specific theme if configured
        let theme_name = self.config.effective_theme(project);
        self.theme_manager.set_theme(&theme_name);

        Ok(())
    }

    pub async fn select_project(&mut self) -> Result<()> {
        if let Some(project) = self.projects_filtered.get(self.projects_selected) {
            let project = project.clone();
            self.switch_project(&project).await?;
            self.refresh_current().await?;
        }
        self.exit_mode();
        Ok(())
    }

    pub async fn select_zone(&mut self) -> Result<()> {
        if let Some(zone) = self.zones_filtered.get(self.zones_selected) {
            let zone = zone.clone();
            self.switch_zone(&zone).await?;
            self.refresh_current().await?;
        }
        self.exit_mode();
        Ok(())
    }

    // =========================================================================
    // Command Execution
    // =========================================================================

    pub async fn execute_command(&mut self) -> Result<bool> {
        let command_text = if self.command_text.is_empty() {
            self.command_preview.clone().unwrap_or_default()
        } else if let Some(preview) = &self.command_preview {
            if preview.contains(&self.command_text) {
                preview.clone()
            } else {
                self.command_text.clone()
            }
        } else {
            self.command_text.clone()
        };

        let parts: Vec<&str> = command_text.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(false);
        }

        let cmd = parts[0];

        match cmd {
            "q" | "quit" => return Ok(true),
            "back" => {
                self.navigate_back().await?;
            },
            "projects" => {
                self.enter_projects_mode();
            },
            "zones" => {
                self.enter_zones_mode();
            },
            "notifications" => {
                if parts.len() > 1 && parts[1] == "clear" {
                    self.clear_notifications();
                } else {
                    self.enter_notifications_mode();
                }
            },
            "zone" if parts.len() > 1 => {
                self.switch_zone(parts[1]).await?;
                self.refresh_current().await?;
            },
            "project" if parts.len() > 1 => {
                self.switch_project(parts[1]).await?;
                self.refresh_current().await?;
            },
            "theme" => {
                if parts.len() > 1 {
                    let theme_name = parts[1];
                    if self.theme_manager.set_theme(theme_name) {
                        if let Err(e) = self.config.set_theme(theme_name) {
                            tracing::warn!("Failed to save theme to config: {}", e);
                        }
                    } else {
                        self.error_message = Some(format!("Unknown theme: {}", theme_name));
                    }
                } else {
                    // Show available themes
                    let themes = ThemeManager::list_available().join(", ");
                    self.error_message = Some(format!("Available themes: {}", themes));
                }
            },
            "alias" if parts.len() >= 3 => {
                // :alias <alias> <resource_key>
                let alias = parts[1];
                let resource_key = parts[2];
                if get_resource(resource_key).is_some() {
                    if let Err(e) = self.config.add_alias(alias, resource_key) {
                        self.error_message = Some(format!("Failed to save alias: {}", e));
                    }
                } else {
                    self.error_message = Some(format!("Unknown resource: {}", resource_key));
                }
            },
            _ => {
                // Check for alias first - clone to avoid borrow issues
                let resolved_cmd = self
                    .config
                    .resolve_alias(cmd)
                    .cloned()
                    .unwrap_or_else(|| cmd.to_string());

                if get_resource(&resolved_cmd).is_some() {
                    if let Some(resource) = self.current_resource() {
                        let is_sub = resource
                            .sub_resources
                            .iter()
                            .any(|s| s.resource_key == resolved_cmd);
                        if is_sub && self.selected_item().is_some() {
                            self.navigate_to_sub_resource(&resolved_cmd).await?;
                        } else {
                            self.navigate_to_resource(&resolved_cmd).await?;
                        }
                    } else {
                        self.navigate_to_resource(&resolved_cmd).await?;
                    }
                } else {
                    self.error_message = Some(format!("Unknown command: {}", cmd));
                }
            },
        }

        Ok(false)
    }

    // =========================================================================
    // Virtual Scrolling
    // =========================================================================

    /// Update the viewport height (called from UI during render)
    pub fn update_viewport(&mut self, height: usize) {
        self.viewport_height = height.max(1);
    }

    /// Ensure the selected item is visible in the viewport
    pub fn ensure_visible(&mut self) {
        if self.filtered_items.is_empty() {
            self.scroll_offset = 0;
            return;
        }

        let visible_height = self.viewport_height;
        let margin = 2; // Keep cursor at least this far from edge

        // If selected is above visible area, scroll up
        if self.selected < self.scroll_offset + margin {
            // Scroll so selected is near top with margin
            self.scroll_offset = self.selected.saturating_sub(margin);
        }
        // If selected is below visible area, scroll down
        else if self.selected >= self.scroll_offset + visible_height.saturating_sub(margin) {
            // Scroll so selected is near bottom with margin
            self.scroll_offset = self
                .selected
                .saturating_sub(visible_height.saturating_sub(margin + 1));
        }

        // Clamp scroll offset to valid range
        let max_offset = self
            .filtered_items
            .len()
            .saturating_sub(self.viewport_height);
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    /// Get the range of visible items based on scroll offset and viewport
    pub fn visible_range(&self) -> Range<usize> {
        let start = self.scroll_offset;
        let end = (self.scroll_offset + self.viewport_height).min(self.filtered_items.len());
        start..end
    }

    // =========================================================================
    // Multi-Selection (Bulk Operations)
    // =========================================================================

    /// Toggle selection of the current item
    pub fn toggle_selection(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }

        if self.selected_indices.contains(&self.selected) {
            self.selected_indices.remove(&self.selected);
        } else {
            self.selected_indices.insert(self.selected);
        }
    }

    /// Select all filtered items
    pub fn select_all(&mut self) {
        self.selected_indices = (0..self.filtered_items.len()).collect();
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        self.selected_indices.clear();
        self.visual_mode = false;
    }

    /// Check if an item at the given index is selected
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

    /// Get count of selected items
    pub fn selection_count(&self) -> usize {
        self.selected_indices.len()
    }

    /// Get all selected items
    #[allow(dead_code)]
    pub fn selected_items(&self) -> Vec<&Value> {
        self.selected_indices
            .iter()
            .filter_map(|&idx| self.filtered_items.get(idx))
            .collect()
    }

    /// Get IDs of all selected items (for bulk actions)
    pub fn selected_resource_ids(&self) -> Vec<String> {
        let Some(resource) = self.current_resource() else {
            return Vec::new();
        };

        self.selected_indices
            .iter()
            .filter_map(|&idx| {
                self.filtered_items.get(idx).map(|item| {
                    let id = extract_json_value(item, &resource.name_field);
                    if id != "-" && !id.is_empty() {
                        id
                    } else {
                        extract_json_value(item, &resource.id_field)
                    }
                })
            })
            .collect()
    }

    /// Toggle visual/multi-select mode
    pub fn toggle_visual_mode(&mut self) {
        self.visual_mode = !self.visual_mode;
        if !self.visual_mode {
            // Optionally clear selection when exiting visual mode
            // self.clear_selection();
        }
    }

    /// Extend selection from current position (for Shift+j/k)
    pub fn extend_selection_down(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }

        // Select current item if not already
        self.selected_indices.insert(self.selected);

        // Move down and select
        if self.selected < self.filtered_items.len() - 1 {
            self.selected += 1;
            self.selected_indices.insert(self.selected);
        }
    }

    /// Extend selection upward (for Shift+k)
    pub fn extend_selection_up(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }

        // Select current item if not already
        self.selected_indices.insert(self.selected);

        // Move up and select
        if self.selected > 0 {
            self.selected -= 1;
            self.selected_indices.insert(self.selected);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_range_basic() {
        // Simulate: 100 items, viewport 10, scroll_offset 0
        let filtered_items: Vec<Value> = (0..100).map(|i| serde_json::json!({"id": i})).collect();
        let scroll_offset = 0;
        let viewport_height = 10;

        let start = scroll_offset;
        let end = (scroll_offset + viewport_height).min(filtered_items.len());
        let range = start..end;

        assert_eq!(range, 0..10);
    }

    #[test]
    fn test_visible_range_scrolled() {
        let filtered_items: Vec<Value> = (0..100).map(|i| serde_json::json!({"id": i})).collect();
        let scroll_offset = 50;
        let viewport_height = 10;

        let start = scroll_offset;
        let end = (scroll_offset + viewport_height).min(filtered_items.len());
        let range = start..end;

        assert_eq!(range, 50..60);
    }

    #[test]
    fn test_visible_range_at_end() {
        let filtered_items: Vec<Value> = (0..25).map(|i| serde_json::json!({"id": i})).collect();
        let scroll_offset = 20;
        let viewport_height = 10;

        let start = scroll_offset;
        let end = (scroll_offset + viewport_height).min(filtered_items.len());
        let range = start..end;

        assert_eq!(range, 20..25);
    }

    #[test]
    fn test_selection_toggle() {
        let mut selected_indices = HashSet::new();
        let selected = 5;

        // Toggle on
        if selected_indices.contains(&selected) {
            selected_indices.remove(&selected);
        } else {
            selected_indices.insert(selected);
        }
        assert!(selected_indices.contains(&5));

        // Toggle off
        if selected_indices.contains(&selected) {
            selected_indices.remove(&selected);
        } else {
            selected_indices.insert(selected);
        }
        assert!(!selected_indices.contains(&5));
    }

    #[test]
    fn test_select_all() {
        let item_count = 50;
        let selected_indices: HashSet<usize> = (0..item_count).collect();

        assert_eq!(selected_indices.len(), 50);
        assert!(selected_indices.contains(&0));
        assert!(selected_indices.contains(&49));
    }

    #[test]
    fn test_clear_selection() {
        let mut selected_indices: HashSet<usize> = (0..10).collect();
        assert_eq!(selected_indices.len(), 10);

        selected_indices.clear();
        assert_eq!(selected_indices.len(), 0);
    }

    #[test]
    fn test_selection_count() {
        let selected_indices: HashSet<usize> = vec![1, 3, 5, 7, 9].into_iter().collect();
        assert_eq!(selected_indices.len(), 5);
    }

    #[test]
    fn test_ensure_visible_logic() {
        let viewport_height: usize = 10;
        let margin: usize = 2;

        // Case 1: selected is at top, scroll should be 0
        let selected: usize = 0;
        let mut scroll_offset: usize = 5;
        if selected < scroll_offset + margin {
            scroll_offset = selected.saturating_sub(margin);
        }
        assert_eq!(scroll_offset, 0);

        // Case 2: selected is at bottom, scroll should adjust
        let selected: usize = 50;
        let mut scroll_offset: usize = 30;
        let filtered_items_len: usize = 100;
        if selected >= scroll_offset + viewport_height.saturating_sub(margin) {
            scroll_offset = selected.saturating_sub(viewport_height.saturating_sub(margin + 1));
        }
        let max_offset = filtered_items_len.saturating_sub(viewport_height);
        scroll_offset = scroll_offset.min(max_offset);
        assert!(scroll_offset <= max_offset);
        assert!(selected >= scroll_offset);
        assert!(selected < scroll_offset + viewport_height);
    }

    #[test]
    fn test_extend_selection_down() {
        let mut selected_indices = HashSet::new();
        let mut selected = 5;
        let filtered_items_len = 100;

        // Insert current and move down
        selected_indices.insert(selected);
        if selected < filtered_items_len - 1 {
            selected += 1;
            selected_indices.insert(selected);
        }

        assert!(selected_indices.contains(&5));
        assert!(selected_indices.contains(&6));
        assert_eq!(selected, 6);
    }

    #[test]
    fn test_extend_selection_up() {
        let mut selected_indices = HashSet::new();
        let mut selected = 5;

        // Insert current and move up
        selected_indices.insert(selected);
        if selected > 0 {
            selected -= 1;
            selected_indices.insert(selected);
        }

        assert!(selected_indices.contains(&5));
        assert!(selected_indices.contains(&4));
        assert_eq!(selected, 4);
    }
}
