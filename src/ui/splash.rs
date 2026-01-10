//! Splash Screen
//!
//! Loading screen shown during initialization.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

/// Splash screen state
pub struct SplashState {
    message: String,
    completed_steps: usize,
    total_steps: usize,
}

impl SplashState {
    pub fn new() -> Self {
        Self {
            message: "Initializing...".to_string(),
            completed_steps: 0,
            total_steps: 5,
        }
    }

    pub fn set_message(&mut self, message: &str) {
        self.message = message.to_string();
    }

    pub fn complete_step(&mut self) {
        self.completed_steps = (self.completed_steps + 1).min(self.total_steps);
    }

    fn progress(&self) -> f64 {
        self.completed_steps as f64 / self.total_steps as f64
    }
}

impl Default for SplashState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render(f: &mut Frame, state: &SplashState) {
    let area = f.area();

    // Center the splash content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Length(10),
            Constraint::Percentage(35),
        ])
        .split(area);

    let center = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(chunks[1])[1];

    // Logo/Title
    let logo = vec![
        Line::from(Span::styled(
            "  _              ",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            " | |_ __ _  ___ _ __  ",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            " | __/ _` |/ __| '_ \\ ",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            " | || (_| | (__| |_) |",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "  \\__\\__, |\\___|  __/ ",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "     |___/     |_|    ",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Terminal UI for Google Cloud Platform",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
    ];

    let logo_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = logo_block.inner(center);
    f.render_widget(logo_block, center);

    // Split inner area for logo and progress
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(inner);

    let logo_para = Paragraph::new(logo).alignment(Alignment::Center);
    f.render_widget(logo_para, inner_chunks[0]);

    // Progress bar
    let progress = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan))
        .percent((state.progress() * 100.0) as u16)
        .label(Span::styled(
            &state.message,
            Style::default().fg(Color::White),
        ));

    f.render_widget(progress, inner_chunks[1]);
}
