//! Default native layout measurements for the operator graph UI.

pub(crate) const DEFAULT_WINDOW_WIDTH: f32 = 1280.0;
pub(crate) const DEFAULT_WINDOW_HEIGHT: f32 = 720.0;
pub(crate) const TOP_STRIP_HEIGHT: f32 = 32.0;
pub(crate) const LEFT_SIDEBAR_WIDTH: f32 = 200.0;
pub(crate) const LEFT_SIDEBAR_MAX_WIDTH: f32 = 240.0;
pub(crate) const RIGHT_INSPECTOR_WIDTH: f32 = 300.0;
pub(crate) const RIGHT_INSPECTOR_MAX_WIDTH: f32 = 360.0;
pub(crate) const BOTTOM_TIMELINE_HEIGHT: f32 = 120.0;
pub(crate) const DEFAULT_CENTER_CANVAS_WIDTH: f32 =
    DEFAULT_WINDOW_WIDTH - LEFT_SIDEBAR_WIDTH - RIGHT_INSPECTOR_WIDTH;
pub(crate) const DEFAULT_CENTER_CANVAS_HEIGHT: f32 =
    DEFAULT_WINDOW_HEIGHT - TOP_STRIP_HEIGHT - BOTTOM_TIMELINE_HEIGHT;
pub(crate) const MIN_CENTER_CANVAS_WIDTH_FRACTION: f32 = 0.50;
