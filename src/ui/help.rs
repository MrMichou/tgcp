//! Help Overlay
//!
//! Shows keyboard shortcuts and help information.

use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, _app: &App) {
    let area = f.area();
    let popup_area = centered_rect(70, 80, area);

    f.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Navigation", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  j/k, ↑/↓    ", Style::default().fg(Color::Yellow)),
            Span::raw("Move up/down"),
        ]),
        Line::from(vec![
            Span::styled("  gg          ", Style::default().fg(Color::Yellow)),
            Span::raw("Go to top"),
        ]),
        Line::from(vec![
            Span::styled("  G           ", Style::default().fg(Color::Yellow)),
            Span::raw("Go to bottom"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+d/u    ", Style::default().fg(Color::Yellow)),
            Span::raw("Page down/up"),
        ]),
        Line::from(vec![
            Span::styled("  [/]         ", Style::default().fg(Color::Yellow)),
            Span::raw("Previous/next page"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Views", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Enter/d     ", Style::default().fg(Color::Yellow)),
            Span::raw("View resource details"),
        ]),
        Line::from(vec![
            Span::styled("  b/Backspace ", Style::default().fg(Color::Yellow)),
            Span::raw("Go back"),
        ]),
        Line::from(vec![
            Span::styled("  R           ", Style::default().fg(Color::Yellow)),
            Span::raw("Refresh current view"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Filtering", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  /           ", Style::default().fg(Color::Yellow)),
            Span::raw("Start filtering"),
        ]),
        Line::from(vec![
            Span::styled("  Esc         ", Style::default().fg(Color::Yellow)),
            Span::raw("Clear filter"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Commands", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  :           ", Style::default().fg(Color::Yellow)),
            Span::raw("Enter command mode"),
        ]),
        Line::from(vec![
            Span::styled("  p           ", Style::default().fg(Color::Yellow)),
            Span::raw("Switch project"),
        ]),
        Line::from(vec![
            Span::styled("  z           ", Style::default().fg(Color::Yellow)),
            Span::raw("Switch zone"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Actions", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  s           ", Style::default().fg(Color::Yellow)),
            Span::raw("Start instance"),
        ]),
        Line::from(vec![
            Span::styled("  S           ", Style::default().fg(Color::Yellow)),
            Span::raw("Stop instance"),
        ]),
        Line::from(vec![
            Span::styled("  r           ", Style::default().fg(Color::Yellow)),
            Span::raw("Reset instance"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+d      ", Style::default().fg(Color::Red)),
            Span::raw("Delete resource (destructive)"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ?/Esc       ", Style::default().fg(Color::Yellow)),
            Span::raw("Close help"),
        ]),
        Line::from(vec![
            Span::styled("  q           ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit application"),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Help ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .alignment(Alignment::Left);

    f.render_widget(paragraph, popup_area);
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
