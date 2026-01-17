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
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table, TableState,
    },
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
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
        _ => {},
    }
}

fn render_main_content(f: &mut Frame, app: &App, area: Rect) {
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
fn render_dynamic_table(f: &mut Frame, app: &App, area: Rect) {
    let Some(resource) = app.current_resource() else {
        let msg = Paragraph::new("Unknown resource").style(Style::default().fg(Color::Red));
        f.render_widget(msg, area);
        return;
    };

    // Build title with count, zone info, and pagination
    let title = {
        let count = app.filtered_items.len();
        let total = app.items.len();
        let is_global = resource.is_global;

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
                format!(" {}[{}]{} ", resource.display_name, count, page_info)
            } else {
                format!(
                    " {}[{}/{}]{} ",
                    resource.display_name, count, total, page_info
                )
            }
        } else if app.filter_text.is_empty() {
            format!(
                " {}({})[{}]{} ",
                resource.display_name, app.zone, count, page_info
            )
        } else {
            format!(
                " {}({})[{}/{}]{} ",
                resource.display_name, app.zone, count, total, page_info
            )
        }
    };

    // Create the bordered box with centered title
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Build header from column definitions with left padding and sort indicators
    let header_cells = resource.columns.iter().enumerate().map(|(idx, col)| {
        // Add sort indicator if this column is sorted
        let sort_indicator = if app.sort_column == Some(idx) {
            if app.sort_ascending {
                " ▲"
            } else {
                " ▼"
            }
        } else {
            ""
        };

        let header_text = if app.sort_column == Some(idx) {
            format!(" {}{}", col.header, sort_indicator)
        } else {
            format!(" {}", col.header)
        };

        Cell::from(header_text).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    });
    let header = Row::new(header_cells).height(1);

    // Build rows from filtered items with left padding
    let rows = app.filtered_items.iter().map(|item| {
        let cells = resource.columns.iter().map(|col| {
            let value = extract_json_value(item, &col.json_path);
            let style = get_cell_style(&value, col);
            let display_value = format_cell_value(&value, col);
            Cell::from(format!(" {}", truncate_string(&display_value, 38))).style(style)
        });
        Row::new(cells)
    });

    // Build column widths
    let widths: Vec<Constraint> = resource
        .columns
        .iter()
        .map(|col| Constraint::Percentage(col.width))
        .collect();

    let table = Table::new(rows, widths).header(header).row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = TableState::default();
    state.select(Some(app.selected));

    f.render_stateful_widget(table, inner_area, &mut state);
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
    let toast_text = app.notification_manager.current_toast().map(|notif| {
        notif.toast_message(app.notification_manager.detail_level)
    });

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
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
