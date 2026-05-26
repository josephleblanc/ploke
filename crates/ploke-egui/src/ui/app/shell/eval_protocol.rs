use crate::ui::eval_protocol::EvalProtocolDashboard;
use crate::ui::eval_protocol::EvalProtocolVisualSummary;
use crate::ui::id_display;
use crate::ui::id_display::{CopyableExpandable, CopyableText};
use eframe::egui;
use ploke_records::protocol::ArtifactBody;
use ploke_tree::Graph;
use std::sync::Arc;

use super::call_review::{
    confidence_emphasis, confidence_label, overall_verdict_emphasis, overall_verdict_label,
    render_call_review_scan, scan_value_label,
};
use super::fields::*;
use super::{
    EvalProtocolRenderMode, InspectorRenderCache, closure_status_label, evidence_state_label,
    patch_projection_check_state_label, render_intent_segmentation_artifact,
    render_patch_projection_counts, render_tool_call_review_artifact,
    render_tool_call_review_detail, render_tool_call_segment_review_artifact,
    show_inspector_collapsing, submission_artifact_state_label,
};

pub(crate) fn render_eval_protocol_for_graph(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
) {
    let dashboard = EvalProtocolDashboard::from_graph(graph);
    if !dashboard.is_available() {
        cached_kv_text(ui, render_cache, "eval protocol", "not_available");
        return;
    }

    let availability = dashboard.availability();
    cached_kv_text(
        ui,
        render_cache,
        "closure evidence",
        evidence_state_label(availability.closure),
    );
    cached_kv_text(
        ui,
        render_cache,
        "run record evidence",
        evidence_state_label(availability.run_records),
    );
    cached_kv_text(
        ui,
        render_cache,
        "protocol evidence",
        evidence_state_label(availability.protocol_artifacts),
    );

    if let Some(closure) = dashboard.closure() {
        cached_kv_path(
            ui,
            render_cache,
            "closure state",
            closure.source_path.to_str().unwrap_or("non_utf8_path"),
        );
        cached_kv_id(
            ui,
            render_cache,
            "campaign",
            closure.state.campaign_id.as_str(),
        );
        cached_kv_text(
            ui,
            render_cache,
            "registry",
            closure_status_label(&closure.state.registry.status).as_str(),
        );
        cached_kv_text(
            ui,
            render_cache,
            "eval",
            closure_status_label(&closure.state.eval.status).as_str(),
        );
        cached_kv_text(
            ui,
            render_cache,
            "protocol",
            closure_status_label(&closure.state.protocol.status).as_str(),
        );
        if let Some(model) = closure.state.config.model_id.as_deref() {
            cached_kv_id(ui, render_cache, "model", model);
        }
        if let Some(provider) = closure.state.config.provider_slug.as_deref() {
            cached_kv_id(ui, render_cache, "provider", provider);
        }
        cached_kv_usize(ui, render_cache, "instances", closure.state.instances.len());

        for instance in closure.state.instances.iter().take(3) {
            ui.separator();
            cached_kv_id(ui, render_cache, "instance", instance.instance_id.as_str());
            cached_kv_text(
                ui,
                render_cache,
                "instance eval",
                closure_status_label(&instance.eval_status).as_str(),
            );
            cached_kv_text(
                ui,
                render_cache,
                "instance protocol",
                closure_status_label(&instance.protocol_status).as_str(),
            );
            if let Some(counts) = instance.protocol_counts.as_ref() {
                cached_kv_usize(ui, render_cache, "reviewed calls", counts.reviewed_calls);
                cached_kv_usize(ui, render_cache, "total calls", counts.total_calls);
                cached_kv_usize(ui, render_cache, "usable segments", counts.usable_segments);
                cached_kv_usize(ui, render_cache, "total segments", counts.total_segments);
            }
        }
        if closure.state.instances.len() > 3 {
            cached_kv_usize(
                ui,
                render_cache,
                "more instances",
                closure.state.instances.len() - 3,
            );
        }
    }

    if let Some(run_records) = dashboard.run_records() {
        ui.separator();
        ui.label(egui::RichText::new("Eval Run Records").strong());
        cached_kv_optional_usize(
            ui,
            render_cache,
            "record files",
            dashboard.run_records_file_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "records",
            dashboard.run_records_parsed_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "branch refs",
            dashboard.run_records_branch_ref_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "turns",
            dashboard.run_records_total_turn_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "tool calls",
            dashboard.run_records_total_tool_call_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "failed tool calls",
            dashboard.run_records_failed_tool_call_count(),
        );

        let patch = dashboard.eval_patch_counts();
        cached_kv_usize(ui, render_cache, "patch phases", patch.patch_phase_count);
        cached_kv_usize(
            ui,
            render_cache,
            "empty submissions",
            patch.empty_submission_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "nonempty submissions",
            patch.nonempty_submission_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "edit proposals",
            patch.edit_proposal_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "create proposals",
            patch.create_proposal_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "expected file changes",
            patch.expected_file_change_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "applied patch artifacts",
            patch.applied_patch_artifact_count,
        );
        render_patch_projection_counts(ui, render_cache, &patch.patch_projection);

        for (record_key, record) in run_records.index.iter().take(2) {
            ui.separator();
            cached_kv_path(ui, render_cache, "record", record_key.as_str());
            cached_kv_id(ui, render_cache, "manifest", record.manifest_id.as_str());
            cached_kv_id(
                ui,
                render_cache,
                "instance",
                record.metadata.benchmark.instance_id.as_str(),
            );
            if let Some(model) = record.metadata.agent.model_id.as_deref() {
                cached_kv_id(ui, render_cache, "record model", model);
            }
            if let Some(provider) = record.metadata.agent.provider.as_deref() {
                cached_kv_id(ui, render_cache, "record provider", provider);
            }
            if let Some(timing) = record.timing.as_ref() {
                cached_kv_text(
                    ui,
                    render_cache,
                    "wall clock",
                    format!("{:.3}s", timing.total_wall_clock_secs).as_str(),
                );
                cached_kv_optional_f64(
                    ui,
                    render_cache,
                    "agent clock",
                    timing.agent_wall_clock_secs,
                );
            }
            if let Some(packaging) = record.phases.packaging.as_ref() {
                cached_kv_text(
                    ui,
                    render_cache,
                    "submission",
                    submission_artifact_state_label(packaging.submission_artifact_state),
                );
                cached_kv_text(
                    ui,
                    render_cache,
                    "projection",
                    patch_projection_check_state_label(packaging.patch_projection_check_state),
                );
            }
        }
        if run_records.index.len() > 2 {
            cached_kv_usize(
                ui,
                render_cache,
                "more records",
                run_records.index.len() - 2,
            );
        }
    }

    if dashboard.protocol_artifacts().is_some() {
        ui.separator();
        ui.label(egui::RichText::new("Protocol").strong());
        cached_kv_optional_usize(
            ui,
            render_cache,
            "artifact files",
            dashboard.protocol_artifacts_file_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "artifacts",
            dashboard.protocol_artifacts_parsed_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "intent segments",
            dashboard.protocol_artifacts_intent_segmentation_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "call reviews",
            dashboard.protocol_artifacts_review_count(),
        );
        cached_kv_optional_usize(
            ui,
            render_cache,
            "segment reviews",
            dashboard.protocol_artifacts_segment_review_count(),
        );

        let stats = dashboard.protocol_aggregate_counts();
        cached_kv_usize(
            ui,
            render_cache,
            "issue detections",
            stats.issue_detection_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "issue cases",
            stats.issue_detection_case_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "interventions",
            stats.intervention_candidate_count,
        );
        cached_kv_usize(
            ui,
            render_cache,
            "intervention applies",
            stats.intervention_apply_count,
        );
        render_eval_protocol_visual_summary(ui, render_cache, dashboard.visual_summary());
        render_protocol_artifact_drilldowns(ui, render_cache, &dashboard);
    }
}

pub(crate) fn render_eval_protocol_pane(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
    mode: EvalProtocolRenderMode,
) {
    match mode {
        EvalProtocolRenderMode::Full => render_eval_protocol_for_graph(ui, graph, render_cache),
        EvalProtocolRenderMode::CallReviewScanOnly => {
            render_eval_protocol_call_review_scan_for_graph(ui, graph, render_cache);
        }
    }
}

fn render_eval_protocol_call_review_scan_for_graph(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
) {
    let dashboard = EvalProtocolDashboard::from_graph(graph);
    let Some(protocol_artifacts) = dashboard.protocol_artifacts() else {
        cached_kv_text(ui, render_cache, "call review scan", "not_available");
        return;
    };
    render_call_review_scan(ui, render_cache, protocol_artifacts);
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::EVAL_PROTOCOL_VISUAL_SUMMARY)]
fn render_eval_protocol_visual_summary(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    summary: EvalProtocolVisualSummary,
) {
    if !summary.has_any_visual_data() {
        return;
    }

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Analyst Snapshot").default_open(true),
        |ui| {
            let volume = [
                ("records", summary.run_records as f32),
                ("turns", summary.turns as f32),
                ("tool calls", summary.tool_calls as f32),
                ("failed tools", summary.failed_tool_calls as f32),
            ];
            crate::ui::charts::horizontal_bar_chart(ui, "Run Effort", &volume);

            ui.separator();
            let coverage = [
                ("reviewed calls", summary.reviewed_calls as f32),
                (
                    "missing call reviews",
                    summary.missing_call_reviews() as f32,
                ),
                ("usable segments", summary.usable_segments as f32),
                ("mismatched segments", summary.mismatched_segments as f32),
                ("missing segments", summary.missing_segments as f32),
            ];
            crate::ui::charts::horizontal_bar_chart(ui, "Protocol Coverage", &coverage);

            let outcomes = summary
                .call_review_outcomes
                .combined(summary.segment_review_outcomes);
            if outcomes.total() > 0 {
                ui.separator();
                let data = [
                    ("focused progress", outcomes.focused_progress as f32),
                    ("useful exploration", outcomes.useful_exploration as f32),
                    ("recoverable detour", outcomes.recoverable_detour as f32),
                    ("redundant thrash", outcomes.redundant_thrash as f32),
                    ("mixed", outcomes.mixed as f32),
                    ("unclear", outcomes.unclear as f32),
                ];
                crate::ui::charts::horizontal_bar_chart(ui, "Review Outcome Mix", &data);
            }

            if summary.patch.edit_proposal_count > 0
                || summary.patch.create_proposal_count > 0
                || summary.patch.expected_file_change_count > 0
                || summary.patch.applied_patch_artifact_count > 0
            {
                ui.separator();
                let patch = [
                    ("edit proposals", summary.patch.edit_proposal_count as f32),
                    (
                        "create proposals",
                        summary.patch.create_proposal_count as f32,
                    ),
                    (
                        "expected file changes",
                        summary.patch.expected_file_change_count as f32,
                    ),
                    (
                        "applied artifacts",
                        summary.patch.applied_patch_artifact_count as f32,
                    ),
                ];
                crate::ui::charts::horizontal_bar_chart(ui, "Patch Production", &patch);
            }

            ui.separator();
            cached_kv_usize(ui, render_cache, "total calls", summary.total_calls);
            cached_kv_usize(ui, render_cache, "total segments", summary.total_segments);
        },
    );
}

fn render_protocol_artifact_drilldowns(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dashboard: &EvalProtocolDashboard<'_>,
) {
    let Some(protocol_artifacts) = dashboard.protocol_artifacts() else {
        return;
    };

    let review_count = protocol_artifacts
        .index
        .values()
        .filter(|artifact| matches!(&artifact.body, ArtifactBody::ToolCallReview(_)))
        .count();
    if review_count > 0 {
        render_call_review_scan(ui, render_cache, protocol_artifacts);
        show_inspector_collapsing(
            ui,
            egui::CollapsingHeader::new("Reviewed Calls").default_open(false),
            |ui| {
                cached_kv_usize(ui, render_cache, "reviewed calls", review_count);
                for (review_index, (artifact_key, artifact)) in protocol_artifacts
                    .index
                    .iter()
                    .filter(|(_, artifact)| {
                        matches!(&artifact.body, ArtifactBody::ToolCallReview(_))
                    })
                    .enumerate()
                {
                    let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
                        continue;
                    };
                    render_tool_call_review_artifact(
                        ui,
                        render_cache,
                        review_index,
                        artifact_key,
                        artifact,
                        payload,
                    );
                }
            },
        );
    }

    let segment_count = protocol_artifacts
        .index
        .values()
        .filter(|artifact| matches!(&artifact.body, ArtifactBody::ToolCallSegmentReview(_)))
        .count();
    if segment_count > 0 {
        show_inspector_collapsing(
            ui,
            egui::CollapsingHeader::new("Usable Segment Reviews").default_open(false),
            |ui| {
                cached_kv_usize(ui, render_cache, "segment reviews", segment_count);
                for (review_index, (artifact_key, artifact)) in protocol_artifacts
                    .index
                    .iter()
                    .filter(|(_, artifact)| {
                        matches!(&artifact.body, ArtifactBody::ToolCallSegmentReview(_))
                    })
                    .enumerate()
                {
                    let ArtifactBody::ToolCallSegmentReview(payload) = &artifact.body else {
                        continue;
                    };
                    render_tool_call_segment_review_artifact(
                        ui,
                        render_cache,
                        review_index,
                        artifact_key,
                        artifact,
                        payload,
                    );
                }
            },
        );
    }

    let segmentation_count = protocol_artifacts
        .index
        .values()
        .filter(|artifact| matches!(&artifact.body, ArtifactBody::ToolCallIntentSegmentation(_)))
        .count();
    if segmentation_count > 0 {
        show_inspector_collapsing(
            ui,
            egui::CollapsingHeader::new("Intent Segmentation").default_open(false),
            |ui| {
                cached_kv_usize(ui, render_cache, "segmentations", segmentation_count);
                for (artifact_index, (artifact_key, artifact)) in protocol_artifacts
                    .index
                    .iter()
                    .filter(|(_, artifact)| {
                        matches!(&artifact.body, ArtifactBody::ToolCallIntentSegmentation(_))
                    })
                    .enumerate()
                {
                    let ArtifactBody::ToolCallIntentSegmentation(payload) = &artifact.body else {
                        continue;
                    };
                    render_intent_segmentation_artifact(
                        ui,
                        render_cache,
                        artifact_index,
                        artifact_key,
                        artifact,
                        payload,
                    );
                }
            },
        );
    }
}

fn selected_eval_protocol_call_review_id() -> egui::Id {
    egui::Id::new("ploke-egui.eval-protocol.selected-call-review")
}

pub(super) fn selected_eval_protocol_call_review_key(ui: &egui::Ui) -> Option<Arc<str>> {
    ui.data(|data| data.get_temp::<Arc<str>>(selected_eval_protocol_call_review_id()))
}

pub(super) fn set_selected_eval_protocol_call_review(ui: &mut egui::Ui, artifact_key: &str) {
    ui.data_mut(|data| {
        data.insert_temp(
            selected_eval_protocol_call_review_id(),
            Arc::<str>::from(artifact_key),
        );
    });
}

pub(super) fn clear_selected_eval_protocol_call_review(ui: &mut egui::Ui) {
    ui.data_mut(|data| {
        let _ = data.remove_temp::<Arc<str>>(selected_eval_protocol_call_review_id());
    });
}

pub(super) fn render_selected_eval_protocol_call_review(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    artifact_key: &str,
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) {
    let Some(artifact) = protocol_artifacts.index.get(artifact_key) else {
        cached_kv_id(ui, render_cache, "call review", "not_available");
        cached_kv_artifact_file(ui, render_cache, "artifact", artifact_key);
        return;
    };

    let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
        cached_kv_id(ui, render_cache, "call review", "not_applicable");
        cached_kv_artifact_file(ui, render_cache, "artifact", artifact_key);
        return;
    };

    render_tool_call_review_detail(ui, render_cache, artifact_key, artifact, payload, true);
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::EVAL_PROTOCOL_CALL_REVIEW_SPOTLIGHT)]
pub(super) fn render_call_review_reasoning_spotlight(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    protocol_artifacts: &ploke_tree::ProtocolArtifactsEvidence,
) {
    let selected_artifact_key = selected_eval_protocol_call_review_key(ui);
    ui.add_space(4.0);
    egui::Frame::group(ui.style())
        .fill(ui.visuals().widgets.active.weak_bg_fill)
        .stroke(egui::Stroke::new(1.0, ui.visuals().selection.bg_fill))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                cached_label(ui, render_cache, "LLM Reasoning Spotlight");
                if let Some(artifact_key) = selected_artifact_key.as_deref() {
                    cached_artifact_file_value(
                        ui,
                        render_cache,
                        ("call-review-spotlight-artifact", artifact_key),
                        artifact_key,
                    );
                }
            });

            let Some(artifact_key) = selected_artifact_key.as_deref() else {
                cached_label(
                    ui,
                    render_cache,
                    "Select a call row to pin its LLM reasoning here.",
                );
                return;
            };
            let Some(artifact) = protocol_artifacts.index.get(artifact_key) else {
                cached_kv_artifact_file(ui, render_cache, "selected", artifact_key);
                cached_kv_id(ui, render_cache, "reasoning", "not_available");
                return;
            };
            let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
                cached_kv_artifact_file(ui, render_cache, "selected", artifact_key);
                cached_kv_id(ui, render_cache, "reasoning", "not_applicable");
                return;
            };

            let focal = &payload.input.focal;
            ui.horizontal_wrapped(|ui| {
                cached_label(ui, render_cache, "call");
                let mut index_buffer = itoa::Buffer::new();
                cached_monospace_label(ui, render_cache, index_buffer.format(focal.index));
                cached_monospace_label(ui, render_cache, focal.tool_name.as_str());
                scan_value_label(
                    ui,
                    render_cache,
                    overall_verdict_label(payload.output.overall),
                    overall_verdict_emphasis(payload.output.overall),
                );
                scan_value_label(
                    ui,
                    render_cache,
                    confidence_label(payload.output.overall_confidence),
                    confidence_emphasis(payload.output.overall_confidence),
                );
            });

            ui.horizontal(|ui| {
                cached_label(ui, render_cache, "synthesis");
                let copy_value = CopyableText::new(payload.output.synthesis_rationale.as_str());
                id_display::copy_button(ui, &copy_value);
            });
            let copy_value = CopyableText::new(payload.output.synthesis_rationale.as_str());
            let response = cached_wrapped_monospace_label(
                ui,
                render_cache,
                payload.output.synthesis_rationale.as_str(),
            )
            .on_hover_text(copy_value.hover_text(false, false));
            id_display::attach_copy_context_menu(&response, &copy_value);
        });
}
