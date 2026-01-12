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
    let popup_area = centered_rect(75, 85, area);

    f.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        // Navigation section
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  ↑/↓ or j/k      ", Style::default().fg(Color::Yellow)),
            Span::raw("Move up/down"),
        ]),
        Line::from(vec![
            Span::styled("  Home or gg      ", Style::default().fg(Color::Yellow)),
            Span::raw("Go to top"),
        ]),
        Line::from(vec![
            Span::styled("  End or G        ", Style::default().fg(Color::Yellow)),
            Span::raw("Go to bottom"),
        ]),
        Line::from(vec![
            Span::styled("  PgUp/PgDn       ", Style::default().fg(Color::Yellow)),
            Span::raw("Page up/down (or Ctrl+u/d)"),
        ]),
        Line::from(vec![
            Span::styled("  1-9             ", Style::default().fg(Color::Yellow)),
            Span::raw("Jump to item 1-9"),
        ]),
        Line::from(vec![
            Span::styled("  [/]             ", Style::default().fg(Color::Yellow)),
            Span::raw("Previous/next API page"),
        ]),
        Line::from(""),
        // Sorting section
        Line::from(vec![Span::styled(
            "Sorting",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  F1-F6           ", Style::default().fg(Color::Yellow)),
            Span::raw("Sort by column 1-6 (toggle direction)"),
        ]),
        Line::from(vec![
            Span::styled("  F12             ", Style::default().fg(Color::Yellow)),
            Span::raw("Clear sort"),
        ]),
        Line::from(""),
        // Views section
        Line::from(vec![Span::styled(
            "Views",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  Enter or d      ", Style::default().fg(Color::Yellow)),
            Span::raw("View resource details (JSON)"),
        ]),
        Line::from(vec![
            Span::styled("  ← or Backspace  ", Style::default().fg(Color::Yellow)),
            Span::raw("Go back"),
        ]),
        Line::from(vec![
            Span::styled("  R               ", Style::default().fg(Color::Yellow)),
            Span::raw("Refresh current view"),
        ]),
        Line::from(""),
        // Filtering section
        Line::from(vec![Span::styled(
            "Filtering",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  /               ", Style::default().fg(Color::Yellow)),
            Span::raw("Start filtering (searches all columns)"),
        ]),
        Line::from(vec![
            Span::styled("  Esc             ", Style::default().fg(Color::Yellow)),
            Span::raw("Clear filter"),
        ]),
        Line::from(""),
        // Selectors section
        Line::from(vec![Span::styled(
            "Selectors",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  p               ", Style::default().fg(Color::Yellow)),
            Span::raw("Switch project (type to search)"),
        ]),
        Line::from(vec![
            Span::styled("  z               ", Style::default().fg(Color::Yellow)),
            Span::raw("Switch zone (type to search)"),
        ]),
        Line::from(vec![
            Span::styled("  :               ", Style::default().fg(Color::Yellow)),
            Span::raw("Command mode (type resource name)"),
        ]),
        Line::from(""),
        // Actions section
        Line::from(vec![Span::styled(
            "Actions (VM Instances)",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  s               ", Style::default().fg(Color::Yellow)),
            Span::raw("Start instance"),
        ]),
        Line::from(vec![
            Span::styled("  S               ", Style::default().fg(Color::Yellow)),
            Span::raw("Stop instance"),
        ]),
        Line::from(vec![
            Span::styled("  r               ", Style::default().fg(Color::Yellow)),
            Span::raw("Reset instance"),
        ]),
        Line::from(vec![
            Span::styled("  x               ", Style::default().fg(Color::Green)),
            Span::raw("SSH to instance"),
        ]),
        Line::from(vec![
            Span::styled("  X               ", Style::default().fg(Color::Green)),
            Span::raw("SSH via IAP tunnel"),
        ]),
        Line::from(vec![
            Span::styled("  C               ", Style::default().fg(Color::Green)),
            Span::raw("Open in GCP Console"),
        ]),
        Line::from(vec![
            Span::styled("  Delete          ", Style::default().fg(Color::Red)),
            Span::raw("Delete resource (destructive)"),
        ]),
        Line::from(""),
        // Commands section
        Line::from(vec![Span::styled(
            "Commands (:)",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  :theme <name>   ", Style::default().fg(Color::Yellow)),
            Span::raw("Switch theme (dracula, monokai, nord...)"),
        ]),
        Line::from(vec![
            Span::styled("  :alias a b      ", Style::default().fg(Color::Yellow)),
            Span::raw("Create alias 'a' for resource 'b'"),
        ]),
        Line::from(""),
        // General section
        Line::from(vec![Span::styled(
            "General",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  ?               ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle help"),
        ]),
        Line::from(vec![
            Span::styled("  q               ", Style::default().fg(Color::Yellow)),
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
