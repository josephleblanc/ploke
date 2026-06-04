//! Structured Protocol artifacts summary (Eval & Protocol center pane).

use eframe::egui;
use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::ui::eval_protocol::{EvalProtocolDashboard, ProtocolAggregateCounts};

use super::InspectorRenderCache;
use super::metric_row_board::{self, MetricBoardRow, MetricRowBoardStyle};
use super::run_dashboard::{
    RunDashboardValueTone, analyst_label_for_overall_verdict_key, effective_tone_for_count,
    render_dashboard_metric_card, render_dashboard_section, tone_for_protocol_issue_count,
    tone_for_protocol_overall_verdict_key, tone_for_protocol_recoverability_verdict_key,
    tone_for_protocol_redundancy_verdict_key,
};

const PROTOCOL_VERDICT_ROW_V_SPACING: f32 = 2.0;
const SECTION_GAP: f32 = 6.0;

const OVERALL_VERDICT_ORDER: &[&str] = &[
    "focused_progress",
    "useful_exploration",
    "recoverable_detour",
    "redundant_thrash",
    "mixed",
    "unclear",
];

const REDUNDANCY_VERDICT_ORDER: &[&str] = &[
    "distinct",
    "overlapping",
    "redundant_repeat",
    "search_thrash",
    "unclear",
];

const RECOVERABILITY_VERDICT_ORDER: &[&str] = &[
    "no_recovery_needed",
    "clear_next_step",
    "partial_next_step",
    "no_clear_recovery",
    "unclear",
];

pub(crate) fn render_protocol_artifacts_summary(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dashboard: &EvalProtocolDashboard<'_>,
) {
    let stats = dashboard.protocol_aggregate_counts();
    let review_stats = dashboard.protocol_review_stats();

    render_inventory_section(ui, render_cache, dashboard);
    render_review_volume_section(ui, render_cache, dashboard);
    render_issues_section(ui, render_cache, &stats);

    if !review_stats.overall.is_empty() {
        ui.add_space(SECTION_GAP);
        render_verdict_distribution_section(
            ui,
            "Overall outcome mix",
            &review_stats.overall,
            OVERALL_VERDICT_ORDER,
            ProtocolVerdictFamily::Overall,
        );
    }
    if !review_stats.redundancy.is_empty() {
        ui.add_space(SECTION_GAP);
        render_verdict_distribution_section(
            ui,
            "Redundancy",
            &review_stats.redundancy,
            REDUNDANCY_VERDICT_ORDER,
            ProtocolVerdictFamily::Redundancy,
        );
    }
    if !review_stats.recoverability.is_empty() {
        ui.add_space(SECTION_GAP);
        render_verdict_distribution_section(
            ui,
            "Recoverability",
            &review_stats.recoverability,
            RECOVERABILITY_VERDICT_ORDER,
            ProtocolVerdictFamily::Recoverability,
        );
    }
}

fn render_inventory_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dashboard: &EvalProtocolDashboard<'_>,
) {
    render_dashboard_section(ui, render_cache, "Inventory", |ui, cache| {
        render_optional_count_card(
            ui,
            cache,
            "artifact files",
            dashboard.protocol_artifacts_file_count(),
        );
        render_optional_count_card(
            ui,
            cache,
            "artifacts",
            dashboard.protocol_artifacts_parsed_count(),
        );
        render_optional_count_card(
            ui,
            cache,
            "intent segments",
            dashboard.protocol_artifacts_intent_segmentation_count(),
        );
    });
}

fn render_review_volume_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dashboard: &EvalProtocolDashboard<'_>,
) {
    render_dashboard_section(ui, render_cache, "Review volume", |ui, cache| {
        render_optional_count_card(
            ui,
            cache,
            "call reviews",
            dashboard.protocol_artifacts_review_count(),
        );
        render_optional_count_card(
            ui,
            cache,
            "segment reviews",
            dashboard.protocol_artifacts_segment_review_count(),
        );
    });
}

fn render_issues_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    stats: &ProtocolAggregateCounts,
) {
    render_dashboard_section(ui, render_cache, "Issues & interventions", |ui, cache| {
        let mut detections = itoa::Buffer::new();
        render_dashboard_metric_card(
            ui,
            cache,
            "issue detections",
            detections.format(stats.issue_detection_count),
            tone_for_protocol_issue_count(stats.issue_detection_count),
        );
        let mut cases = itoa::Buffer::new();
        render_dashboard_metric_card(
            ui,
            cache,
            "issue cases",
            cases.format(stats.issue_detection_case_count),
            tone_for_protocol_issue_count(stats.issue_detection_case_count),
        );
        let mut interventions = itoa::Buffer::new();
        render_dashboard_metric_card(
            ui,
            cache,
            "interventions",
            interventions.format(stats.intervention_candidate_count),
            tone_for_protocol_issue_count(stats.intervention_candidate_count),
        );
        let mut applies = itoa::Buffer::new();
        render_dashboard_metric_card(
            ui,
            cache,
            "intervention applies",
            applies.format(stats.intervention_apply_count),
            effective_tone_for_count(
                RunDashboardValueTone::Positive,
                stats.intervention_apply_count,
            ),
        );
    });
}

fn render_optional_count_card(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    label: &str,
    value: Option<usize>,
) {
    let mut buf = itoa::Buffer::new();
    let (display, tone) = match value {
        Some(count) => (buf.format(count), RunDashboardValueTone::Neutral),
        None => ("—", RunDashboardValueTone::Neutral),
    };
    render_dashboard_metric_card(ui, render_cache, label, display, tone);
}

fn render_verdict_distribution_section(
    ui: &mut egui::Ui,
    title: &str,
    counts: &BTreeMap<String, usize>,
    preferred_order: &[&str],
    family: ProtocolVerdictFamily,
) {
    let rows = ordered_protocol_verdict_rows(counts, preferred_order);
    if rows.is_empty() {
        return;
    }

    ui.label(egui::RichText::new(title).strong().size(12.0));
    ui.add_space(2.0);

    let section_width = ui.available_width();
    let labels: Vec<String> = rows
        .iter()
        .map(|(key, _)| protocol_verdict_display_label(family, key).into_owned())
        .collect();
    let board_rows: Vec<MetricBoardRow<'_>> = rows
        .iter()
        .zip(labels.iter())
        .map(|((key, count), label)| MetricBoardRow {
            label: label.as_str(),
            value: *count as f32,
            tone: effective_tone_for_count(family.tone_for_key(key), *count),
        })
        .collect();

    metric_row_board::render_metric_row_board(
        ui,
        ("protocol-artifacts-verdict", title),
        &board_rows,
        section_width,
        MetricRowBoardStyle {
            row_v_spacing: PROTOCOL_VERDICT_ROW_V_SPACING,
            ..MetricRowBoardStyle::default()
        },
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtocolVerdictFamily {
    Overall,
    Redundancy,
    Recoverability,
}

impl ProtocolVerdictFamily {
    fn tone_for_key(self, key: &str) -> RunDashboardValueTone {
        match self {
            Self::Overall => tone_for_protocol_overall_verdict_key(key),
            Self::Redundancy => tone_for_protocol_redundancy_verdict_key(key),
            Self::Recoverability => tone_for_protocol_recoverability_verdict_key(key),
        }
    }
}

fn protocol_verdict_display_label(family: ProtocolVerdictFamily, key: &str) -> Cow<'_, str> {
    match family {
        ProtocolVerdictFamily::Overall => Cow::Borrowed(analyst_label_for_overall_verdict_key(key)),
        ProtocolVerdictFamily::Redundancy | ProtocolVerdictFamily::Recoverability => {
            if key.contains('_') {
                Cow::Owned(humanize_snake_case_key(key))
            } else {
                Cow::Borrowed(key)
            }
        }
    }
}

/// Verdict histogram rows: every key in `preferred_order`, count `0` when absent.
pub(crate) fn ordered_protocol_verdict_rows<'a>(
    counts: &'a BTreeMap<String, usize>,
    preferred_order: &[&'a str],
) -> Vec<(&'a str, usize)> {
    let mut rows: Vec<(&'a str, usize)> = preferred_order
        .iter()
        .map(|&key| (key, counts.get(key).copied().unwrap_or(0)))
        .collect();
    for (key, &count) in counts {
        if preferred_order.contains(&key.as_str()) {
            continue;
        }
        rows.push((key.as_str(), count));
    }
    rows
}

pub(crate) fn ordered_protocol_counts<'a>(
    counts: &'a BTreeMap<String, usize>,
    preferred_order: &[&'a str],
) -> Vec<(&'a str, usize)> {
    let mut rows = Vec::new();
    for &key in preferred_order {
        if let Some(&count) = counts.get(key) {
            rows.push((key, count));
        }
    }
    for (key, &count) in counts {
        if preferred_order.contains(&key.as_str()) {
            continue;
        }
        rows.push((key.as_str(), count));
    }
    rows
}

pub(crate) fn humanize_snake_case_key(key: &str) -> String {
    key.split('_')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn ordered_protocol_verdict_rows_include_zero_counts() {
        let mut counts = BTreeMap::new();
        counts.insert("focused_progress".to_string(), 3);
        let rows = ordered_protocol_verdict_rows(&counts, OVERALL_VERDICT_ORDER);
        assert_eq!(rows.len(), OVERALL_VERDICT_ORDER.len());
        assert_eq!(rows[0], ("focused_progress", 3));
        assert_eq!(rows[1], ("useful_exploration", 0));
    }

    #[test]
    fn ordered_protocol_counts_respects_preferred_order() {
        let mut counts = BTreeMap::new();
        counts.insert("mixed".to_string(), 2);
        counts.insert("focused_progress".to_string(), 26);
        counts.insert("unclear".to_string(), 1);
        let rows = ordered_protocol_counts(&counts, OVERALL_VERDICT_ORDER);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].0, "focused_progress");
        assert_eq!(rows[0].1, 26);
        assert_eq!(rows[1].0, "mixed");
        assert_eq!(rows[2].0, "unclear");
    }

    #[test]
    fn ordered_protocol_counts_appends_unknown_keys_last() {
        let mut counts = BTreeMap::new();
        counts.insert("custom_verdict".to_string(), 4);
        counts.insert("focused_progress".to_string(), 1);
        let rows = ordered_protocol_counts(&counts, OVERALL_VERDICT_ORDER);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "focused_progress");
        assert_eq!(rows[1].0, "custom_verdict");
    }

    #[test]
    fn humanize_snake_case_key_splits_on_underscores() {
        assert_eq!(
            humanize_snake_case_key("no_clear_recovery"),
            "no clear recovery"
        );
        assert_eq!(
            humanize_snake_case_key("redundant_repeat"),
            "redundant repeat"
        );
    }

    #[test]
    fn overall_display_label_matches_analyst_snapshot() {
        let label =
            protocol_verdict_display_label(ProtocolVerdictFamily::Overall, "focused_progress");
        assert_eq!(label.as_ref(), "focused progress");
    }

    #[test]
    fn protocol_verdict_rows_use_shared_metric_row_board() {
        use super::super::metric_row_board::{BAR_ROW_HEIGHT, LABEL_COL_WIDTH};
        assert_eq!(LABEL_COL_WIDTH, 140.0);
        assert_eq!(BAR_ROW_HEIGHT, 18.0);
    }
}
