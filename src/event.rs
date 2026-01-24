//! Event Handling
//!
//! Keyboard and event handling for tgcp.

use crate::app::{App, Mode};
use crate::gcp::client::extract_operation_url;
use crate::resource::{execute_action, extract_json_value};
use crate::shell::{self, ShellResult, SshOptions};
use anyhow::Result;
use crossterm::event::{poll, read, Event, KeyCode, KeyModifiers};
use std::time::Duration;

// =========================================================================
// Configuration Constants
// =========================================================================

/// Timeout for double-key sequences like 'gg' (go to top)
const DOUBLE_KEY_TIMEOUT_MS: u64 = 1000;

/// Number of items to scroll for page up/down
const PAGE_SCROLL_SIZE: usize = 10;

/// Event poll interval in milliseconds
const EVENT_POLL_INTERVAL_MS: u64 = 100;

/// Handle events, returns true if app should quit
pub async fn handle_events(app: &mut App) -> Result<bool> {
    if poll(Duration::from_millis(EVENT_POLL_INTERVAL_MS))? {
        if let Event::Key(key) = read()? {
            return handle_key_event(app, key.code, key.modifiers).await;
        }
    }
    Ok(false)
}

async fn handle_key_event(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
    // Global quit shortcut
    if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
        return Ok(true);
    }

    match app.mode {
        Mode::Normal => handle_normal_mode(app, code, modifiers).await,
        Mode::Command => handle_command_mode(app, code, modifiers).await,
        Mode::Help => handle_help_mode(app, code),
        Mode::Confirm => handle_confirm_mode(app, code, modifiers).await,
        Mode::Warning => handle_warning_mode(app, code),
        Mode::Projects => handle_projects_mode(app, code, modifiers).await,
        Mode::Zones => handle_zones_mode(app, code, modifiers).await,
        Mode::Describe => handle_describe_mode(app, code, modifiers),
        Mode::Notifications => handle_notifications_mode(app, code),
        Mode::ColumnConfig => handle_column_config_mode(app, code),
    }
}

async fn handle_normal_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
    // Check for double-g (go to top) - keep for vim users but increase timeout
    if code == KeyCode::Char('g') {
        if let Some((KeyCode::Char('g'), time)) = app.last_key_press {
            if time.elapsed() < Duration::from_millis(DOUBLE_KEY_TIMEOUT_MS) {
                app.go_to_top();
                app.last_key_press = None;
                return Ok(false);
            }
        }
        app.last_key_press = Some((code, std::time::Instant::now()));
        return Ok(false);
    }

    // Clear last key press for non-g keys
    app.last_key_press = None;

    // Handle filter input first
    if app.filter_sort.filter_active {
        match code {
            KeyCode::Esc => {
                app.clear_filter();
            },
            KeyCode::Enter => {
                app.filter_sort.filter_active = false;
            },
            KeyCode::Backspace => {
                app.filter_sort.filter_text.pop();
                app.apply_filter();
            },
            KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                app.filter_sort.filter_text.push(c);
                app.apply_filter();
            },
            _ => {},
        }
        return Ok(false);
    }

    match code {
        // Quit
        KeyCode::Char('q') => return Ok(true),

        // Multi-selection (bulk operations)
        KeyCode::Char(' ') => {
            // Space toggles selection on current item
            app.toggle_selection();
            app.next(); // Move to next item after selecting
        },
        KeyCode::Char('v') if !modifiers.contains(KeyModifiers::SHIFT) => {
            // Toggle visual/multi-select mode
            app.toggle_visual_mode();
        },
        KeyCode::Char('V') | KeyCode::Char('v') if modifiers.contains(KeyModifiers::SHIFT) => {
            // Select all visible items
            app.select_all();
        },
        KeyCode::Esc if app.selection.count() > 0 || app.selection.visual_mode => {
            // Clear selection with Escape (only when there's selection or visual mode)
            app.clear_selection();
        },
        KeyCode::Char('J') | KeyCode::Char('j') if modifiers.contains(KeyModifiers::SHIFT) => {
            // Extend selection downward
            app.extend_selection_down();
        },
        KeyCode::Char('K') | KeyCode::Char('k') if modifiers.contains(KeyModifiers::SHIFT) => {
            // Extend selection upward
            app.extend_selection_up();
        },

        // Navigation - vim style + accessible alternatives
        KeyCode::Char('j') | KeyCode::Down => app.next(),
        KeyCode::Char('k') | KeyCode::Up => app.previous(),
        KeyCode::Home => app.go_to_top(),
        KeyCode::End | KeyCode::Char('G') => app.go_to_bottom(),
        KeyCode::PageDown => app.page_down(PAGE_SCROLL_SIZE),
        KeyCode::PageUp => app.page_up(PAGE_SCROLL_SIZE),

        // Ctrl+D/U for page navigation
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.page_down(PAGE_SCROLL_SIZE);
        },
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.page_up(PAGE_SCROLL_SIZE);
        },

        // Quick jump to position 1-9
        KeyCode::Char(c @ '1'..='9') if !app.filter_sort.filter_active => {
            let idx = c.to_digit(10).unwrap() as usize - 1;
            if idx < app.filtered_items.len() {
                app.nav.selected = idx;
            }
        },

        // Sorting with F1-F6
        KeyCode::F(n @ 1..=6) => {
            app.sort_by_column((n - 1) as usize);
        },
        // Clear sort with F12
        KeyCode::F(12) => {
            app.clear_sort();
        },

        // Pagination
        KeyCode::Char(']') => {
            app.next_page().await?;
        },
        KeyCode::Char('[') => {
            app.prev_page().await?;
        },

        // Refresh
        KeyCode::Char('R') => {
            app.reset_pagination();
            app.filter_sort.sort_column = None; // Reset sort on refresh
            app.refresh_current().await?;
        },

        // Describe/Enter
        KeyCode::Enter => {
            app.enter_describe_mode().await;
        },
        KeyCode::Char('d') => {
            app.enter_describe_mode().await;
        },

        // Filter
        KeyCode::Char('/') => {
            app.filter_sort.filter_active = true;
        },

        // Command mode
        KeyCode::Char(':') => {
            app.enter_command_mode();
        },

        // Help
        KeyCode::Char('?') => {
            app.enter_help_mode();
        },

        // Back navigation
        KeyCode::Backspace | KeyCode::Left => {
            if app.nav.parent_context.is_some() {
                app.navigate_back().await?;
            }
        },
        KeyCode::Char('b') => {
            if app.nav.parent_context.is_some() {
                app.navigate_back().await?;
            }
        },

        // Projects
        KeyCode::Char('p') => {
            app.enter_projects_mode();
        },

        // Zones
        KeyCode::Char('z') => {
            app.enter_zones_mode();
        },

        // Notifications
        KeyCode::Char('n') => {
            app.enter_notifications_mode();
        },

        // Column configuration
        KeyCode::Char('o') => {
            app.enter_column_config_mode();
        },

        // Delete action with Delete key (resolves Ctrl+D conflict)
        KeyCode::Delete => {
            if let Some(resource) = app.current_resource() {
                // Find delete action (usually has "delete" in sdk_method)
                let delete_action = resource
                    .actions
                    .iter()
                    .find(|a| a.sdk_method.to_lowercase().contains("delete"));

                if let Some(action_def) = delete_action {
                    handle_action(app, action_def).await?;
                }
            }
        },

        // Sub-resource and action shortcuts
        KeyCode::Char(c) => {
            // Check if it's a sub-resource shortcut
            if let Some(resource) = app.current_resource() {
                let sub = resource
                    .sub_resources
                    .iter()
                    .find(|s| s.shortcut == c.to_string());

                if let Some(sub_def) = sub {
                    if app.selected_item().is_some() {
                        let key = sub_def.resource_key.clone();
                        app.navigate_to_sub_resource(&key).await?;
                        return Ok(false);
                    }
                }

                // Check if it's an action shortcut
                let action = resource
                    .actions
                    .iter()
                    .find(|a| a.shortcut.as_deref() == Some(&c.to_string()));

                if let Some(action_def) = action {
                    handle_action(app, action_def).await?;
                    return Ok(false);
                }
            }
        },

        _ => {},
    }

    Ok(false)
}

async fn handle_action(app: &mut App, action_def: &crate::resource::ActionDef) -> Result<()> {
    // Shell actions don't respect readonly mode (they don't modify resources)
    if app.readonly && !action_def.shell_action {
        app.show_warning("Read-only mode: actions are disabled");
        return Ok(());
    }

    let Some(resource) = app.current_resource() else {
        return Ok(());
    };

    // Check if we have multiple selections (bulk operation)
    let selected_ids = app.selected_resource_ids();
    let has_bulk_selection = selected_ids.len() > 1;

    // If bulk selection, handle bulk action
    if has_bulk_selection && !action_def.shell_action {
        return handle_bulk_action(app, action_def, selected_ids).await;
    }

    // Single item action (existing behavior)
    let Some(item) = app.selected_item().cloned() else {
        return Ok(());
    };

    // Get resource ID (use name for GCP resources)
    let resource_id = extract_json_value(&item, &resource.name_field);
    if resource_id == "-" {
        return Ok(());
    }

    // Handle shell actions (SSH, console, etc.)
    if action_def.shell_action {
        return handle_shell_action(app, action_def, &resource_id, &item).await;
    }

    if action_def.requires_confirm() {
        if let Some(pending) = app.create_pending_action(action_def, &resource_id) {
            app.enter_confirm_mode(pending);
        }
    } else {
        // Create notification before executing
        let notification_id = app.create_operation_notification(
            &action_def.sdk_method,
            &resource.service,
            &resource_id,
        );

        // Execute directly
        let result = execute_action(
            &resource.service,
            &action_def.sdk_method,
            &app.client,
            &resource_id,
            &serde_json::Value::Null,
        )
        .await;

        match result {
            Ok(response) => {
                // Extract operation URL for polling
                let operation_url = extract_operation_url(&response);
                app.mark_notification_in_progress(notification_id, operation_url.clone());

                // If no operation URL (immediate completion), mark success
                if operation_url.is_none() {
                    app.mark_notification_success(notification_id);
                }

                app.refresh_current().await?;
            },
            Err(e) => {
                let error_msg = crate::gcp::client::format_gcp_error(&e);
                app.mark_notification_error(notification_id, error_msg.clone());
                app.error_message = Some(error_msg);
            },
        }
    }

    Ok(())
}

/// Handle bulk action on multiple selected resources
async fn handle_bulk_action(
    app: &mut App,
    action_def: &crate::resource::ActionDef,
    resource_ids: Vec<String>,
) -> Result<()> {
    let Some(resource) = app.current_resource() else {
        return Ok(());
    };

    let count = resource_ids.len();

    // Build bulk confirmation message
    let action_name = &action_def.display_name;
    let is_destructive = action_def
        .confirm
        .as_ref()
        .map(|c| c.destructive)
        .unwrap_or(false);

    let message = format!(
        "{} {} {}?",
        action_name,
        count,
        if count == 1 { "resource" } else { "resources" }
    );

    // Create bulk pending action using the existing PendingAction struct
    // We store all resource IDs joined by a special separator
    let bulk_resource_id = resource_ids.join("\n");

    let pending = crate::app::PendingAction {
        service: resource.service.clone(),
        sdk_method: action_def.sdk_method.clone(),
        resource_id: bulk_resource_id,
        message,
        destructive: is_destructive,
        selected_yes: false,
    };

    app.enter_confirm_mode(pending);
    Ok(())
}

/// Execute SSH to an instance with proper terminal handling and error reporting
fn execute_ssh_to_instance(
    app: &mut App,
    resource_id: &str,
    item: &serde_json::Value,
    force_iap: bool,
) {
    // Get zone from the instance if available
    let zone = extract_json_value(item, "zone_short");
    let zone = if zone != "-" { zone } else { app.zone.clone() };

    // Build SSH options
    let mut opts = SshOptions::new(resource_id, &zone, &app.project);

    // Apply IAP: either forced (for ssh_instance_iap) or from config
    if force_iap || app.config.ssh.use_iap {
        opts = opts.with_iap();
    }
    opts.extra_args = app.config.ssh.extra_args.clone();

    let iap_label = if opts.use_iap { " (IAP)" } else { "" };

    // Execute SSH with terminal handling
    let result = shell::execute_with_terminal_handling(|| shell::ssh_to_instance(&opts));

    match result {
        Ok(ShellResult::Success) => {
            tracing::info!("SSH{} session completed successfully", iap_label);
        },
        Ok(ShellResult::Failed(code)) => {
            app.error_message = Some(format!("SSH{} exited with code {}", iap_label, code));
        },
        Ok(ShellResult::Error(msg)) => {
            app.error_message = Some(msg);
        },
        Err(e) => {
            app.error_message = Some(format!("SSH{} error: {}", iap_label, e));
        },
    }
}

/// Handle shell actions like SSH, console URL, etc.
async fn handle_shell_action(
    app: &mut App,
    action_def: &crate::resource::ActionDef,
    resource_id: &str,
    item: &serde_json::Value,
) -> Result<()> {
    let method = action_def.sdk_method.as_str();

    match method {
        "ssh_instance" => {
            execute_ssh_to_instance(app, resource_id, item, false);
        },
        "ssh_instance_iap" => {
            execute_ssh_to_instance(app, resource_id, item, true);
        },
        "open_console" => {
            let zone = extract_json_value(item, "zone_short");
            let zone = if zone != "-" { zone } else { app.zone.clone() };

            let url =
                shell::console_url(&app.current_resource_key, resource_id, &app.project, &zone);

            let result = shell::open_browser(&url);

            match result {
                ShellResult::Success => {
                    tracing::info!("Opened console URL: {}", url);
                },
                ShellResult::Error(msg) => {
                    app.error_message = Some(format!("Failed to open browser: {}", msg));
                },
                _ => {},
            }
        },
        _ => {
            app.error_message = Some(format!("Unknown shell action: {}", method));
        },
    }

    Ok(())
}

async fn handle_command_mode(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<bool> {
    match code {
        KeyCode::Esc => {
            app.exit_mode();
        },
        KeyCode::Enter => {
            let should_quit = app.execute_command().await?;
            app.exit_mode();
            return Ok(should_quit);
        },
        KeyCode::Backspace => {
            app.command.text.pop();
            app.update_command_suggestions();
        },
        KeyCode::Tab | KeyCode::Right => {
            app.apply_suggestion();
        },
        KeyCode::Down => {
            app.next_suggestion();
        },
        KeyCode::Up => {
            app.prev_suggestion();
        },
        KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
            app.command.text.push(c);
            app.update_command_suggestions();
        },
        _ => {},
    }
    Ok(false)
}

fn handle_help_mode(app: &mut App, code: KeyCode) -> Result<bool> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter => {
            app.exit_mode();
        },
        _ => {},
    }
    Ok(false)
}

async fn handle_confirm_mode(
    app: &mut App,
    code: KeyCode,
    _modifiers: KeyModifiers,
) -> Result<bool> {
    match code {
        KeyCode::Esc | KeyCode::Char('N') => {
            app.exit_mode();
        },
        KeyCode::Left | KeyCode::Char('h') => {
            if let Some(ref mut pending) = app.pending_action {
                pending.selected_yes = true;
            }
        },
        KeyCode::Right | KeyCode::Char('l') => {
            if let Some(ref mut pending) = app.pending_action {
                pending.selected_yes = false;
            }
        },
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(pending) = app.pending_action.take() {
                if pending.selected_yes || code == KeyCode::Char('y') || code == KeyCode::Char('Y')
                {
                    // Check if this is a bulk action (multiple resource IDs separated by newline)
                    let resource_ids: Vec<&str> = pending.resource_id.split('\n').collect();
                    let is_bulk = resource_ids.len() > 1;

                    if is_bulk {
                        // Execute bulk action
                        let mut success_count = 0;
                        let mut error_count = 0;
                        let total = resource_ids.len();

                        for resource_id in resource_ids {
                            // Create notification for each resource
                            let notification_id = app.create_operation_notification(
                                &pending.sdk_method,
                                &pending.service,
                                resource_id,
                            );

                            // Execute the action
                            let result = execute_action(
                                &pending.service,
                                &pending.sdk_method,
                                &app.client,
                                resource_id,
                                &serde_json::Value::Null,
                            )
                            .await;

                            match result {
                                Ok(response) => {
                                    let operation_url = extract_operation_url(&response);
                                    app.mark_notification_in_progress(
                                        notification_id,
                                        operation_url.clone(),
                                    );
                                    if operation_url.is_none() {
                                        app.mark_notification_success(notification_id);
                                    }
                                    success_count += 1;
                                },
                                Err(e) => {
                                    let error_msg = crate::gcp::client::format_gcp_error(&e);
                                    app.mark_notification_error(notification_id, error_msg);
                                    error_count += 1;
                                },
                            }
                        }

                        // Show summary message
                        if error_count > 0 {
                            app.error_message = Some(format!(
                                "Bulk action: {} succeeded, {} failed of {}",
                                success_count, error_count, total
                            ));
                        }

                        // Clear selection after bulk action
                        app.clear_selection();

                        // Refresh view
                        app.refresh_current().await?;
                    } else {
                        // Single item action (existing behavior)
                        let notification_id = app.create_operation_notification(
                            &pending.sdk_method,
                            &pending.service,
                            &pending.resource_id,
                        );

                        let result = execute_action(
                            &pending.service,
                            &pending.sdk_method,
                            &app.client,
                            &pending.resource_id,
                            &serde_json::Value::Null,
                        )
                        .await;

                        match result {
                            Ok(response) => {
                                let operation_url = extract_operation_url(&response);
                                app.mark_notification_in_progress(
                                    notification_id,
                                    operation_url.clone(),
                                );

                                if operation_url.is_none() {
                                    app.mark_notification_success(notification_id);
                                }

                                app.refresh_current().await?;
                            },
                            Err(e) => {
                                let error_msg = crate::gcp::client::format_gcp_error(&e);
                                app.mark_notification_error(notification_id, error_msg.clone());
                                app.error_message = Some(error_msg);
                            },
                        }
                    }
                }
            }
            app.exit_mode();
        },
        _ => {},
    }
    Ok(false)
}

fn handle_warning_mode(app: &mut App, code: KeyCode) -> Result<bool> {
    match code {
        KeyCode::Esc | KeyCode::Enter => {
            app.warning_message = None;
            app.exit_mode();
        },
        _ => {},
    }
    Ok(false)
}

/// Selector type for generic selector mode handling
enum SelectorType {
    Projects,
    Zones,
}

/// Generic handler for selector modes (projects/zones) to avoid code duplication
async fn handle_selector_mode(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
    selector_type: SelectorType,
) -> Result<bool> {
    match code {
        KeyCode::Esc => {
            app.exit_mode();
        },
        KeyCode::Enter => match selector_type {
            SelectorType::Projects => app.select_project().await?,
            SelectorType::Zones => app.select_zone().await?,
        },
        KeyCode::Char('j') | KeyCode::Down => {
            app.next();
        },
        KeyCode::Char('k') | KeyCode::Up => {
            app.previous();
        },
        KeyCode::Home => {
            app.go_to_top();
        },
        KeyCode::End | KeyCode::Char('G') => {
            app.go_to_bottom();
        },
        KeyCode::PageDown => {
            app.page_down(PAGE_SCROLL_SIZE);
        },
        KeyCode::PageUp => {
            app.page_up(PAGE_SCROLL_SIZE);
        },
        KeyCode::Backspace => match selector_type {
            SelectorType::Projects => {
                app.projects_selector.search_text.pop();
                app.apply_projects_filter();
            },
            SelectorType::Zones => {
                app.zones_selector.search_text.pop();
                app.apply_zones_filter();
            },
        },
        KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => match selector_type {
            SelectorType::Projects => {
                app.projects_selector.search_text.push(c);
                app.apply_projects_filter();
            },
            SelectorType::Zones => {
                app.zones_selector.search_text.push(c);
                app.apply_zones_filter();
            },
        },
        _ => {},
    }
    Ok(false)
}

async fn handle_projects_mode(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<bool> {
    handle_selector_mode(app, code, modifiers, SelectorType::Projects).await
}

async fn handle_zones_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
    handle_selector_mode(app, code, modifiers, SelectorType::Zones).await
}

fn handle_describe_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
            app.exit_mode();
        },
        KeyCode::Char('j') | KeyCode::Down => {
            app.describe.scroll = app.describe.scroll.saturating_add(1);
        },
        KeyCode::Char('k') | KeyCode::Up => {
            app.describe.scroll = app.describe.scroll.saturating_sub(1);
        },
        KeyCode::PageDown => {
            app.describe.scroll = app.describe.scroll.saturating_add(10);
        },
        KeyCode::PageUp => {
            app.describe.scroll = app.describe.scroll.saturating_sub(10);
        },
        KeyCode::Char('d') => {
            if modifiers.contains(KeyModifiers::CONTROL) {
                app.describe.scroll = app.describe.scroll.saturating_add(10);
            } else {
                app.exit_mode();
            }
        },
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.describe.scroll = app.describe.scroll.saturating_sub(10);
        },
        KeyCode::Char('g') | KeyCode::Home => {
            app.describe.scroll = 0;
        },
        KeyCode::Char('G') | KeyCode::End => {
            app.describe_scroll_to_bottom(30); // Approximate visible lines
        },
        _ => {},
    }
    Ok(false)
}

fn handle_notifications_mode(app: &mut App, code: KeyCode) -> Result<bool> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('n') => {
            app.exit_mode();
        },
        KeyCode::Char('j') | KeyCode::Down => {
            let count = app.notification_manager.notifications.len();
            if count > 0 && app.notifications_selected < count - 1 {
                app.notifications_selected += 1;
            }
        },
        KeyCode::Char('k') | KeyCode::Up => {
            app.notifications_selected = app.notifications_selected.saturating_sub(1);
        },
        KeyCode::Home | KeyCode::Char('g') => {
            app.notifications_selected = 0;
        },
        KeyCode::End | KeyCode::Char('G') => {
            let count = app.notification_manager.notifications.len();
            if count > 0 {
                app.notifications_selected = count - 1;
            }
        },
        KeyCode::Char('c') => {
            // Clear all notifications
            app.clear_notifications();
            app.notifications_selected = 0;
        },
        _ => {},
    }
    Ok(false)
}

fn handle_column_config_mode(app: &mut App, code: KeyCode) -> Result<bool> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.cancel_column_config();
        },
        KeyCode::Enter => {
            app.apply_column_config();
        },
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(ref mut state) = app.column_config_state {
                if state.selected < state.columns.len().saturating_sub(1) {
                    state.selected += 1;
                }
            }
        },
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(ref mut state) = app.column_config_state {
                state.selected = state.selected.saturating_sub(1);
            }
        },
        KeyCode::Char(' ') => {
            app.toggle_column_visibility();
        },
        KeyCode::Home | KeyCode::Char('g') => {
            if let Some(ref mut state) = app.column_config_state {
                state.selected = 0;
            }
        },
        KeyCode::End | KeyCode::Char('G') => {
            if let Some(ref mut state) = app.column_config_state {
                if !state.columns.is_empty() {
                    state.selected = state.columns.len() - 1;
                }
            }
        },
        _ => {},
    }
    Ok(false)
}
