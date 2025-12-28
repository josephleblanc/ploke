use crate::app::AppEvent;
use crate::app::types::Mode;
use crate::app::view::EventSubscriber;
use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, ScrollbarState};
use textwrap;

/// Encapsulates input box rendering and state (scroll, cursor).
#[derive(Debug, Clone, Default)]
pub struct InputView {
    vscroll: u16,
    scrollstate: ScrollbarState,
    cursor_row: u16,
    cursor_col: u16,
}

impl InputView {
    pub fn desired_height(&self, buffer: &str, area_width: u16) -> u16 {
        let inner_width = area_width.saturating_sub(2).max(1);
        let wrapped = textwrap::wrap(buffer, inner_width as usize);
        wrapped.len().max(1) as u16 + 2
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        buffer: &str,
        mode: Mode,
        _title: &str,
    ) {
        let inner_area = area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        // Wrap text to inner area width
        let input_width = inner_area.width.max(1);
        let input_wrapped = textwrap::wrap(buffer, input_width as usize);
        // scrollstate updated after auto-scroll below

        // Update cursor position: include trailing spaces and wrapping
        let input_lines = input_wrapped.len() as u16;
        let base_last_len: u16 = input_wrapped
            .last()
            .map(|line| line.chars().count() as u16)
            .unwrap_or(0);

        // Count trailing spaces only after the last explicit newline
        let tail_segment = buffer.rsplit('\n').next().unwrap_or("");
        let trailing_spaces: u16 =
            tail_segment.chars().rev().take_while(|&c| c == ' ').count() as u16;

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

        // Auto-scroll to keep cursor visible and clamp within content
        let inner_h: u16 = inner_area.height.max(1);
        let total_lines: u16 = input_wrapped.len() as u16;

        // Ensure cursor is within the visible window
        if self.cursor_row >= self.vscroll.saturating_add(inner_h) {
            self.vscroll = self.cursor_row.saturating_sub(inner_h).saturating_add(1);
        }
        if self.cursor_row < self.vscroll {
            self.vscroll = self.cursor_row;
        }

        // Clamp vscroll to valid range based on content height
        if total_lines > inner_h {
            self.vscroll = self.vscroll.min(total_lines.saturating_sub(inner_h));
        } else {
            self.vscroll = 0;
        }

        // Keep scrollbar state in sync (even if not rendered yet)
        self.scrollstate = self
            .scrollstate
            .content_length(input_wrapped.len())
            .position(self.vscroll as usize);

        // Build paragraph
        let input_text = Text::from_iter(input_wrapped);
        let background_style = Style::default().bg(Color::Rgb(90, 90, 90));
        let input_style = match mode {
            Mode::Command => background_style.fg(Color::Blue),
            _ => background_style.fg(Color::Rgb(220, 220, 220)),
        };
        let background = Block::default().borders(Borders::NONE).style(background_style);
        let input = Paragraph::new(input_text)
            .scroll((self.vscroll, 0))
            .block(Block::default().borders(Borders::NONE))
            .style(input_style);

        frame.render_widget(background, area);
        frame.render_widget(input, inner_area);

        // Manage cursor visibility/position
        match mode {
            Mode::Insert | Mode::Command => {
                let visible_row = self.cursor_row.saturating_sub(self.vscroll);
                frame.set_cursor_position((
                    inner_area.x + self.cursor_col,
                    inner_area.y + visible_row,
                ));
            }
            Mode::Normal => {
                // No cursor positioning => hidden by terminal backend
            }
        }
    }

    pub fn scroll_prev(&mut self) {
        self.vscroll = self.vscroll.saturating_sub(1);
        self.scrollstate = self.scrollstate.position(self.vscroll as usize);
    }

    pub fn scroll_next(&mut self) {
        self.vscroll = self.vscroll.saturating_add(1);
        self.scrollstate = self.scrollstate.position(self.vscroll as usize);
    }
}

impl EventSubscriber for InputView {
    fn on_event(&mut self, _event: &AppEvent) {
        // Currently no-op; placeholder for future input-related reactions to events.
    }
}

#[cfg(test)]
mod tests {
    use super::InputView;

    #[test]
    fn desired_height_is_single_line_for_empty_buffer() {
        let view = InputView::default();
        assert_eq!(view.desired_height("", 10), 3);
    }

    #[test]
    fn desired_height_wraps_long_lines() {
        let view = InputView::default();
        assert_eq!(view.desired_height("abcdefgh", 6), 4);
    }

    #[test]
    fn desired_height_counts_explicit_newlines() {
        let view = InputView::default();
        assert_eq!(view.desired_height("a\nb\nc", 10), 5);
    }
}
