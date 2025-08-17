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

        // Update cursor position: include trailing spaces and wrapping
        let input_lines = input_wrapped.len() as u16;
        let base_last_len: u16 = input_wrapped
            .last()
            .map(|line| line.chars().count() as u16)
            .unwrap_or(0);

        // Count trailing spaces only after the last explicit newline
        let tail_segment = buffer.rsplit('\n').next().unwrap_or("");
        let trailing_spaces: u16 = tail_segment
            .chars()
            .rev()
            .take_while(|&c| c == ' ')
            .count() as u16;

        // Start from the wrapped baseline, then add trailing spaces with width-based wrapping.
        let mut row = input_lines.saturating_sub(1);
        let mut col = base_last_len;

        if input_width > 0 {
            let total = base_last_len.saturating_add(trailing_spaces);
            row = row.saturating_add(total / input_width);
            col = total % input_width;
        }

        self.cursor_row = row;
        self.cursor_col = col;

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
