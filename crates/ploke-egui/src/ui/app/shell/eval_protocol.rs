use crate::ui::eval_protocol::EvalProtocolDashboard;
use crate::ui::eval_protocol::EvalProtocolVisualSummary;
use eframe::egui;
use ploke_records::protocol::ArtifactBody;
use ploke_tree::Graph;
use std::sync::Arc;

use super::cache::scope_eval_pane_content_width;
use super::call_review::{render_call_review_scan, render_run_synthesis_spotlight};
use super::context_strip::render_run_evidence_context_strip;
use super::fields::*;
use super::{
    EvalProtocolRenderMode, InspectorRenderCache, closure_status_label,
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
    scope_eval_pane_content_width(ui, |ui| {
        render_eval_protocol_for_graph_scoped(ui, graph, render_cache);
    });
}

fn render_eval_protocol_for_graph_scoped(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
) {
    let dashboard = EvalProtocolDashboard::from_graph(graph);
    if !dashboard.is_available() {
        cached_kv_text(ui, render_cache, "eval protocol", "not_available");
        return;
    }

    render_run_evidence_context_strip(
        ui,
        render_cache.run_evidence_source_label(),
        render_cache.run_evidence_catalog_error(),
    );
    render_eval_protocol_dashboard_header(ui, render_cache, &dashboard);
    render_eval_protocol_visual_summary(ui, render_cache, dashboard.visual_summary());

    if dashboard.protocol_artifacts().is_some() {
        render_call_review_scan(
            ui,
            render_cache,
            dashboard.protocol_artifacts().expect("checked"),
        );
    }

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Closure evidence").default_open(false),
        |ui| {
            render_eval_protocol_closure_detail(ui, render_cache, &dashboard);
        },
    );

    if dashboard.run_records().is_some() {
        show_inspector_collapsing(
            ui,
            egui::CollapsingHeader::new("Eval run records").default_open(false),
            |ui| {
                render_eval_protocol_run_records_detail(ui, render_cache, &dashboard);
            },
        );
    }

    if dashboard.protocol_artifacts().is_some() {
        show_inspector_collapsing(
            ui,
            egui::CollapsingHeader::new("Protocol artifacts").default_open(false),
            |ui| {
                render_eval_protocol_protocol_detail(ui, render_cache, &dashboard);
            },
        );
    }
}

fn render_eval_protocol_dashboard_header(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dashboard: &EvalProtocolDashboard<'_>,
) {
    super::run_dashboard::render_run_dashboard_status_board(ui, render_cache, dashboard);
    if let Some(protocol_artifacts) = dashboard.protocol_artifacts() {
        render_run_synthesis_spotlight(ui, render_cache, protocol_artifacts);
    }
}

fn render_eval_protocol_closure_detail(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dashboard: &EvalProtocolDashboard<'_>,
) {
    let Some(closure) = dashboard.closure() else {
        cached_kv_text(ui, render_cache, "closure", "not_available");
        return;
    };

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
    if let Some(model) = closure.state.config.model_id.as_deref() {
        cached_kv_id(ui, render_cache, "model", model);
    }
    if let Some(provider) = closure.state.config.provider_slug.as_deref() {
        cached_kv_id(ui, render_cache, "provider", provider);
    }

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

fn render_eval_protocol_run_records_detail(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dashboard: &EvalProtocolDashboard<'_>,
) {
    let Some(run_records) = dashboard.run_records() else {
        cached_kv_text(ui, render_cache, "run records", "not_available");
        return;
    };

    render_run_evidence_context_strip(
        ui,
        render_cache.run_evidence_source_label(),
        render_cache.run_evidence_catalog_error(),
    );

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

fn render_eval_protocol_protocol_detail(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    dashboard: &EvalProtocolDashboard<'_>,
) {
    if dashboard.protocol_artifacts().is_none() {
        cached_kv_text(ui, render_cache, "protocol", "not_available");
        return;
    }

    super::protocol_artifacts_panel::render_protocol_artifacts_summary(ui, render_cache, dashboard);
    render_protocol_artifact_drilldowns(ui, render_cache, dashboard);
}

pub(crate) fn render_eval_protocol_pane(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
    mode: EvalProtocolRenderMode,
) {
    scope_eval_pane_content_width(ui, |ui| match mode {
        EvalProtocolRenderMode::Full => {
            render_eval_protocol_for_graph_scoped(ui, graph, render_cache)
        }
        EvalProtocolRenderMode::CallReviewScanOnly => {
            render_eval_protocol_call_review_scan_for_graph(ui, graph, render_cache);
        }
    });
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
            super::analyst_snapshot::render_analyst_snapshot_panel(ui, summary);

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

/// Resolves a pinned call-review artifact key to its `ToolCallReview` payload.
pub(crate) fn tool_call_review_for_artifact_key<'a>(
    artifact_key: &str,
    protocol_artifacts: &'a ploke_tree::ProtocolArtifactsEvidence,
) -> Option<&'a ploke_records::protocol::ToolCallReviewPayload> {
    let artifact = protocol_artifacts.index.get(artifact_key)?;
    let ArtifactBody::ToolCallReview(payload) = &artifact.body else {
        return None;
    };
    Some(payload)
}

/// Selected scan row's call review, if any.
///
/// Run synthesis is per-call [`ploke_protocol::LocalAnalysisAssessment::synthesis_rationale`]
/// on the selected `ToolCallReview` artifact. The protocol graph has no separate run-level
/// synthesis field; when nothing is selected, focal synthesis stays empty until the operator
/// picks a row in Call Review Scan.
pub(crate) fn selected_tool_call_review<'a>(
    ui: &egui::Ui,
    protocol_artifacts: &'a ploke_tree::ProtocolArtifactsEvidence,
) -> Option<(Arc<str>, &'a ploke_records::protocol::ToolCallReviewPayload)> {
    let artifact_key = selected_eval_protocol_call_review_key(ui)?;
    let payload = tool_call_review_for_artifact_key(artifact_key.as_ref(), protocol_artifacts)?;
    Some((artifact_key, payload))
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

#[cfg(test)]
mod selection_tests {
    use super::tool_call_review_for_artifact_key;
    use ploke_tree::ProtocolArtifactsEvidence;

    #[test]
    fn tool_call_review_for_artifact_key_returns_none_for_missing_or_empty_index() {
        let artifacts = ProtocolArtifactsEvidence::default();
        assert!(tool_call_review_for_artifact_key("missing.json", &artifacts).is_none());
    }
}
