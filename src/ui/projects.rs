//! Projects Selector
//!
//! Project selection overlay with search functionality.

use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(60, 70, area);
    f.render_widget(Clear, popup_area);

    // Title with count
    let title = format!(
        " Select Project [{}/{}] ",
        app.projects_filtered.len(),
        app.available_projects.len()
    );

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

    // Split inner into: search box, help text, separator, list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Search input
            Constraint::Length(1), // Help text
            Constraint::Length(1), // Separator
            Constraint::Min(1),    // Project list
        ])
        .split(inner);

    // Search input with cursor
    let search_line = Line::from(vec![
        Span::styled(" / ", Style::default().fg(Color::Yellow)),
        Span::styled(&app.projects_search_text, Style::default().fg(Color::White)),
        Span::styled("_", Style::default().fg(Color::Yellow)),
    ]);
    f.render_widget(
        Paragraph::new(search_line).style(Style::default()),
        chunks[0],
    );

    // Help text
    let help = Line::from(vec![
        Span::styled(" Type", Style::default().fg(Color::DarkGray)),
        Span::styled(" to search", Style::default().fg(Color::DarkGray)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::styled(":nav ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(":select ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(":cancel", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(help), chunks[1]);

    // Separator line
    let sep = "─".repeat(chunks[2].width as usize);
    f.render_widget(
        Paragraph::new(sep).style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );

    // Filtered project list
    let items: Vec<ListItem> = app
        .projects_filtered
        .iter()
        .map(|project| {
            let style = if project == &app.project {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            // Mark current project with a checkmark
            let prefix = if project == &app.project {
                "✓ "
            } else {
                "  "
            };
            ListItem::new(Span::styled(format!("{}{}", prefix, project), style))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    state.select(Some(app.projects_selected));

    f.render_stateful_widget(list, chunks[3], &mut state);
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
