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
#[cfg(feature = "dev")]
pub(crate) const DEFAULT_CENTER_CANVAS_HEIGHT: f32 =
    DEFAULT_WINDOW_HEIGHT - TOP_STRIP_HEIGHT - BOTTOM_TIMELINE_HEIGHT;
pub(crate) const MIN_CENTER_CANVAS_WIDTH_FRACTION: f32 = 0.50;

pub(crate) const INSPECTOR_MARGIN_INNER: egui::Margin = egui::Margin {
    left: 8,
    right: 8,
    top: 8,
    bottom: 8,
};

/// Left run-navigation panel: keep padding on the outer edges only so the seam
/// against [`central_dashboard_panel_frame`] does not leave an unpainted gutter.
pub(crate) fn run_navigation_panel_frame(style: &egui::Style) -> egui::Frame {
    egui::Frame::side_top_panel(style).inner_margin(egui::Margin {
        left: 8,
        right: 0,
        top: 2,
        bottom: 2,
    })
}

/// Central tile tree panel: zero left inner margin where it meets the left sidebar.
pub(crate) fn central_dashboard_panel_frame(style: &egui::Style) -> egui::Frame {
    egui::Frame::central_panel(style).inner_margin(egui::Margin {
        left: 0,
        right: 8,
        top: 8,
        bottom: 8,
    })
}
