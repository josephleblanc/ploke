//! Shared 3-column metric board: fixed label | count | bar track.
//!
//! All label|count|bar operator rows must use this module so egui `Grid` column
//! count stays consistent (bar padding + track live in **one** grid cell).

use eframe::egui;

use crate::ui::bar_profiles::{self, metric_fill_width, METRIC_TRACK_X_PADDING};

use super::run_dashboard::{
    RunDashboardValueTone, dashboard_bar_fill_color, dashboard_value_color,
};

pub(crate) const LABEL_COL_WIDTH: f32 = 140.0;
pub(crate) const VALUE_COL_WIDTH: f32 = 40.0;
pub(crate) const BAR_COLUMN_LEFT_PADDING: f32 = 10.0;
/// Right inset before inspector pane / resize handle (mirrors timeline `LANE_X_PADDING` scale).
pub(crate) const BAR_TRACK_RIGHT_INSET: f32 = METRIC_TRACK_X_PADDING;
pub(crate) const LABEL_FONT_SIZE: f32 = 13.0;
pub(crate) const COUNT_FONT_SIZE: f32 = 14.0;
pub(crate) const BAR_ROW_HEIGHT: f32 = 18.0;
pub(crate) const ROW_V_SPACING: f32 = 7.0;
pub(crate) const MIN_BAR_TRACK_WIDTH: f32 = 48.0;

/// One row in a metric board (value drives bar scale and count text).
pub(crate) struct MetricBoardRow<'a> {
    pub label: &'a str,
    pub value: f32,
    pub tone: RunDashboardValueTone,
}

/// Per-section layout knobs (defaults match Analyst Snapshot).
#[derive(Debug, Clone, Copy)]
pub(crate) struct MetricRowBoardStyle {
    pub label_col_width: f32,
    pub row_v_spacing: f32,
    pub label_font_size: f32,
}

impl Default for MetricRowBoardStyle {
    fn default() -> Self {
        Self {
            label_col_width: LABEL_COL_WIDTH,
            row_v_spacing: ROW_V_SPACING,
            label_font_size: LABEL_FONT_SIZE,
        }
    }
}

/// Minimum width for a full metric row (label + count + pad + min bar track + grid gaps).
pub(crate) fn row_board_min_width(label_col_width: f32, item_spacing_x: f32) -> f32 {
    label_col_width
        + VALUE_COL_WIDTH
        + BAR_COLUMN_LEFT_PADDING
        + BAR_TRACK_RIGHT_INSET
        + item_spacing_x * 2.0
        + MIN_BAR_TRACK_WIDTH
}

/// Bar-track width for column 3 when the section spans `section_width`.
pub(crate) fn metric_bar_track_width(
    section_width: f32,
    item_spacing_x: f32,
    label_col_width: f32,
) -> f32 {
    (section_width
        - label_col_width
        - VALUE_COL_WIDTH
        - BAR_COLUMN_LEFT_PADDING
        - BAR_TRACK_RIGHT_INSET
        - item_spacing_x * 2.0)
        .max(MIN_BAR_TRACK_WIDTH)
}

/// Render a 3-column grid of metric rows; `section_width` is captured at section start
/// (per-column `ui.available_width()` in two-column layouts, not the parent pane width).
pub(crate) fn render_metric_row_board(
    ui: &mut egui::Ui,
    grid_id: (&str, &str),
    rows: &[MetricBoardRow<'_>],
    section_width: f32,
    style: MetricRowBoardStyle,
) {
    if rows.is_empty() {
        return;
    }

    let section_max = rows
        .iter()
        .map(|row| row.value)
        .fold(0.0_f32, f32::max)
        .max(1.0);

    let item_spacing_x = ui.spacing().item_spacing.x;
    let section_track_width =
        metric_bar_track_width(section_width, item_spacing_x, style.label_col_width);
    let grid_spacing = egui::vec2(item_spacing_x, style.row_v_spacing);

    egui::Grid::new(grid_id)
        .num_columns(3)
        .spacing(grid_spacing)
        .min_row_height(BAR_ROW_HEIGHT)
        .show(ui, |ui| {
            for row in rows {
                render_metric_row_board_row(
                    ui,
                    row,
                    section_max,
                    style,
                    section_track_width,
                );
            }
        });
}

/// Truncated single-line layout for metric grid cells (`Grid` forces `Label` left align).
fn truncated_haligned_layout_job(
    text: &str,
    col_width: f32,
    font_id: egui::FontId,
    color: egui::Color32,
    halign: egui::Align,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font_id, color, col_width);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.halign = halign;
    job
}

fn paint_truncated_haligned_cell(
    ui: &mut egui::Ui,
    text: &str,
    cell_size: egui::Vec2,
    font_id: egui::FontId,
    color: egui::Color32,
    halign: egui::Align,
) -> egui::Response {
    let job = truncated_haligned_layout_job(text, cell_size.x, font_id, color, halign);
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let anchor = match halign {
        egui::Align::LEFT => egui::Align2::LEFT_CENTER,
        egui::Align::Center => egui::Align2::CENTER_CENTER,
        egui::Align::RIGHT => egui::Align2::RIGHT_CENTER,
    };
    let (rect, mut response) = ui.allocate_exact_size(cell_size, egui::Sense::hover());
    if ui.is_rect_visible(rect) && !galley.is_empty() {
        let galley_rect = anchor.align_size_within_rect(galley.size(), rect);
        let pos = match halign {
            egui::Align::LEFT => galley_rect.left_top(),
            egui::Align::Center => galley_rect.center_top(),
            egui::Align::RIGHT => galley_rect.right_top(),
        };
        ui.painter().galley(pos, galley.clone(), color);
    }
    if galley.elided {
        response = response.on_hover_text(text);
    }
    response
}

fn render_metric_row_board_row(
    ui: &mut egui::Ui,
    row: &MetricBoardRow<'_>,
    section_max: f32,
    style: MetricRowBoardStyle,
    section_track_width: f32,
) {
    let label_size = egui::vec2(style.label_col_width, BAR_ROW_HEIGHT);
    paint_truncated_haligned_cell(
        ui,
        row.label,
        label_size,
        egui::FontId::proportional(style.label_font_size),
        ui.visuals().weak_text_color(),
        egui::Align::RIGHT,
    );

    let count_text = format_metric_board_value(row.value);
    let count_color = metric_board_count_color(ui, row.tone, row.value);
    paint_truncated_haligned_cell(
        ui,
        &count_text,
        egui::vec2(VALUE_COL_WIDTH, BAR_ROW_HEIGHT),
        egui::FontId::monospace(COUNT_FONT_SIZE),
        count_color,
        egui::Align::RIGHT,
    );

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(BAR_COLUMN_LEFT_PADDING);
        let (track_rect, _) = ui.allocate_exact_size(
            egui::vec2(section_track_width, BAR_ROW_HEIGHT),
            egui::Sense::hover(),
        );
        let fill_width = metric_fill_width(row.value, section_max, track_rect.width());
        bar_profiles::paint_metric_fill_bar(
            ui,
            track_rect,
            fill_width,
            dashboard_bar_fill_color(ui, row.tone),
        );
    });

    ui.end_row();
}

fn metric_board_count_color(
    ui: &egui::Ui,
    tone: RunDashboardValueTone,
    value: f32,
) -> egui::Color32 {
    if value <= 0.0 {
        ui.visuals().weak_text_color()
    } else {
        dashboard_value_color(ui, tone)
    }
}

pub(crate) fn format_metric_board_value(value: f32) -> String {
    if value.fract().abs() < f32::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::bar_profiles::metric_fill_width;

    #[test]
    fn row_board_min_width_sums_fixed_columns_and_min_track() {
        let spacing = 8.0;
        assert_eq!(
            row_board_min_width(LABEL_COL_WIDTH, spacing),
            LABEL_COL_WIDTH
                + VALUE_COL_WIDTH
                + BAR_COLUMN_LEFT_PADDING
                + BAR_TRACK_RIGHT_INSET
                + spacing * 2.0
                + MIN_BAR_TRACK_WIDTH
        );
    }

    #[test]
    fn bar_track_right_inset_matches_bar_profiles_metric_padding() {
        assert_eq!(BAR_TRACK_RIGHT_INSET, METRIC_TRACK_X_PADDING);
        assert_eq!(BAR_TRACK_RIGHT_INSET, 8.0);
    }

    #[test]
    fn metric_bar_track_width_reserves_fixed_columns() {
        let track = metric_bar_track_width(400.0, 8.0, LABEL_COL_WIDTH);
        assert_eq!(
            track,
            400.0
                - LABEL_COL_WIDTH
                - VALUE_COL_WIDTH
                - BAR_COLUMN_LEFT_PADDING
                - BAR_TRACK_RIGHT_INSET
                - 16.0
        );
        assert_eq!(track, 186.0);
    }

    #[test]
    fn metric_bar_track_width_uses_compact_label_column() {
        let compact = 120.0;
        let track = metric_bar_track_width(280.0, 8.0, compact);
        assert_eq!(track, 86.0);
    }

    #[test]
    fn metric_bar_track_width_enforces_minimum_track() {
        assert_eq!(
            metric_bar_track_width(100.0, 8.0, LABEL_COL_WIDTH),
            MIN_BAR_TRACK_WIDTH
        );
    }

    #[test]
    fn metric_fill_width_never_exceeds_track_when_value_within_section_max() {
        let track = 80.0;
        let section_max = 26.0;
        for value in [0.0, 1.0, 13.0, 26.0] {
            let fill = metric_fill_width(value, section_max, track);
            assert!(
                fill <= track + f32::EPSILON,
                "fill {fill} > track {track} for value {value}"
            );
        }
    }

    #[test]
    fn format_metric_board_value_shows_zero() {
        assert_eq!(format_metric_board_value(0.0), "0");
    }

    #[test]
    fn row_label_layout_job_is_right_aligned_and_truncated() {
        let job = truncated_haligned_layout_job(
            "focused progress",
            LABEL_COL_WIDTH,
            egui::FontId::proportional(LABEL_FONT_SIZE),
            egui::Color32::WHITE,
            egui::Align::RIGHT,
        );
        assert_eq!(job.halign, egui::Align::RIGHT);
        assert_eq!(job.wrap.max_rows, 1);
        assert_eq!(job.wrap.max_width, LABEL_COL_WIDTH);
        assert!(job.wrap.break_anywhere);
    }
}
