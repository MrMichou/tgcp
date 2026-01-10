//! Dialog Components
//!
//! Confirmation and warning dialogs.

use crate::app::{App, Mode};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    match app.mode {
        Mode::Confirm => render_confirm_dialog(f, app),
        Mode::Warning => render_warning_dialog(f, app),
        _ => {},
    }
}

fn render_confirm_dialog(f: &mut Frame, app: &App) {
    let Some(pending) = &app.pending_action else {
        return;
    };

    let area = f.area();
    let popup_area = centered_rect(50, 25, area);

    f.render_widget(Clear, popup_area);

    let border_color = if pending.destructive {
        Color::Red
    } else {
        Color::Yellow
    };

    let title = if pending.destructive {
        " Confirm Destructive Action "
    } else {
        " Confirm Action "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Content
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(inner);

    // Message
    let message = Paragraph::new(Line::from(Span::styled(
        &pending.message,
        Style::default().fg(Color::White),
    )))
    .alignment(Alignment::Center);
    f.render_widget(message, content_chunks[0]);

    // Buttons
    let yes_style = if pending.selected_yes {
        Style::default()
            .fg(Color::Black)
            .bg(if pending.destructive {
                Color::Red
            } else {
                Color::Green
            })
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let no_style = if !pending.selected_yes {
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let buttons = Line::from(vec![
        Span::raw("  "),
        Span::styled(" Yes (y) ", yes_style),
        Span::raw("    "),
        Span::styled(" No (n) ", no_style),
        Span::raw("  "),
    ]);

    let buttons_para = Paragraph::new(buttons).alignment(Alignment::Center);
    f.render_widget(buttons_para, content_chunks[2]);
}

fn render_warning_dialog(f: &mut Frame, app: &App) {
    let Some(message) = &app.warning_message else {
        return;
    };

    let area = f.area();
    let popup_area = centered_rect(50, 20, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Span::styled(
            " Warning ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            message.as_str(),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press Enter or Esc to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(content).alignment(Alignment::Center);
    f.render_widget(paragraph, inner);
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
