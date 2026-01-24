//! Terminal User Interface rendering module
//!
//! This module handles all UI rendering for tgcp using the ratatui framework.
//! It provides a composable widget system for displaying GCP resources in a
//! table format with vim-style navigation.
//!
//! # Architecture
//!
//! - [`splash`] - Startup splash screen animation
//! - `header` - Header bar with project/zone info
//! - `help` - Help overlay showing keybindings
//! - `dialog` - Confirmation dialogs for destructive operations
//! - `command_box` - Command mode input (`:` key)
//! - `projects` - Project selector UI
//! - `zones` - Zone selector UI
//! - `notifications` - Toast notifications for async operations
//!
//! # Virtual Scrolling
//!
//! The table rendering uses virtual scrolling for performance with large datasets.
//! Only visible rows are rendered, with a scrollbar indicating position.
//!
//! # JSON Highlighting
//!
//! The describe view provides syntax highlighting for JSON output:
//! - Keys in cyan
//! - Strings in green
//! - Numbers in light blue
//! - Booleans in magenta
//! - Null values in dark gray

mod column_config;
mod command_box;
mod dialog;
mod header;
mod help;
mod notifications;
mod projects;
pub mod splash;
mod zones;

use crate::app::{App, Mode};
use crate::resource::{extract_json_value, get_color_for_value, ColumnDef};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table, TableState,
    },
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Header (multi-line)
            Constraint::Min(1),    // Main content (table or describe)
            Constraint::Length(1), // Footer/crumb
        ])
        .split(f.area());

    // Header - multi-line with context info
    header::render(f, app, chunks[0]);

    // Main content - depends on mode and view
    match app.mode {
        Mode::Projects => {
            projects::render(f, app, chunks[1]);
        },
        Mode::Zones => {
            zones::render(f, app, chunks[1]);
        },
        Mode::Describe => {
            render_describe_view(f, app, chunks[1]);
        },
        _ => {
            render_main_content(f, app, chunks[1]);
        },
    }

    // Footer/crumb
    render_crumb(f, app, chunks[2]);

    // Overlays
    match app.mode {
        Mode::Help => {
            help::render(f, app);
        },
        Mode::Confirm | Mode::Warning => {
            dialog::render(f, app);
        },
        Mode::Command => {
            command_box::render(f, app);
        },
        Mode::Notifications => {
            notifications::render(f, app);
        },
        Mode::ColumnConfig => {
            column_config::render(f, app, f.area());
        },
        _ => {},
    }
}

fn render_main_content(f: &mut Frame, app: &mut App, area: Rect) {
    // If filter is active or has text, show filter input above table
    let show_filter = app.filter_active || !app.filter_text.is_empty();

    if show_filter {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);

        render_filter_bar(f, app, chunks[0]);
        render_dynamic_table(f, app, chunks[1]);
    } else {
        render_dynamic_table(f, app, area);
    }
}

fn render_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let cursor_style = if app.filter_active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let filter_display = if app.filter_active {
        format!("/{}_", app.filter_text)
    } else {
        format!("/{}", app.filter_text)
    };

    let paragraph = Paragraph::new(Line::from(vec![Span::styled(filter_display, cursor_style)]));
    f.render_widget(paragraph, area);
}

/// Render dynamic table based on current resource definition
/// Uses virtual scrolling for performance with large datasets
fn render_dynamic_table(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(resource) = app.current_resource() else {
        let msg = Paragraph::new("Unknown resource").style(Style::default().fg(Color::Red));
        f.render_widget(msg, area);
        return;
    };

    // Build title with count, zone info, selection, and pagination
    let title = {
        let count = app.filtered_items.len();
        let total = app.items.len();
        let is_global = resource.is_global;
        let selection_count = app.selection_count();

        // Build selection indicator
        let selection_info = if selection_count > 0 {
            format!(" [{}✓]", selection_count)
        } else if app.visual_mode {
            " [V]".to_string()
        } else {
            String::new()
        };

        // Build pagination indicator
        let page_info = if app.pagination.has_more || app.pagination.current_page > 1 {
            format!(
                " pg.{}{}",
                app.pagination.current_page,
                if app.pagination.has_more { "+" } else { "" }
            )
        } else {
            String::new()
        };

        if is_global {
            if app.filter_text.is_empty() {
                format!(
                    " {}[{}]{}{} ",
                    resource.display_name, count, selection_info, page_info
                )
            } else {
                format!(
                    " {}[{}/{}]{}{} ",
                    resource.display_name, count, total, selection_info, page_info
                )
            }
        } else if app.filter_text.is_empty() {
            format!(
                " {}({})[{}]{}{} ",
                resource.display_name, app.zone, count, selection_info, page_info
            )
        } else {
            format!(
                " {}({})[{}/{}]{}{} ",
                resource.display_name, app.zone, count, total, selection_info, page_info
            )
        }
    };

    // Create the bordered box with centered title
    let border_color = if app.visual_mode {
        Color::Magenta
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Calculate viewport - account for header row
    let visible_height = (inner_area.height as usize).saturating_sub(1);
    app.update_viewport(visible_height);
    app.ensure_visible();

    let total_items = app.filtered_items.len();
    let needs_scrollbar = total_items > visible_height;

    // Adjust table area for scrollbar if needed
    let table_area = if needs_scrollbar {
        Rect {
            width: inner_area.width.saturating_sub(1),
            ..inner_area
        }
    } else {
        inner_area
    };

    // Get visible range for virtual scrolling
    let range = app.visible_range();

    // Get hidden columns for this resource
    let hidden_columns = app.config.get_hidden_columns(&app.current_resource_key);

    // Build list of visible columns with their original indices (for sort tracking)
    let visible_columns: Vec<(usize, &ColumnDef)> = resource
        .columns
        .iter()
        .enumerate()
        .filter(|(_, col)| !hidden_columns.contains(&col.header))
        .collect();

    // Build header from column definitions with selection column and sort indicators
    let has_selection = app.selection_count() > 0 || app.visual_mode;

    let header_cells: Vec<Cell> = if has_selection {
        // Add selection column header
        let mut cells = vec![Cell::from(" ").style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )];
        cells.extend(visible_columns.iter().map(|(orig_idx, col)| {
            let sort_indicator = if app.sort_column == Some(*orig_idx) {
                if app.sort_ascending {
                    " ▲"
                } else {
                    " ▼"
                }
            } else {
                ""
            };

            let header_text = if app.sort_column == Some(*orig_idx) {
                format!(" {}{}", col.header, sort_indicator)
            } else {
                format!(" {}", col.header)
            };

            Cell::from(header_text).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        }));
        cells
    } else {
        visible_columns
            .iter()
            .map(|(orig_idx, col)| {
                let sort_indicator = if app.sort_column == Some(*orig_idx) {
                    if app.sort_ascending {
                        " ▲"
                    } else {
                        " ▼"
                    }
                } else {
                    ""
                };

                let header_text = if app.sort_column == Some(*orig_idx) {
                    format!(" {}{}", col.header, sort_indicator)
                } else {
                    format!(" {}", col.header)
                };

                Cell::from(header_text).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            })
            .collect()
    };

    let header = Row::new(header_cells).height(1);

    // Build only visible rows (virtual scrolling)
    let rows: Vec<Row> = app.filtered_items[range.clone()]
        .iter()
        .enumerate()
        .map(|(rel_idx, item)| {
            let abs_idx = range.start + rel_idx;
            let is_selected = app.is_selected(abs_idx);

            let mut cells: Vec<Cell> = Vec::new();

            // Add selection indicator column if in selection mode
            if has_selection {
                let indicator = if is_selected { "●" } else { " " };
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                cells.push(Cell::from(format!(" {}", indicator)).style(style));
            }

            // Add data cells (only for visible columns)
            cells.extend(visible_columns.iter().map(|(_, col)| {
                let value = extract_json_value(item, &col.json_path);
                let base_style = get_cell_style(&value, col);
                let display_value = format_cell_value(&value, col);

                // Apply selection highlighting to the entire row if selected
                let style = if is_selected {
                    base_style.bg(Color::Rgb(40, 60, 40))
                } else {
                    base_style
                };

                Cell::from(format!(" {}", truncate_string(&display_value, 38))).style(style)
            }));

            Row::new(cells)
        })
        .collect();

    // Build column widths - add selection column if in selection mode
    let widths: Vec<Constraint> = if has_selection {
        let mut w = vec![Constraint::Length(3)]; // Selection indicator column
        w.extend(
            visible_columns
                .iter()
                .map(|(_, col)| Constraint::Percentage(col.width)),
        );
        w
    } else {
        visible_columns
            .iter()
            .map(|(_, col)| Constraint::Percentage(col.width))
            .collect()
    };

    let table = Table::new(rows, widths).header(header).row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    // Adjust selected index for virtual scrolling
    let mut state = TableState::default();
    if app.selected >= range.start && app.selected < range.end {
        state.select(Some(app.selected - range.start));
    }

    f.render_stateful_widget(table, table_area, &mut state);

    // Render scrollbar if needed
    if needs_scrollbar {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .symbols(symbols::scrollbar::VERTICAL)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(total_items.saturating_sub(visible_height))
            .position(app.scroll_offset);

        f.render_stateful_widget(scrollbar, inner_area, &mut scrollbar_state);
    }
}

/// Get cell style based on value and column definition
fn get_cell_style(value: &str, col: &ColumnDef) -> Style {
    if let Some(ref color_map_name) = col.color_map {
        if let Some([r, g, b]) = get_color_for_value(color_map_name, value) {
            return Style::default().fg(Color::Rgb(r, g, b));
        }
    }
    Style::default()
}

/// Format cell value, adding indicators for transitional states
fn format_cell_value(value: &str, col: &ColumnDef) -> String {
    // Check if this is a state/status column with transitional states
    if col.color_map.is_some() {
        let lower = value.to_lowercase();
        // Transitional states get an arrow indicator
        if lower.contains("pending")
            || lower.contains("starting")
            || lower.contains("stopping")
            || lower.contains("staging")
            || lower.contains("provisioning")
            || lower.contains("suspending")
            || lower.contains("repairing")
        {
            return format!("{} ↻", value);
        }
    }
    value.to_string()
}

/// Truncate string for display (Unicode-safe)
fn truncate_string(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

fn render_describe_view(f: &mut Frame, app: &App, area: Rect) {
    let json = app
        .selected_item_json()
        .unwrap_or_else(|| "No item selected".to_string());

    // Apply JSON syntax highlighting
    let lines: Vec<Line> = json.lines().map(highlight_json_line).collect();
    let total_lines = lines.len();

    let title = if let Some(resource) = app.current_resource() {
        format!(" {} Details ", resource.display_name)
    } else {
        " Details ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Calculate max scroll based on inner area (content area without borders)
    let visible_lines = inner_area.height as usize;
    let max_scroll = total_lines.saturating_sub(visible_lines);
    let scroll = app.describe_scroll.min(max_scroll);

    let paragraph = Paragraph::new(lines.clone()).scroll((scroll as u16, 0));

    f.render_widget(paragraph, inner_area);

    // Render scrollbar if content exceeds visible area
    if total_lines > visible_lines {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut scrollbar_state = ScrollbarState::new(max_scroll + visible_lines).position(scroll);
        f.render_stateful_widget(scrollbar, inner_area, &mut scrollbar_state);
    }
}

/// Apply JSON syntax highlighting to a single line
fn highlight_json_line(line: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut chars = line.chars().peekable();
    let mut current = String::new();
    let mut is_key = true;

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if !current.is_empty() {
                    spans.push(Span::raw(current.clone()));
                    current.clear();
                }

                let mut string_content = String::from("\"");
                while let Some(&next_c) = chars.peek() {
                    chars.next();
                    string_content.push(next_c);
                    if next_c == '"' {
                        break;
                    }
                    if next_c == '\\' {
                        if let Some(&escaped) = chars.peek() {
                            chars.next();
                            string_content.push(escaped);
                        }
                    }
                }

                let style = if is_key {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Green)
                };
                spans.push(Span::styled(string_content, style));
            },
            ':' => {
                current.push(c);
                spans.push(Span::styled(
                    current.clone(),
                    Style::default().fg(Color::White),
                ));
                current.clear();
                is_key = false;
            },
            ',' => {
                if !current.is_empty() {
                    let style = get_json_value_style(&current);
                    spans.push(Span::styled(current.clone(), style));
                    current.clear();
                }
                spans.push(Span::styled(
                    ",".to_string(),
                    Style::default().fg(Color::White),
                ));
                is_key = true;
            },
            '{' | '}' | '[' | ']' => {
                if !current.is_empty() {
                    let style = get_json_value_style(&current);
                    spans.push(Span::styled(current.clone(), style));
                    current.clear();
                }
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default().fg(Color::Yellow),
                ));
                if c == '{' || c == '[' {
                    is_key = c == '{';
                }
            },
            ' ' | '\t' => {
                if !current.is_empty() {
                    let style = get_json_value_style(&current);
                    spans.push(Span::styled(current.clone(), style));
                    current.clear();
                }
                spans.push(Span::raw(c.to_string()));
            },
            _ => {
                current.push(c);
            },
        }
    }

    if !current.is_empty() {
        let style = get_json_value_style(&current);
        spans.push(Span::styled(current, style));
    }

    Line::from(spans)
}

/// Get style for JSON values (numbers, booleans, null)
fn get_json_value_style(value: &str) -> Style {
    let trimmed = value.trim();
    if trimmed == "null" {
        Style::default().fg(Color::DarkGray)
    } else if trimmed == "true" || trimmed == "false" {
        Style::default().fg(Color::Magenta)
    } else if trimmed.parse::<f64>().is_ok() {
        Style::default().fg(Color::LightBlue)
    } else {
        Style::default().fg(Color::White)
    }
}

fn render_crumb(f: &mut Frame, app: &App, area: Rect) {
    let breadcrumb = app.get_breadcrumb();
    let crumb_display = breadcrumb.join(" > ");

    // Build sub-resource shortcuts hint
    let shortcuts_hint = if let Some(resource) = app.current_resource() {
        if !resource.sub_resources.is_empty() && app.mode == Mode::Normal {
            let hints: Vec<String> = resource
                .sub_resources
                .iter()
                .map(|s| format!("{}:{}", s.shortcut, s.display_name))
                .collect();
            format!(" | {}", hints.join(" "))
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Build pagination hint
    let pagination_hint = if app.pagination.has_more || app.pagination.current_page > 1 {
        let mut hints = Vec::new();
        if app.pagination.current_page > 1 {
            hints.push("[:prev");
        }
        if app.pagination.has_more {
            hints.push("]:next");
        }
        format!(" | {}", hints.join(" "))
    } else {
        String::new()
    };

    // Check for toast notification
    let toast_text = app
        .notification_manager
        .current_toast()
        .map(|notif| notif.toast_message(app.notification_manager.detail_level));

    // Build notification indicator
    let notification_indicator = {
        let in_progress = app.notification_manager.in_progress_count();
        let total = app.notification_manager.notifications.len();
        if in_progress > 0 {
            format!(" [↻{}]", in_progress)
        } else if total > 0 {
            " [n]".to_string()
        } else {
            String::new()
        }
    };

    let status_text = if let Some(err) = &app.error_message {
        format!("Error: {}", err)
    } else if let Some(ref toast) = toast_text {
        toast.clone()
    } else if app.loading {
        "Loading...".to_string()
    } else if app.mode == Mode::Describe {
        "j/k: scroll | q/d/Esc: back".to_string()
    } else if app.filter_active {
        "Type to filter | Enter: apply | Esc: clear".to_string()
    } else {
        format!("{}{}", shortcuts_hint, pagination_hint)
    };

    let style = if app.error_message.is_some() {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if toast_text.is_some() {
        // Use different colors based on notification status
        if let Some(notif) = app.notification_manager.current_toast() {
            match &notif.status {
                crate::notification::NotificationStatus::Success => {
                    Style::default().fg(Color::Green)
                },
                crate::notification::NotificationStatus::Error(_) => {
                    Style::default().fg(Color::Red)
                },
                crate::notification::NotificationStatus::InProgress => {
                    Style::default().fg(Color::Yellow)
                },
                _ => Style::default().fg(Color::Cyan),
            }
        } else {
            Style::default().fg(Color::Cyan)
        }
    } else if app.loading {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Build notification indicator style
    let indicator_style = if app.notification_manager.in_progress_count() > 0 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let crumb = Line::from(vec![
        Span::styled(
            format!("<{}>", crumb_display),
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::raw(" "),
        Span::styled(status_text, style),
        Span::styled(notification_indicator, indicator_style),
    ]);

    let paragraph = Paragraph::new(crumb);
    f.render_widget(paragraph, area);
}
