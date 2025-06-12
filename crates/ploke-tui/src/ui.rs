use ratatui::widgets::{Clear, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::prelude::Color;
use ratatui::{
    backend::Backend,
    layout::Rect,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style, Stylize},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

/// Helper to create centered rects for modal dialogs
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

use crate::app::{App, ModalType, Mode};

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
    // Render the block first to get its inner area for content + scrollbar
    f.render_widget(history_block.clone(), chunks[0]);
    let history_inner_area = history_block.inner(chunks[0]);

    // Create a new horizontal sub-layout within history_inner_area
    let history_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(history_inner_area);

    let text_area = history_chunks[0];
    let scrollbar_area = history_chunks[1];

    let messages: Vec<ratatui::text::Line> = app
        .messages
        .iter()
        .map(|m| ratatui::text::Line::from(m.clone()))
        .collect();

    let history_paragraph = Paragraph::new(messages)
        // No block here if the outer block is already rendered
        .wrap(Wrap { trim: false })
        .scroll((app.history_scroll_offset as u16, 0));

    f.render_widget(history_paragraph, text_area); // Render paragraph in its dedicated area

    // Create and render the Scrollbar
    // Only show scrollbar if content might be larger than the available space.
    // This is a heuristic, as precise line count after wrapping is complex.
    if app.messages.len() > text_area.height as usize {
        let mut scrollbar_state = ScrollbarState::new(app.messages.len())
            .position(app.history_scroll_offset);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
            .thumb_symbol("█");

        f.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }

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

    // Render modals on top
    if let Some(modal) = app.active_modals.last() {
        let dialog = match modal {
            ModalType::QuitConfirm => Paragraph::new("Quit application? (y/n)")
                .block(Block::default().title("Confirm Quit").borders(Borders::ALL))
                .style(Style::default().fg(Color::Yellow)),
        };
        
        let area = centered_rect(60, 20, f.size());
        f.render_widget(Clear, area); // Clear background
        f.render_widget(dialog, area);
    }

    // Set cursor position if in input mode
    if app.mode == Mode::Input {
        f.set_cursor(
            chunks[1].x + app.current_input.len() as u16 + 1, // +1 for border
            chunks[1].y + 1, // +1 for border
        );
    }
}
