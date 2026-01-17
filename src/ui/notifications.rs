//! Notifications Panel UI
//!
//! Renders the notifications history panel overlay.

use crate::app::App;
use crate::notification::NotificationStatus;
use ratatui::{
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

/// Render the notifications history panel as an overlay
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    // Center the panel (80% width, 70% height)
    let popup_width = (area.width as f32 * 0.8) as u16;
    let popup_height = (area.height as f32 * 0.7) as u16;
    let popup_x = (area.width - popup_width) / 2;
    let popup_y = (area.height - popup_height) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    f.render_widget(Clear, popup_area);

    let in_progress = app.notification_manager.in_progress_count();
    let title = if in_progress > 0 {
        format!(" Notifications History [{} in progress] ", in_progress)
    } else {
        " Notifications History ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);

    let inner_area = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if app.notification_manager.notifications.is_empty() {
        let msg = Paragraph::new("No notifications yet")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner_area);
        return;
    }

    // Build table
    let header_cells = [" STATUS", " ACTION", " RESOURCE", " DURATION", " TIME AGO"]
        .iter()
        .map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        });
    let header = Row::new(header_cells).height(1);

    let rows = app.notification_manager.notifications.iter().map(|notif| {
        let (status_icon, status_color) = match &notif.status {
            NotificationStatus::Pending => ("◯", Color::DarkGray),
            NotificationStatus::InProgress => ("↻", Color::Yellow),
            NotificationStatus::Success => ("✓", Color::Green),
            NotificationStatus::Error(_) => ("✗", Color::Red),
        };

        let action = notif.operation_type.display_name();
        let resource = &notif.resource_id;
        let duration = notif.duration_display();
        let time_ago = format_time_ago(notif.created_at.elapsed());

        Row::new(vec![
            Cell::from(format!(" {}", status_icon)).style(Style::default().fg(status_color)),
            Cell::from(format!(" {}", action)),
            Cell::from(format!(" {}", truncate(resource, 30))),
            Cell::from(format!(" {}", duration)),
            Cell::from(format!(" {}", time_ago)),
        ])
    });

    let widths = [
        Constraint::Length(8),
        Constraint::Length(12),
        Constraint::Min(20),
        Constraint::Length(12),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths).header(header).row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = TableState::default();
    state.select(Some(app.notifications_selected));

    f.render_stateful_widget(table, inner_area, &mut state);

    // Render help text at bottom
    let help_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + popup_area.height - 1,
        popup_area.width - 2,
        1,
    );
    let help = Line::from(vec![
        Span::styled("j/k", Style::default().fg(Color::Yellow)),
        Span::raw(": navigate  "),
        Span::styled("c", Style::default().fg(Color::Yellow)),
        Span::raw(": clear all  "),
        Span::styled("q/n/Esc", Style::default().fg(Color::Yellow)),
        Span::raw(": close"),
    ]);
    let help_para = Paragraph::new(help).alignment(Alignment::Center);
    f.render_widget(help_para, help_area);
}

/// Format elapsed time as human-readable string
fn format_time_ago(elapsed: std::time::Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}

/// Truncate string for display
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}
