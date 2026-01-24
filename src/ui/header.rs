//! Header Component
//!
//! Displays project, zone, and context information.

use crate::app::App;
use crate::VERSION;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            format!(" tgcp v{} ", VERSION),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split into rows
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    // Row 1: Project and Zone
    let project_zone = Line::from(vec![
        Span::styled(" Project: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            &app.project,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("Zone: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if app.zone == "all" {
                "All zones"
            } else {
                &app.zone
            },
            Style::default()
                .fg(if app.zone == "all" {
                    Color::Yellow
                } else {
                    Color::Green
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(project_zone), rows[0]);

    // Row 2: Current resource and count
    let resource_info = if let Some(resource) = app.current_resource() {
        Line::from(vec![
            Span::styled(" Resource: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &resource.display_name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Count: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", app.filtered_items.len()),
                Style::default().fg(Color::White),
            ),
            if app.items.len() != app.filtered_items.len() {
                Span::styled(
                    format!(" (filtered from {})", app.items.len()),
                    Style::default().fg(Color::DarkGray),
                )
            } else {
                Span::raw("")
            },
        ])
    } else {
        Line::from(vec![Span::styled(
            " No resource selected",
            Style::default().fg(Color::Red),
        )])
    };
    f.render_widget(Paragraph::new(resource_info), rows[1]);

    // Row 3: Actions (if available)
    let actions_line = if let Some(resource) = app.current_resource() {
        if !resource.actions.is_empty() {
            let action_hints: Vec<Span> = resource
                .actions
                .iter()
                .filter_map(|a| {
                    a.shortcut.as_ref().map(|s| {
                        Span::styled(
                            format!(" [{}]{} ", s, a.display_name),
                            if a.confirm.as_ref().map(|c| c.destructive).unwrap_or(false) {
                                Style::default().fg(Color::Red)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            },
                        )
                    })
                })
                .collect();

            if action_hints.is_empty() {
                Line::from(Span::raw(""))
            } else {
                let mut spans = vec![Span::styled(
                    " Actions:",
                    Style::default().fg(Color::DarkGray),
                )];
                spans.extend(action_hints);
                Line::from(spans)
            }
        } else {
            Line::from(Span::raw(""))
        }
    } else {
        Line::from(Span::raw(""))
    };
    f.render_widget(Paragraph::new(actions_line), rows[2]);

    // Row 4: Help hint - more accessible with clear labels
    let mut help_spans = vec![
        Span::styled(" ?", Style::default().fg(Color::Yellow)),
        Span::styled(":help ", Style::default().fg(Color::DarkGray)),
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::styled(":nav ", Style::default().fg(Color::DarkGray)),
        Span::styled("/", Style::default().fg(Color::Yellow)),
        Span::styled(":filter ", Style::default().fg(Color::DarkGray)),
        Span::styled("p", Style::default().fg(Color::Yellow)),
        Span::styled(":proj ", Style::default().fg(Color::DarkGray)),
        Span::styled("z", Style::default().fg(Color::Yellow)),
        Span::styled(":zone ", Style::default().fg(Color::DarkGray)),
        Span::styled("F1-F6", Style::default().fg(Color::Yellow)),
        Span::styled(":sort ", Style::default().fg(Color::DarkGray)),
    ];

    // Add sort indicator if active
    if let Some(col) = app.filter_sort.sort_column {
        help_spans.push(Span::styled(
            format!(
                "[sorted:col{}{}]",
                col + 1,
                if app.filter_sort.sort_ascending {
                    "↑"
                } else {
                    "↓"
                }
            ),
            Style::default().fg(Color::Cyan),
        ));
    }

    if app.readonly {
        help_spans.push(Span::styled(
            " [READ-ONLY]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let help_line = Line::from(help_spans);
    f.render_widget(Paragraph::new(help_line), rows[3]);
}
