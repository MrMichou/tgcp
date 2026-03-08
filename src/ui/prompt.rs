//! Prompt Dialog
//!
//! Text input dialog for actions that require user input.

use super::centered_rect;
use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let Some(prompt) = &app.prompt_state else {
        return;
    };

    let area = f.area();
    let popup_area = centered_rect(50, 25, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Input Required ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Split hint into separate lines on " | "
    let hint_lines: Vec<&str> = if prompt.hint.is_empty() {
        vec![]
    } else {
        prompt.hint.split(" | ").collect()
    };
    let hint_height = hint_lines.len().max(1) as u16;

    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),           // Title/prompt text
            Constraint::Length(hint_height), // Hint lines
            Constraint::Length(1),           // Spacer
            Constraint::Length(3),           // Input box
            Constraint::Length(1),           // Help text
        ])
        .split(inner);

    // Prompt text
    let title = Paragraph::new(Line::from(Span::styled(
        &prompt.title,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center);
    f.render_widget(title, content_chunks[0]);

    // Hint lines (current value, disks, warnings)
    if !hint_lines.is_empty() {
        let lines: Vec<Line> = hint_lines
            .iter()
            .map(|line| {
                let color = if line.starts_with("Warning") {
                    Color::Yellow
                } else {
                    Color::DarkGray
                };
                Line::from(Span::styled(line.to_string(), Style::default().fg(color)))
            })
            .collect();
        let hint = Paragraph::new(lines).alignment(Alignment::Center);
        f.render_widget(hint, content_chunks[1]);
    }

    // Input box
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let input_text = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {}", prompt.input),
            Style::default().fg(Color::White),
        ),
        Span::styled("_", Style::default().fg(Color::Yellow)),
    ]))
    .block(input_block);
    f.render_widget(input_text, content_chunks[3]);

    // Help text
    let help = Paragraph::new(Line::from(Span::styled(
        "Enter: confirm | Esc: cancel",
        Style::default().fg(Color::DarkGray),
    )))
    .alignment(Alignment::Center);
    f.render_widget(help, content_chunks[4]);
}

