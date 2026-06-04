//! Compact Analyst Snapshot metric board (Eval & Protocol inspector).
//!
//! **Resize checklist (manual):** narrow → single column, labels/bars aligned; widen past
//! ~560px → 2×2 sections without label/bar overlap; shrink below ~500px → back to single column
//! without flicker; intermediate ~500–559px stays single column.

use eframe::egui;

use crate::ui::eval_protocol::EvalProtocolVisualSummary;

use super::cache::effective_eval_pane_content_width;
use super::metric_row_board::{
    self, LABEL_COL_WIDTH, MetricBoardRow, MetricRowBoardStyle, row_board_min_width,
};
use super::run_dashboard::effective_tone_for_analyst_snapshot_metric;

const SECTION_TITLE_FONT_SIZE: f32 = 14.0;
const SECTION_TITLE_SPACING: f32 = 4.0;
const SECTION_BLOCK_SPACING: f32 = 10.0;
/// Panel width to enter 2×2 section layout (hysteresis high edge).
const TWO_COLUMN_ENTER_WIDTH: f32 = 560.0;
/// Panel width to leave 2×2 layout (hysteresis low edge).
const TWO_COLUMN_EXIT_WIDTH: f32 = 500.0;
/// Narrower label column in 2-col so label+count+bar min fits ~half panel width.
const TWO_COLUMN_LABEL_COL_WIDTH: f32 = 120.0;
const TWO_COLUMN_INNER_MARGIN: i8 = 8;
const TWO_COLUMN_LAYOUT_ID: &str = "analyst_snapshot_two_col";

fn two_column_min_panel_width(item_spacing_x: f32) -> f32 {
    let per_column = row_board_min_width(TWO_COLUMN_LABEL_COL_WIDTH, item_spacing_x);
    2.0 * per_column + (TWO_COLUMN_INNER_MARGIN as f32) * 2.0 + item_spacing_x
}

fn analyst_snapshot_two_column_layout_resolved(
    available_width: f32,
    section_count: usize,
    was_two_column: bool,
    item_spacing_x: f32,
) -> bool {
    if section_count <= 1 {
        return false;
    }
    let min_panel = two_column_min_panel_width(item_spacing_x);
    if was_two_column {
        available_width >= TWO_COLUMN_EXIT_WIDTH && available_width >= min_panel
    } else {
        available_width >= TWO_COLUMN_ENTER_WIDTH.max(min_panel)
    }
}

fn analyst_snapshot_two_column_layout(
    ui: &mut egui::Ui,
    available_width: f32,
    section_count: usize,
) -> bool {
    let id = ui.id().with(TWO_COLUMN_LAYOUT_ID);
    let was_two_column = ui.data(|data| data.get_temp::<bool>(id)).unwrap_or(false);
    let item_spacing_x = ui.spacing().item_spacing.x;
    let two_column = analyst_snapshot_two_column_layout_resolved(
        available_width,
        section_count,
        was_two_column,
        item_spacing_x,
    );
    ui.data_mut(|data| data.insert_temp(id, two_column));
    two_column
}

pub(crate) fn render_analyst_snapshot_panel(ui: &mut egui::Ui, summary: EvalProtocolVisualSummary) {
    let pane_width = effective_eval_pane_content_width(ui);
    ui.set_max_width(pane_width);

    let run_effort = [
        ("records", summary.run_records as f32),
        ("turns", summary.turns as f32),
        ("tool calls", summary.tool_calls as f32),
        ("failed tools", summary.failed_tool_calls as f32),
    ];
    let protocol_coverage = [
        ("reviewed calls", summary.reviewed_calls as f32),
        (
            "missing call reviews",
            summary.missing_call_reviews() as f32,
        ),
        ("usable segments", summary.usable_segments as f32),
        ("mismatched segments", summary.mismatched_segments as f32),
        ("missing segments", summary.missing_segments as f32),
    ];

    let outcomes = summary
        .call_review_outcomes
        .combined(summary.segment_review_outcomes);
    let outcome_rows = [
        ("focused progress", outcomes.focused_progress as f32),
        ("useful exploration", outcomes.useful_exploration as f32),
        ("recoverable detour", outcomes.recoverable_detour as f32),
        ("redundant thrash", outcomes.redundant_thrash as f32),
        ("mixed", outcomes.mixed as f32),
        ("unclear", outcomes.unclear as f32),
    ];
    let show_outcomes = outcomes.total() > 0;

    let patch = summary.patch;
    let patch_rows = [
        ("edit proposals", patch.edit_proposal_count as f32),
        ("create proposals", patch.create_proposal_count as f32),
        (
            "expected file changes",
            patch.expected_file_change_count as f32,
        ),
        (
            "applied artifacts",
            patch.applied_patch_artifact_count as f32,
        ),
    ];
    let show_patch = patch.edit_proposal_count > 0
        || patch.create_proposal_count > 0
        || patch.expected_file_change_count > 0
        || patch.applied_patch_artifact_count > 0;

    let section_count = 2 + usize::from(show_outcomes) + usize::from(show_patch);
    let two_column = analyst_snapshot_two_column_layout(ui, pane_width, section_count);
    let board_style = MetricRowBoardStyle {
        label_col_width: if two_column {
            TWO_COLUMN_LABEL_COL_WIDTH
        } else {
            LABEL_COL_WIDTH
        },
        ..MetricRowBoardStyle::default()
    };

    if two_column {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(TWO_COLUMN_INNER_MARGIN, 0))
            .show(ui, |ui| {
                ui.columns(2, |columns| {
                    render_snapshot_section(
                        &mut columns[0],
                        "Run Effort",
                        &run_effort,
                        true,
                        board_style,
                    );
                    render_snapshot_section(
                        &mut columns[0],
                        "Protocol Coverage",
                        &protocol_coverage,
                        false,
                        board_style,
                    );
                    if show_outcomes {
                        render_snapshot_section(
                            &mut columns[1],
                            "Review Outcome Mix",
                            &outcome_rows,
                            true,
                            board_style,
                        );
                    }
                    if show_patch {
                        render_snapshot_section(
                            &mut columns[1],
                            "Patch Production",
                            &patch_rows,
                            !show_outcomes,
                            board_style,
                        );
                    }
                });
            });
    } else {
        render_snapshot_section(ui, "Run Effort", &run_effort, true, board_style);
        render_snapshot_section(
            ui,
            "Protocol Coverage",
            &protocol_coverage,
            false,
            board_style,
        );
        if show_outcomes {
            render_snapshot_section(ui, "Review Outcome Mix", &outcome_rows, false, board_style);
        }
        if show_patch {
            render_snapshot_section(ui, "Patch Production", &patch_rows, false, board_style);
        }
    }
}

fn render_snapshot_section(
    ui: &mut egui::Ui,
    title: &str,
    rows: &[(&str, f32)],
    first_in_column: bool,
    style: MetricRowBoardStyle,
) {
    if !first_in_column {
        ui.add_space(SECTION_BLOCK_SPACING);
    }
    ui.label(
        egui::RichText::new(title)
            .strong()
            .size(SECTION_TITLE_FONT_SIZE),
    );
    ui.add_space(SECTION_TITLE_SPACING);

    let section_width = effective_eval_pane_content_width(ui);
    let board_rows: Vec<MetricBoardRow<'_>> = rows
        .iter()
        .map(|(label, value)| MetricBoardRow {
            label,
            value: *value,
            tone: effective_tone_for_analyst_snapshot_metric(label, *value),
        })
        .collect();
    metric_row_board::render_metric_row_board(
        ui,
        ("analyst-snapshot-metrics", title),
        &board_rows,
        section_width,
        style,
    );
}

#[cfg(test)]
mod tests {
    use super::super::metric_row_board::{LABEL_COL_WIDTH, metric_bar_track_width};
    use super::*;

    #[test]
    fn two_column_layout_hysteresis_and_minimum_width() {
        let spacing = 8.0;
        assert!(!analyst_snapshot_two_column_layout_resolved(
            559.0, 4, false, spacing
        ));
        assert!(analyst_snapshot_two_column_layout_resolved(
            560.0, 4, false, spacing
        ));
        assert!(analyst_snapshot_two_column_layout_resolved(
            520.0, 4, true, spacing
        ));
        assert!(!analyst_snapshot_two_column_layout_resolved(
            499.0, 4, true, spacing
        ));
        assert!(!analyst_snapshot_two_column_layout_resolved(
            800.0, 1, false, spacing
        ));
    }

    #[test]
    fn two_column_min_panel_width_uses_compact_labels() {
        let spacing = 8.0;
        let min_panel = two_column_min_panel_width(spacing);
        assert_eq!(
            min_panel,
            2.0 * row_board_min_width(TWO_COLUMN_LABEL_COL_WIDTH, spacing)
                + (TWO_COLUMN_INNER_MARGIN as f32) * 2.0
                + spacing
        );
        assert!(TWO_COLUMN_ENTER_WIDTH >= min_panel);
    }

    #[test]
    fn analyst_snapshot_uses_shared_row_board_constants() {
        assert_eq!(LABEL_COL_WIDTH, 140.0);
        let track = metric_bar_track_width(400.0, 8.0, LABEL_COL_WIDTH);
        assert_eq!(track, 186.0);
    }
}
