//! Column Configuration Overlay
//!
//! Allows users to show/hide columns for the current resource type.

use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 60, area);
    f.render_widget(Clear, popup_area);

    // Get resource display name for title
    let resource_name = app
        .current_resource()
        .map(|r| r.display_name.as_str())
        .unwrap_or("Resource");

    let title = format!(" Configure Columns: {} ", resource_name);

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

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Split inner into: help text, separator, list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Help text
            Constraint::Length(1), // Separator
            Constraint::Min(1),    // Column list
        ])
        .split(inner);

    // Help text
    let help = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled("j/k", Style::default().fg(Color::Yellow)),
        Span::styled(":nav ", Style::default().fg(Color::DarkGray)),
        Span::styled("Space", Style::default().fg(Color::Yellow)),
        Span::styled(":toggle ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(":apply ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(":cancel", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(help), chunks[0]);

    // Separator line
    let sep = "─".repeat(chunks[1].width as usize);
    f.render_widget(
        Paragraph::new(sep).style(Style::default().fg(Color::DarkGray)),
        chunks[1],
    );

    // Column list with checkboxes
    let Some(ref state) = app.column_config_state else {
        return;
    };

    // Count visible columns to show warning when only one is left
    let visible_count = state.columns.iter().filter(|c| c.visible).count();

    let items: Vec<ListItem> = state
        .columns
        .iter()
        .map(|col| {
            let checkbox = if col.visible { "[x]" } else { "[ ]" };

            let checkbox_style = if col.visible {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let text_style = if col.visible {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            // Show warning if this is the last visible column
            let warning = if col.visible && visible_count == 1 {
                Span::styled(" (required)", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", checkbox), checkbox_style),
                Span::styled(&col.header, text_style),
                warning,
            ]))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected));

    f.render_stateful_widget(list, chunks[2], &mut list_state);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
