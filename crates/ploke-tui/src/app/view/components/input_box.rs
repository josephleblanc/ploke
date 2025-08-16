use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, ScrollbarState};
use ratatui::Frame;
use textwrap;
use crate::app::types::Mode;
use crate::app::view::EventSubscriber;
use crate::app::AppEvent;

/// Encapsulates input box rendering and state (scroll, cursor).
#[derive(Debug, Clone, Default)]
pub struct InputView {
    vscroll: u16,
    scrollstate: ScrollbarState,
    cursor_row: u16,
    cursor_col: u16,
    trailing_whitespace: bool,
}

impl InputView {
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        buffer: &str,
        mode: Mode,
        title: &str,
    ) {
        // Wrap text to area width minus borders
        let input_width = area.width.saturating_sub(2);
        let input_wrapped = textwrap::wrap(buffer, input_width as usize);
        self.scrollstate = self.scrollstate.content_length(input_wrapped.len());

        // Update cursor position
        self.trailing_whitespace = buffer.chars().last().is_some_and(|c| c == ' ');
        if !self.trailing_whitespace {
            self.cursor_col = input_wrapped
                .last()
                .map(|line| line.len())
                .unwrap_or(0) as u16;
        }
        self.cursor_row = (input_wrapped.len().saturating_sub(1)) as u16;

        // Build paragraph
        let input_text = Text::from_iter(input_wrapped);
        let input = Paragraph::new(input_text)
            .scroll((self.vscroll, 0))
            .block(Block::bordered().title(title))
            .style(match mode {
                Mode::Normal => Style::default(),
                Mode::Insert => Style::default().fg(Color::Yellow),
                Mode::Command => Style::default().fg(Color::Cyan),
            });

        frame.render_widget(input, area);

        // Manage cursor visibility/position
        match mode {
            Mode::Insert | Mode::Command => {
                frame.set_cursor_position((area.x + 1 + self.cursor_col, area.y + 1 + self.cursor_row));
            }
            Mode::Normal => {
                // No cursor positioning => hidden by terminal backend
            }
        }
    }

    pub fn scroll_prev(&mut self) {
        self.scrollstate.prev();
    }

    pub fn scroll_next(&mut self) {
        self.scrollstate.next();
    }
}

impl EventSubscriber for InputView {
    fn on_event(&mut self, _event: &AppEvent) {
        // Currently no-op; placeholder for future input-related reactions to events.
    }
}
