//! Compact Run dashboard status board for Eval & Protocol.

use eframe::egui;

use crate::ui::eval_protocol::{EvalProtocolDashboard, EvalProtocolVisualSummary, EvidenceState};
use crate::ui::theme::tokens_from_ui;

use super::InspectorRenderCache;
use super::fields::cached_label;

const STATUS_CARD_MIN_WIDTH: f32 = 100.0;
const STATUS_CARD_MAX_WIDTH: f32 = 132.0;

/// Semantic tone for a dashboard card value (maps to theme tokens / scan emphasis).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunDashboardValueTone {
    Neutral,
    Positive,
    Warn,
    Error,
}

pub(crate) fn tone_for_evidence_state(state: EvidenceState) -> RunDashboardValueTone {
    match state {
        EvidenceState::Available => RunDashboardValueTone::Positive,
        EvidenceState::Missing => RunDashboardValueTone::Error,
        EvidenceState::NotApplicable => RunDashboardValueTone::Neutral,
    }
}

pub(crate) fn tone_for_status_label(label: &str) -> RunDashboardValueTone {
    let normalized = label.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "available" | "complete" | "completed" | "success" | "passed" | "ok"
    ) {
        return RunDashboardValueTone::Positive;
    }
    if matches!(
        normalized.as_str(),
        "partial" | "in_progress" | "in progress" | "pending" | "running" | "warn" | "warning"
    ) {
        return RunDashboardValueTone::Warn;
    }
    if matches!(
        normalized.as_str(),
        "failed" | "error" | "missing" | "incomplete" | "not_available" | "not_applicable"
    ) {
        return RunDashboardValueTone::Error;
    }
    RunDashboardValueTone::Neutral
}

pub(crate) fn tone_for_failed_tool_calls(count: usize) -> RunDashboardValueTone {
    if count > 0 {
        RunDashboardValueTone::Error
    } else {
        RunDashboardValueTone::Neutral
    }
}

pub(crate) fn tone_for_missing_call_reviews(count: usize) -> RunDashboardValueTone {
    if count > 0 {
        RunDashboardValueTone::Warn
    } else {
        RunDashboardValueTone::Neutral
    }
}

pub(crate) fn dashboard_value_color(ui: &egui::Ui, tone: RunDashboardValueTone) -> egui::Color32 {
    let tokens = tokens_from_ui(ui);
    match tone {
        RunDashboardValueTone::Neutral => ui.visuals().weak_text_color(),
        RunDashboardValueTone::Positive => tokens.success,
        RunDashboardValueTone::Warn => tokens.warning,
        RunDashboardValueTone::Error => tokens.error,
    }
}

/// Bar fill for Analyst Snapshot / status-board metrics (stronger than value text when neutral).
pub(crate) fn dashboard_bar_fill_color(
    ui: &egui::Ui,
    tone: RunDashboardValueTone,
) -> egui::Color32 {
    let tokens = tokens_from_ui(ui);
    match tone {
        RunDashboardValueTone::Neutral => tokens.text_muted,
        RunDashboardValueTone::Positive => tokens.success,
        RunDashboardValueTone::Warn => tokens.warning,
        RunDashboardValueTone::Error => tokens.error,
    }
}

const TOOL_STEP_OUTCOME_BAR_HEIGHT: f32 = 10.0;
const TOOL_STEP_OUTCOME_BAR_MIN_TRACK: f32 = 120.0;

/// Succeeded tool steps derived from total and failed counts (failed is clamped to total).
pub(crate) fn tool_steps_succeeded_count(total: usize, failed: usize) -> usize {
    total.saturating_sub(failed.min(total))
}

/// Stacked horizontal bar for succeeded vs failed tool steps plus accessible numeric labels.
pub(crate) fn render_tool_step_outcome_histogram(ui: &mut egui::Ui, total: usize, failed: usize) {
    if total == 0 {
        return;
    }
    let failed = failed.min(total);
    let succeeded = tool_steps_succeeded_count(total, failed);
    let track_width = ui.available_width().max(TOOL_STEP_OUTCOME_BAR_MIN_TRACK);

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        let (rect, response) = ui.allocate_at_least(
            egui::vec2(track_width, TOOL_STEP_OUTCOME_BAR_HEIGHT),
            egui::Sense::hover(),
        );
        let mut hover = itoa::Buffer::new();
        let hover_total = hover.format(total);
        let mut hover_succeeded = itoa::Buffer::new();
        let mut hover_failed = itoa::Buffer::new();
        response.on_hover_text(format!(
            "{} tool steps: {} succeeded, {} failed",
            hover_total,
            hover_succeeded.format(succeeded),
            hover_failed.format(failed),
        ));

        crate::ui::bar_profiles::paint_stacked_outcome_bar(
            ui,
            rect,
            total,
            succeeded,
            failed,
            dashboard_bar_fill_color(ui, RunDashboardValueTone::Positive),
            dashboard_bar_fill_color(ui, RunDashboardValueTone::Error),
        );

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let mut total_buf = itoa::Buffer::new();
            ui.monospace(total_buf.format(total));
            ui.label(egui::RichText::new("·").weak());
            let mut succeeded_buf = itoa::Buffer::new();
            ui.label(
                egui::RichText::new(format!("{} succeeded", succeeded_buf.format(succeeded)))
                    .monospace()
                    .color(dashboard_value_color(ui, RunDashboardValueTone::Positive)),
            );
            if failed > 0 {
                ui.label(egui::RichText::new("·").weak());
                let mut failed_buf = itoa::Buffer::new();
                ui.label(
                    egui::RichText::new(format!("{} failed", failed_buf.format(failed)))
                        .monospace()
                        .color(dashboard_value_color(ui, RunDashboardValueTone::Error)),
                );
            }
        });
    });
}

/// Semantic tone for an Analyst Snapshot metric label (non-zero values only; see
/// [`effective_tone_for_analyst_snapshot_metric`]).
pub(crate) fn tone_for_analyst_snapshot_metric(metric: &str) -> RunDashboardValueTone {
    match metric {
        "failed tools" | "redundant thrash" | "mismatched segments" => RunDashboardValueTone::Error,
        "missing call reviews"
        | "missing segments"
        | "recoverable detour"
        | "mixed"
        | "unclear" => RunDashboardValueTone::Warn,
        "focused progress" | "useful exploration" | "reviewed calls" | "usable segments"
        | "applied artifacts" => RunDashboardValueTone::Positive,
        _ => RunDashboardValueTone::Neutral,
    }
}

pub(crate) fn effective_tone_for_analyst_snapshot_metric(
    metric: &str,
    value: f32,
) -> RunDashboardValueTone {
    if value <= 0.0 {
        RunDashboardValueTone::Neutral
    } else {
        tone_for_analyst_snapshot_metric(metric)
    }
}

pub(crate) fn effective_tone_for_count(
    tone_when_positive: RunDashboardValueTone,
    count: usize,
) -> RunDashboardValueTone {
    if count == 0 {
        RunDashboardValueTone::Neutral
    } else {
        tone_when_positive
    }
}

/// Analyst Snapshot label for a protocol `overall` verdict serde key (`snake_case`).
pub(crate) fn analyst_label_for_overall_verdict_key(key: &str) -> &str {
    match key {
        "focused_progress" => "focused progress",
        "useful_exploration" => "useful exploration",
        "recoverable_detour" => "recoverable detour",
        "redundant_thrash" => "redundant thrash",
        "mixed" => "mixed",
        "unclear" => "unclear",
        other => other,
    }
}

pub(crate) fn tone_for_protocol_overall_verdict_key(key: &str) -> RunDashboardValueTone {
    tone_for_analyst_snapshot_metric(analyst_label_for_overall_verdict_key(key))
}

pub(crate) fn tone_for_protocol_redundancy_verdict_key(key: &str) -> RunDashboardValueTone {
    match key {
        "distinct" | "overlapping" => RunDashboardValueTone::Positive,
        "redundant_repeat" | "search_thrash" => RunDashboardValueTone::Error,
        "unclear" => RunDashboardValueTone::Warn,
        _ => RunDashboardValueTone::Neutral,
    }
}

pub(crate) fn tone_for_protocol_recoverability_verdict_key(key: &str) -> RunDashboardValueTone {
    match key {
        "no_recovery_needed" | "clear_next_step" => RunDashboardValueTone::Positive,
        "partial_next_step" | "unclear" => RunDashboardValueTone::Warn,
        "no_clear_recovery" => RunDashboardValueTone::Error,
        _ => RunDashboardValueTone::Neutral,
    }
}

pub(crate) fn tone_for_protocol_issue_count(count: usize) -> RunDashboardValueTone {
    effective_tone_for_count(RunDashboardValueTone::Warn, count)
}

/// Semantic tone for a flattened call-review **Signals** metric (non-zero counts only).
pub(crate) fn tone_for_local_analysis_signal_metric(metric: &str) -> RunDashboardValueTone {
    match metric {
        "failed calls" => RunDashboardValueTone::Error,
        "repeated tools" | "similar searches" | "directory pivots" | "ambiguous segments"
        | "uncovered calls" => RunDashboardValueTone::Warn,
        _ => RunDashboardValueTone::Neutral,
    }
}

pub(crate) fn effective_tone_for_local_analysis_signal_metric(
    metric: &str,
    count: usize,
) -> RunDashboardValueTone {
    effective_tone_for_count(tone_for_local_analysis_signal_metric(metric), count)
}

/// Weak/muted presentation for optional signal fields absent from the export.
pub(crate) fn tone_for_local_analysis_signal_not_recorded() -> RunDashboardValueTone {
    RunDashboardValueTone::Neutral
}

pub(crate) fn tone_for_local_analysis_concern(
    concern: &ploke_protocol::Concern,
) -> RunDashboardValueTone {
    match concern {
        ploke_protocol::Concern::SearchThrashRisk => RunDashboardValueTone::Error,
        ploke_protocol::Concern::RecoveryOpportunity
        | ploke_protocol::Concern::RepeatedToolCluster
        | ploke_protocol::Concern::FilePivot
        | ploke_protocol::Concern::AmbiguousScope
        | ploke_protocol::Concern::ResidualCoverageGap => RunDashboardValueTone::Warn,
    }
}

pub(crate) fn evidence_state_label(state: EvidenceState) -> &'static str {
    match state {
        EvidenceState::Available => "available",
        EvidenceState::Missing => "missing",
        EvidenceState::NotApplicable => "not_applicable",
    }
}

pub(crate) fn render_run_dashboard_status_board(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dashboard: &EvalProtocolDashboard<'_>,
) {
    ui.label(egui::RichText::new("Run dashboard").strong());
    let availability = dashboard.availability();
    let summary = dashboard.visual_summary();

    render_dashboard_section(ui, render_cache, "Evidence", |ui, cache| {
        render_dashboard_metric_card(
            ui,
            cache,
            "closure",
            evidence_state_label(availability.closure),
            tone_for_evidence_state(availability.closure),
        );
        render_dashboard_metric_card(
            ui,
            cache,
            "run records",
            evidence_state_label(availability.run_records),
            tone_for_evidence_state(availability.run_records),
        );
        render_dashboard_metric_card(
            ui,
            cache,
            "protocol",
            evidence_state_label(availability.protocol_artifacts),
            tone_for_evidence_state(availability.protocol_artifacts),
        );
    });

    if let Some(closure) = dashboard.closure() {
        let registry = closure_status_label(&closure.state.registry.status);
        let eval = closure_status_label(&closure.state.eval.status);
        let protocol = closure_status_label(&closure.state.protocol.status);
        render_dashboard_section(ui, render_cache, "Lifecycle", |ui, cache| {
            render_dashboard_metric_card(
                ui,
                cache,
                "registry",
                registry.as_str(),
                tone_for_status_label(registry.as_str()),
            );
            render_dashboard_metric_card(
                ui,
                cache,
                "eval",
                eval.as_str(),
                tone_for_status_label(eval.as_str()),
            );
            render_dashboard_metric_card(
                ui,
                cache,
                "protocol",
                protocol.as_str(),
                tone_for_status_label(protocol.as_str()),
            );
            let mut instances = itoa::Buffer::new();
            render_dashboard_metric_card(
                ui,
                cache,
                "instances",
                instances.format(closure.state.instances.len()),
                RunDashboardValueTone::Neutral,
            );
        });
    }

    render_dashboard_coverage_section(ui, render_cache, summary);
    ui.separator();
}

fn render_dashboard_coverage_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    summary: EvalProtocolVisualSummary,
) {
    render_dashboard_section(ui, render_cache, "Coverage", |ui, cache| {
        let mut reviewed = itoa::Buffer::new();
        let mut total_calls = itoa::Buffer::new();
        let reviewed_ratio = format!(
            "{}/{}",
            reviewed.format(summary.reviewed_calls),
            total_calls.format(summary.total_calls)
        );
        let reviewed_tone = if summary.missing_call_reviews() > 0 {
            RunDashboardValueTone::Warn
        } else {
            RunDashboardValueTone::Neutral
        };
        render_dashboard_metric_card(
            ui,
            cache,
            "reviewed",
            reviewed_ratio.as_str(),
            reviewed_tone,
        );

        let mut missing = itoa::Buffer::new();
        render_dashboard_metric_card(
            ui,
            cache,
            "missing reviews",
            missing.format(summary.missing_call_reviews()),
            tone_for_missing_call_reviews(summary.missing_call_reviews()),
        );

        let mut failed = itoa::Buffer::new();
        render_dashboard_metric_card(
            ui,
            cache,
            "failed tools",
            failed.format(summary.failed_tool_calls),
            tone_for_failed_tool_calls(summary.failed_tool_calls),
        );

        let patch = summary.patch;
        let mut applied = itoa::Buffer::new();
        render_dashboard_metric_card(
            ui,
            cache,
            "applied patches",
            applied.format(patch.applied_patch_artifact_count),
            RunDashboardValueTone::Neutral,
        );
    });
}

pub(crate) fn render_dashboard_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    title: &str,
    body: impl FnOnce(&mut egui::Ui, &mut InspectorRenderCache),
) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(title)
            .small()
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(2.0);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.with_layout(
            egui::Layout::left_to_right(egui::Align::TOP).with_main_wrap(true),
            |ui| {
                body(ui, render_cache);
            },
        );
    });
}

pub(crate) fn render_dashboard_metric_card(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    label: &str,
    value: &str,
    tone: RunDashboardValueTone,
) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.set_min_width(STATUS_CARD_MIN_WIDTH);
            ui.set_max_width(STATUS_CARD_MAX_WIDTH);
            ui.vertical(|ui| {
                cached_label(ui, render_cache, label);
                render_dashboard_value(ui, value, tone);
            });
        });
}

fn render_dashboard_value(ui: &mut egui::Ui, value: &str, tone: RunDashboardValueTone) {
    ui.label(
        egui::RichText::new(value)
            .monospace()
            .color(dashboard_value_color(ui, tone)),
    );
}

fn closure_status_label<T>(value: &T) -> String
where
    T: serde::Serialize + std::fmt::Debug,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{value:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_available_maps_positive() {
        assert_eq!(
            tone_for_evidence_state(EvidenceState::Available),
            RunDashboardValueTone::Positive
        );
    }

    #[test]
    fn evidence_missing_maps_error() {
        assert_eq!(
            tone_for_evidence_state(EvidenceState::Missing),
            RunDashboardValueTone::Error
        );
    }

    #[test]
    fn status_complete_and_available_are_positive() {
        assert_eq!(
            tone_for_status_label("complete"),
            RunDashboardValueTone::Positive
        );
        assert_eq!(
            tone_for_status_label("available"),
            RunDashboardValueTone::Positive
        );
    }

    #[test]
    fn status_failed_and_missing_are_error() {
        assert_eq!(
            tone_for_status_label("failed"),
            RunDashboardValueTone::Error
        );
        assert_eq!(
            tone_for_status_label("missing"),
            RunDashboardValueTone::Error
        );
    }

    #[test]
    fn failed_tool_calls_error_only_when_positive() {
        assert_eq!(
            tone_for_failed_tool_calls(0),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(tone_for_failed_tool_calls(1), RunDashboardValueTone::Error);
    }

    #[test]
    fn tool_steps_succeeded_count_clamps_failed_and_subtracts() {
        assert_eq!(tool_steps_succeeded_count(47, 8), 39);
        assert_eq!(tool_steps_succeeded_count(10, 0), 10);
        assert_eq!(tool_steps_succeeded_count(5, 5), 0);
        assert_eq!(tool_steps_succeeded_count(3, 99), 0);
    }

    #[test]
    fn missing_reviews_warn_when_positive() {
        assert_eq!(
            tone_for_missing_call_reviews(0),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(
            tone_for_missing_call_reviews(2),
            RunDashboardValueTone::Warn
        );
    }

    #[test]
    fn status_label_matching_is_case_insensitive() {
        assert_eq!(
            tone_for_status_label("Complete"),
            RunDashboardValueTone::Positive
        );
        assert_eq!(
            tone_for_status_label(" FAILED "),
            RunDashboardValueTone::Error
        );
    }

    #[test]
    fn analyst_snapshot_positive_outcomes() {
        assert_eq!(
            tone_for_analyst_snapshot_metric("focused progress"),
            RunDashboardValueTone::Positive
        );
        assert_eq!(
            tone_for_analyst_snapshot_metric("applied artifacts"),
            RunDashboardValueTone::Positive
        );
    }

    #[test]
    fn analyst_snapshot_error_and_warn_metrics() {
        assert_eq!(
            tone_for_analyst_snapshot_metric("failed tools"),
            RunDashboardValueTone::Error
        );
        assert_eq!(
            tone_for_analyst_snapshot_metric("redundant thrash"),
            RunDashboardValueTone::Error
        );
        assert_eq!(
            tone_for_analyst_snapshot_metric("missing call reviews"),
            RunDashboardValueTone::Warn
        );
    }

    #[test]
    fn analyst_snapshot_volume_metrics_neutral() {
        assert_eq!(
            tone_for_analyst_snapshot_metric("tool calls"),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(
            tone_for_analyst_snapshot_metric("edit proposals"),
            RunDashboardValueTone::Neutral
        );
    }

    #[test]
    fn analyst_snapshot_zero_value_forces_neutral() {
        assert_eq!(
            effective_tone_for_analyst_snapshot_metric("failed tools", 0.0),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(
            effective_tone_for_analyst_snapshot_metric("focused progress", 0.0),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(
            effective_tone_for_analyst_snapshot_metric("failed tools", 2.0),
            RunDashboardValueTone::Error
        );
    }

    #[test]
    fn protocol_overall_keys_map_to_analyst_tones() {
        assert_eq!(
            tone_for_protocol_overall_verdict_key("focused_progress"),
            RunDashboardValueTone::Positive
        );
        assert_eq!(
            tone_for_protocol_overall_verdict_key("redundant_thrash"),
            RunDashboardValueTone::Error
        );
    }

    #[test]
    fn protocol_issue_count_neutral_at_zero() {
        assert_eq!(
            tone_for_protocol_issue_count(0),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(
            tone_for_protocol_issue_count(3),
            RunDashboardValueTone::Warn
        );
    }

    #[test]
    fn effective_tone_for_count_zero_is_neutral() {
        assert_eq!(
            effective_tone_for_count(RunDashboardValueTone::Error, 0),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(
            effective_tone_for_count(RunDashboardValueTone::Error, 1),
            RunDashboardValueTone::Error
        );
    }

    #[test]
    fn local_analysis_signal_failed_calls_error_when_positive() {
        assert_eq!(
            effective_tone_for_local_analysis_signal_metric("failed calls", 0),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(
            effective_tone_for_local_analysis_signal_metric("failed calls", 1),
            RunDashboardValueTone::Error
        );
    }

    #[test]
    fn local_analysis_signal_repeated_tools_warn_when_positive() {
        assert_eq!(
            effective_tone_for_local_analysis_signal_metric("repeated tools", 0),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(
            effective_tone_for_local_analysis_signal_metric("repeated tools", 2),
            RunDashboardValueTone::Warn
        );
    }

    #[test]
    fn local_analysis_signal_volume_metrics_stay_neutral() {
        assert_eq!(
            effective_tone_for_local_analysis_signal_metric("turns", 3),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(
            effective_tone_for_local_analysis_signal_metric("search calls", 4),
            RunDashboardValueTone::Neutral
        );
        assert_eq!(
            effective_tone_for_local_analysis_signal_metric("distinct tools", 2),
            RunDashboardValueTone::Neutral
        );
    }

    #[test]
    fn local_analysis_signal_similar_searches_warn_when_positive() {
        assert_eq!(
            effective_tone_for_local_analysis_signal_metric("similar searches", 1),
            RunDashboardValueTone::Warn
        );
    }

    #[test]
    fn local_analysis_concern_tones() {
        use ploke_protocol::Concern;

        assert_eq!(
            tone_for_local_analysis_concern(&Concern::RecoveryOpportunity),
            RunDashboardValueTone::Warn
        );
        assert_eq!(
            tone_for_local_analysis_concern(&Concern::SearchThrashRisk),
            RunDashboardValueTone::Error
        );
    }
}
