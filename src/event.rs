//! Event Handling
//!
//! Keyboard and event handling for tgcp.

use crate::app::{App, Mode};
use crate::resource::{execute_action, extract_json_value};
use anyhow::Result;
use crossterm::event::{poll, read, Event, KeyCode, KeyModifiers};
use std::time::Duration;

/// Handle events, returns true if app should quit
pub async fn handle_events(app: &mut App) -> Result<bool> {
    if poll(Duration::from_millis(100))? {
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
    }
}

async fn handle_normal_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
    // Check for double-g (go to top)
    if code == KeyCode::Char('g') {
        if let Some((KeyCode::Char('g'), time)) = app.last_key_press {
            if time.elapsed() < Duration::from_millis(500) {
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
    if app.filter_active {
        match code {
            KeyCode::Esc => {
                app.clear_filter();
            },
            KeyCode::Enter => {
                app.filter_active = false;
            },
            KeyCode::Backspace => {
                app.filter_text.pop();
                app.apply_filter();
            },
            KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                app.filter_text.push(c);
                app.apply_filter();
            },
            _ => {},
        }
        return Ok(false);
    }

    match code {
        // Quit
        KeyCode::Char('q') => return Ok(true),

        // Navigation
        KeyCode::Char('j') | KeyCode::Down => app.next(),
        KeyCode::Char('k') | KeyCode::Up => app.previous(),
        KeyCode::Char('G') => app.go_to_bottom(),
        KeyCode::PageDown => app.page_down(10),
        KeyCode::PageUp => app.page_up(10),

        // Ctrl+D/U for page navigation
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.page_down(10);
        },
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.page_up(10);
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
            app.filter_active = true;
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
        KeyCode::Backspace => {
            if app.parent_context.is_some() {
                app.navigate_back().await?;
            }
        },
        KeyCode::Char('b') => {
            if app.parent_context.is_some() {
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
    if app.readonly {
        app.show_warning("Read-only mode: actions are disabled");
        return Ok(());
    }

    let Some(item) = app.selected_item() else {
        return Ok(());
    };

    let Some(resource) = app.current_resource() else {
        return Ok(());
    };

    // Get resource ID (use name for GCP resources)
    let resource_id = extract_json_value(item, &resource.name_field);
    if resource_id == "-" {
        return Ok(());
    }

    if action_def.requires_confirm() {
        if let Some(pending) = app.create_pending_action(action_def, &resource_id) {
            app.enter_confirm_mode(pending);
        }
    } else {
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
            Ok(_) => {
                app.refresh_current().await?;
            },
            Err(e) => {
                app.error_message = Some(crate::gcp::client::format_gcp_error(&e));
            },
        }
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
            app.command_text.pop();
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
            app.command_text.push(c);
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
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
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
                    // Execute the action
                    let result = execute_action(
                        &pending.service,
                        &pending.sdk_method,
                        &app.client,
                        &pending.resource_id,
                        &serde_json::Value::Null,
                    )
                    .await;

                    match result {
                        Ok(_) => {
                            app.refresh_current().await?;
                        },
                        Err(e) => {
                            app.error_message = Some(crate::gcp::client::format_gcp_error(&e));
                        },
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

async fn handle_projects_mode(
    app: &mut App,
    code: KeyCode,
    _modifiers: KeyModifiers,
) -> Result<bool> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.exit_mode();
        },
        KeyCode::Enter => {
            app.select_project().await?;
        },
        KeyCode::Char('j') | KeyCode::Down => {
            app.next();
        },
        KeyCode::Char('k') | KeyCode::Up => {
            app.previous();
        },
        KeyCode::Char('g') => {
            app.go_to_top();
        },
        KeyCode::Char('G') => {
            app.go_to_bottom();
        },
        _ => {},
    }
    Ok(false)
}

async fn handle_zones_mode(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) -> Result<bool> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.exit_mode();
        },
        KeyCode::Enter => {
            app.select_zone().await?;
        },
        KeyCode::Char('j') | KeyCode::Down => {
            app.next();
        },
        KeyCode::Char('k') | KeyCode::Up => {
            app.previous();
        },
        KeyCode::Char('g') => {
            app.go_to_top();
        },
        KeyCode::Char('G') => {
            app.go_to_bottom();
        },
        _ => {},
    }
    Ok(false)
}

fn handle_describe_mode(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.exit_mode();
        },
        KeyCode::Char('j') | KeyCode::Down => {
            app.describe_scroll = app.describe_scroll.saturating_add(1);
        },
        KeyCode::Char('k') | KeyCode::Up => {
            app.describe_scroll = app.describe_scroll.saturating_sub(1);
        },
        KeyCode::Char('d') => {
            if modifiers.contains(KeyModifiers::CONTROL) {
                app.describe_scroll = app.describe_scroll.saturating_add(10);
            } else {
                app.exit_mode();
            }
        },
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.describe_scroll = app.describe_scroll.saturating_sub(10);
        },
        KeyCode::Char('g') => {
            app.describe_scroll = 0;
        },
        KeyCode::Char('G') => {
            app.describe_scroll_to_bottom(30); // Approximate visible lines
        },
        _ => {},
    }
    Ok(false)
}
