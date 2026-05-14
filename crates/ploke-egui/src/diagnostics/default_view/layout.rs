use serde::{Deserialize, Serialize};

use crate::ui::app::layout as app_layout;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub top_strip_present: bool,
    pub left_sidebar_present: bool,
    pub center_canvas_present: bool,
    pub right_inspector_present: bool,
    pub bottom_timeline_present: bool,
    pub width_budget: WidthBudget,
}

impl Layout {
    pub(super) fn current() -> Self {
        Self {
            top_strip_present: false,
            left_sidebar_present: true,
            center_canvas_present: true,
            right_inspector_present: false,
            bottom_timeline_present: false,
            width_budget: WidthBudget::current_default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidthBudget {
    pub default_window_width_logical_px: u32,
    pub left_sidebar_width_logical_px: u32,
    pub left_sidebar_max_width_logical_px: u32,
    pub center_canvas_width_logical_px: u32,
    pub min_center_canvas_width_logical_px: u32,
    pub center_canvas_width_percent: u32,
    pub min_center_canvas_width_percent: u32,
}

impl WidthBudget {
    fn current_default() -> Self {
        let default_window_width = logical_px(app_layout::DEFAULT_WINDOW_WIDTH);
        let left_sidebar_width = logical_px(app_layout::LEFT_SIDEBAR_WIDTH);
        let left_sidebar_max_width = logical_px(app_layout::LEFT_SIDEBAR_MAX_WIDTH);
        let center_canvas_width = logical_px(app_layout::DEFAULT_CENTER_CANVAS_WIDTH);
        let min_center_canvas_width = default_window_width.saturating_sub(left_sidebar_max_width);

        Self {
            default_window_width_logical_px: default_window_width,
            left_sidebar_width_logical_px: left_sidebar_width,
            left_sidebar_max_width_logical_px: left_sidebar_max_width,
            center_canvas_width_logical_px: center_canvas_width,
            min_center_canvas_width_logical_px: min_center_canvas_width,
            center_canvas_width_percent: percent(center_canvas_width, default_window_width),
            min_center_canvas_width_percent: percent(
                app_layout::MIN_CENTER_CANVAS_WIDTH_FRACTION,
                1.0,
            ),
        }
    }

    pub fn center_canvas_satisfies_minimum(&self) -> bool {
        percent(
            self.min_center_canvas_width_logical_px,
            self.default_window_width_logical_px,
        ) >= self.min_center_canvas_width_percent
    }
}

fn logical_px(value: f32) -> u32 {
    value.round().max(0.0) as u32
}

fn percent(part: impl Into<f64>, whole: impl Into<f64>) -> u32 {
    let whole = whole.into();
    if whole <= 0.0 {
        0
    } else {
        ((part.into() / whole) * 100.0).round().max(0.0) as u32
    }
}
