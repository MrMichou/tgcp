//! Command Box
//!
//! Command input with autocomplete.

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    // Command box at bottom of screen
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(12)])
        .split(area);

    let command_area = chunks[1];

    f.render_widget(Clear, command_area);

    // Split into input and suggestions
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(command_area);

    // Input box
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Command ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    // Show input with ghost text preview
    let input_content = if let Some(preview) = &app.command_preview {
        if app.command_text.is_empty() {
            Line::from(vec![
                Span::styled(":", Style::default().fg(Color::Cyan)),
                Span::styled(preview, Style::default().fg(Color::DarkGray)),
            ])
        } else if preview.starts_with(&app.command_text) {
            let remaining = &preview[app.command_text.len()..];
            Line::from(vec![
                Span::styled(":", Style::default().fg(Color::Cyan)),
                Span::styled(&app.command_text, Style::default().fg(Color::White)),
                Span::styled(remaining, Style::default().fg(Color::DarkGray)),
            ])
        } else {
            Line::from(vec![
                Span::styled(":", Style::default().fg(Color::Cyan)),
                Span::styled(&app.command_text, Style::default().fg(Color::White)),
            ])
        }
    } else {
        Line::from(vec![
            Span::styled(":", Style::default().fg(Color::Cyan)),
            Span::styled(&app.command_text, Style::default().fg(Color::White)),
        ])
    };

    let input_para = Paragraph::new(input_content).block(input_block);
    f.render_widget(input_para, inner_chunks[0]);

    // Suggestions list
    let suggestions_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Suggestions (↑/↓ to select, Tab to complete) ",
            Style::default().fg(Color::DarkGray),
        ));

    let suggestions: Vec<ListItem> = app
        .command_suggestions
        .iter()
        .enumerate()
        .take(8)
        .map(|(i, cmd)| {
            let style = if i == app.command_suggestion_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Span::styled(format!("  {}", cmd), style))
        })
        .collect();

    let suggestions_list = List::new(suggestions).block(suggestions_block);
    f.render_widget(suggestions_list, inner_chunks[1]);
}
