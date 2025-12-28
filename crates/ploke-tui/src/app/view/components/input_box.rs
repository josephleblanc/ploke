use crate::app::AppEvent;
use crate::ui_theme::UiTheme;
use crate::app::types::Mode;
use crate::app::view::EventSubscriber;
use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
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

#[derive(Debug, Clone)]
pub struct CommandSuggestion {
    pub command: String,
    pub description: String,
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
        theme: &UiTheme,
        ghost_text: Option<&str>,
        suggestions: &[CommandSuggestion],
    ) {
        let desired_input_height = self.desired_height(buffer, area.width);
        let input_height = desired_input_height.min(area.height);
        let suggestion_height = area.height.saturating_sub(input_height);
        let suggestion_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: suggestion_height,
        };
        let input_area = Rect {
            x: area.x,
            y: area.y.saturating_add(suggestion_height),
            width: area.width,
            height: input_height,
        };
        let inner_area = input_area.inner(Margin {
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
        let background_style = Style::default().bg(theme.input_bg);
        let input_style = match mode {
            Mode::Command => background_style.fg(theme.input_command_fg),
            _ => background_style.fg(theme.input_fg),
        };
        let background = Block::default().borders(Borders::NONE).style(background_style);
        let input = Paragraph::new(input_text)
            .scroll((self.vscroll, 0))
            .block(Block::default().borders(Borders::NONE))
            .style(input_style);

        frame.render_widget(background, input_area);
        frame.render_widget(input, inner_area);

        if suggestion_area.height > 0 && !suggestions.is_empty() {
            let suggestion_style = Style::default()
                .bg(theme.input_suggestion_bg)
                .fg(theme.input_suggestion_fg);
            let desc_style = Style::default()
                .bg(theme.input_suggestion_bg)
                .fg(theme.input_suggestion_desc_fg);
            let mut lines: Vec<Line> = Vec::new();
            for suggestion in suggestions.iter().take(suggestion_area.height as usize) {
                lines.push(Line::from(vec![
                    Span::styled(
                        suggestion.command.as_str(),
                        suggestion_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        suggestion.description.as_str(),
                        desc_style.add_modifier(Modifier::DIM),
                    ),
                ]));
            }
            let suggestion_text = Text::from(lines);
            let suggestion_block = Block::default()
                .borders(Borders::NONE)
                .style(Style::default().bg(theme.input_suggestion_bg));
            let suggestion_widget = Paragraph::new(suggestion_text)
                .block(suggestion_block)
                .style(Style::default().bg(theme.input_suggestion_bg));
            frame.render_widget(suggestion_widget, suggestion_area);
        }

        if let Some(ghost) = ghost_text {
            if !ghost.is_empty() {
                let visible_row = self.cursor_row.saturating_sub(self.vscroll);
                if visible_row < inner_area.height {
                    let ghost_area = Rect {
                        x: inner_area.x.saturating_add(self.cursor_col),
                        y: inner_area.y.saturating_add(visible_row),
                        width: inner_area.width.saturating_sub(self.cursor_col),
                        height: 1,
                    };
                    let ghost_widget = Paragraph::new(ghost)
                        .style(Style::default().fg(theme.input_ghost_fg).bg(theme.input_bg));
                    frame.render_widget(ghost_widget, ghost_area);
                }
            }
        }

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
