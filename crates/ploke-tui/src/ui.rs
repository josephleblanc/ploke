// src/ui.rs
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style, Stylize},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Mode};

/// Renders the application's UI.
pub fn render(f: &mut Frame, app: &App) {
    // Define main layout: history pane, input pane
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
        .split(f.size());

    // Chat History Pane
    let history_block = Block::default()
        .borders(Borders::ALL)
        .title("Chat History");

    let messages: Vec<ratatui::text::Line> = app
        .messages
        .iter()
        .map(|m| ratatui::text::Line::from(m.clone()))
        .collect();

    let history_paragraph = Paragraph::new(messages)
        .block(history_block)
        .wrap(Wrap { trim: false }); // Allow long lines to wrap

    f.render_widget(history_paragraph, chunks[0]);

    // Input Pane
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Input ({:?})", app.mode));

    let input_paragraph = Paragraph::new(app.current_input.as_str())
        .block(input_block)
        .style(match app.mode {
            Mode::Input => Style::default().fg(ratatui::style::Color::Yellow).add_modifier(Modifier::BOLD),
            _ => Style::default(),
        });

    f.render_widget(input_paragraph, chunks[1]);

    // Set cursor position if in input mode
    if app.mode == Mode::Input {
        f.set_cursor(
            chunks[1].x + app.current_input.len() as u16 + 1, // +1 for border
            chunks[1].y + 1, // +1 for border
        );
    }
}
