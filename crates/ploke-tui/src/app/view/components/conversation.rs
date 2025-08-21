use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::AppEvent;
use crate::app::message_item::{measure_messages, render_messages};
use crate::app::types::RenderMsg;
use crate::app::view::EventSubscriber;

/// Encapsulates conversation view state: scroll, auto-follow, and item heights.
#[derive(Debug, Default, Clone)]
pub struct ConversationView {
    offset_y: u16,
    content_height: u16,
    item_heights: Vec<u16>,
    auto_follow: bool,
    free_scrolling: bool,
    last_viewport_height: u16,
    last_chat_area: Rect,
}

impl ConversationView {
    pub fn prepare<'a, I, T: RenderMsg + 'a>(
        &mut self,
        path: I,
        path_len: usize,
        conversation_width: u16,
        viewport_height: u16,
        selected_index_opt: Option<usize>,
    ) where
        I: IntoIterator<Item = &'a T>,
    {
        self.last_viewport_height = viewport_height;

        // 1) Measure
        let (total_height, heights) =
            measure_messages(path, conversation_width, selected_index_opt);
        self.content_height = total_height;
        self.item_heights = heights;

        // 2) Decide/adjust offset using current metrics
        let max_offset = self.content_height.saturating_sub(viewport_height);

        if path_len == 0 {
            self.offset_y = 0;
            self.auto_follow = true;
            self.free_scrolling = false;
        } else if let Some(selected_index) = selected_index_opt {
            let is_last = selected_index + 1 == path_len;
            if is_last {
                if self.auto_follow && !self.free_scrolling {
                    self.offset_y = max_offset;
                }
            } else if !self.free_scrolling {
                // Exit auto-follow when navigating to a non-last message and minimally reveal selection
                self.auto_follow = false;

                // Minimally reveal selection within current viewport
                let mut prefix_sum = 0u16;
                for (i, h) in self.item_heights.iter().enumerate() {
                    if i == selected_index {
                        break;
                    }
                    prefix_sum = prefix_sum.saturating_add(*h);
                }
                let selected_top = prefix_sum;
                let selected_bottom = prefix_sum.saturating_add(self.item_heights[selected_index]);
                let viewport_bottom = self.offset_y.saturating_add(viewport_height);

                if selected_top < self.offset_y {
                    self.offset_y = selected_top;
                } else if selected_bottom > viewport_bottom {
                    self.offset_y = selected_bottom.saturating_sub(viewport_height);
                }
            }
        } else {
            // No explicit selection; keep existing offset (will clamp below)
        }

        // Clamp offset
        if self.offset_y > max_offset {
            self.offset_y = max_offset;
        }

        // 3) Auto-follow status
        if !self.free_scrolling {
            if let Some(selected_index) = selected_index_opt {
                let is_last = selected_index + 1 == path_len;
                self.auto_follow = is_last || self.offset_y >= max_offset;
            } else {
                self.auto_follow = self.offset_y >= max_offset;
            }
        }
    }

    pub fn render<'a, I, T: RenderMsg + 'a>(
        &self,
        frame: &mut Frame,
        path: I,
        conversation_width: u16,
        conversation_area: Rect,
        selected_index_opt: Option<usize>,
    ) where
        I: IntoIterator<Item = &'a T>,
    {
        render_messages(
            frame,
            path,
            conversation_width,
            conversation_area,
            self.offset_y,
            &self.item_heights,
            selected_index_opt,
        );
    }

    pub fn set_last_chat_area(&mut self, area: Rect) {
        self.last_chat_area = area;
    }

    pub fn last_chat_area(&self) -> Rect {
        self.last_chat_area
    }

    pub fn item_heights(&self) -> &Vec<u16> {
        &self.item_heights
    }

    pub fn offset(&self) -> u16 {
        self.offset_y
    }

    pub fn set_free_scrolling(&mut self, val: bool) {
        self.free_scrolling = val;
    }

    pub fn scroll_line_down(&mut self) {
        self.scroll_lines_down(1);
    }

    pub fn scroll_line_up(&mut self) {
        self.scroll_lines_up(1);
    }

    pub fn scroll_lines_down(&mut self, lines: u16) {
        let max_offset = self
            .content_height
            .saturating_sub(self.last_viewport_height);
        let new_offset = self.offset_y.saturating_add(lines);
        self.offset_y = new_offset.min(max_offset);
    }

    pub fn scroll_lines_up(&mut self, lines: u16) {
        self.offset_y = self.offset_y.saturating_sub(lines);
    }

    pub fn page_down(&mut self) {
        let vh = self.last_viewport_height.max(1);
        let page_step: u16 = (vh / 10).clamp(1, 5);
        self.scroll_lines_down(page_step);
    }

    pub fn page_up(&mut self) {
        let vh = self.last_viewport_height.max(1);
        let page_step: u16 = (vh / 10).clamp(1, 5);
        self.scroll_lines_up(page_step);
    }

    pub fn request_bottom(&mut self) {
        // Will clamp in prepare()
        self.offset_y = u16::MAX;
    }

    pub fn request_top(&mut self) {
        self.offset_y = 0;
    }
}

impl EventSubscriber for ConversationView {
    fn on_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::MessageUpdated(_) => {
                // When a message is updated/added, request a snap-to-bottom if either:
                // - auto-follow is enabled (user is following the tail), or
                // - we're in free-scrolling (typing/editing) to reveal full new content.
                if self.auto_follow || self.free_scrolling {
                    self.request_bottom();
                }
            }
            _ => {}
        }
    }
}
