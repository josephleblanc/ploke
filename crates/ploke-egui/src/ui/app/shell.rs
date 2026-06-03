//! Stable frame rendering for the operator graph UI.

use crate::ui::render::text::*;
use eframe::egui;

use crate::allocation::scope;
use crate::ui::eval_protocol::{EvalProtocolDashboard, EvidenceState, PatchProjectionCounts};
use crate::ui::inspector::{
    InspectorSections, PatchInspection, SelectionEdge, UnavailableReason,
    response_finish_reason_label, surface_apply_status_label, surface_check_status_label,
    turn_outcome_elapsed_secs, turn_outcome_error, turn_outcome_label, turn_outcome_tool_count,
};
use ploke_tree::Graph;
use ploke_tree::graph::AgentTurnArtifactMetadata;
use std::sync::Arc;

mod cache;
mod call_review;
mod chrome;
mod eval_protocol;
pub(super) mod fields;
mod identity;
mod inspector;
mod llm_trace;
mod parent_create;
mod protocol_detail;
mod run_records;
mod timeline;
use self::fields::*;
use self::llm_trace::render_run_record_turn_llm_trace;
use self::run_records::{
    patch_projection_check_state_label, render_run_record_tool_steps, render_run_record_turns,
    render_token_usage, submission_artifact_state_label,
};
#[cfg(not(target_arch = "wasm32"))]
use self::run_records::{render_decoded_tool_arguments, render_decoded_tool_result};
pub(crate) use cache::InspectorRenderCache;
pub(crate) use chrome::{add_inspector_scroll_end_padding, render_top_strip};
pub(crate) use eval_protocol::{render_eval_protocol_for_graph, render_eval_protocol_pane};
pub(crate) use identity::{
    render_artifact_ids_for_inspector, render_identity, render_lineage_authority_for_inspector,
    render_roles_and_metrics, render_source_refs_for_inspector,
};
pub(crate) use inspector::{
    EvalProtocolRenderMode, InspectorOpenState, InspectorPanelSection, render_right_inspector,
};
pub(super) use llm_trace::render_run_level_llm_trace_for_graph;
#[allow(unused_imports)]
pub(crate) use parent_create::{
    render_parent_create_for_inspector, render_parent_create_llm_calls,
    render_parent_create_llm_calls_body,
};
pub(super) use protocol_detail::{
    render_intent_segmentation_artifact, render_tool_call_review_artifact,
    render_tool_call_review_detail, render_tool_call_segment_review_artifact,
};
pub(crate) use run_records::render_run_records_for_inspector;
pub(crate) use timeline::render_bottom_timeline;
#[cfg(test)]
mod render_cache_tests;

fn show_inspector_collapsing<R>(
    ui: &mut egui::Ui,
    header: egui::CollapsingHeader,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) {
    let _span = tracing::trace_span!(scope::INSPECTOR_COLLAPSING_HEADER_LAYOUT).entered();
    header.show(ui, add_body);
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::INSPECTOR_PATCH_GENERATION)]
fn render_run_level_patch_generation_for_graph(
    ui: &mut egui::Ui,
    graph: &Graph,
    render_cache: &mut InspectorRenderCache,
) {
    let dashboard = EvalProtocolDashboard::from_graph(graph);
    let Some(run_records) = dashboard.run_records() else {
        cached_kv_id(ui, render_cache, "patch generation", "not_available");
        return;
    };
    if run_records.index.is_empty() {
        cached_kv_id(ui, render_cache, "patch generation", "none");
        return;
    }

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Run Patch Generation").default_open(true),
        |ui| {
            cached_kv_usize(ui, render_cache, "records", run_records.index.len());
            for (record_index, (record_key, record)) in run_records.index.iter().enumerate() {
                render_patch_generation_record(ui, render_cache, record_index, record_key, record);
            }
        },
    );
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::INSPECTOR_PATCH_GENERATION_RECORD)]
fn render_patch_generation_record(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    record_index: usize,
    record_key: &str,
    record: &ploke_records::run_record::RunRecord,
) {
    let title = format!(
        "{} {} patch trace",
        record.metadata.benchmark.instance_id, record.manifest_id
    );
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt(("patch-generation-record", record_key, record_index))
            .default_open(record_index == 0),
        |ui| {
            cached_kv_path(ui, render_cache, "record", record_key);
            if let Some(patch_phase) = record.phases.patch.as_ref() {
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new("patch phase").default_open(true),
                    |ui| {
                        cached_kv_text(
                            ui,
                            render_cache,
                            "started",
                            patch_phase.started_at.as_str(),
                        );
                        cached_kv_text(ui, render_cache, "ended", patch_phase.ended_at.as_str());
                        render_patch_artifact_details(
                            ui,
                            render_cache,
                            "patch-generation",
                            "patch-phase",
                            record_key,
                            0,
                            &patch_phase.patch_artifact,
                        );
                        if let Some(diff) = patch_phase.diff.as_deref() {
                            cached_label(ui, render_cache, "diff");
                            render_cached_code_block(
                                ui,
                                render_cache,
                                ("patch-generation-diff", record_key),
                                diff,
                            );
                        }
                    },
                );
            } else {
                cached_kv_id(ui, render_cache, "patch phase", "not_recorded");
            }

            let mut patch_turns = 0usize;
            for (turn_index, turn) in record.phases.agent_turns.iter().enumerate() {
                if turn.agent_turn_artifact.is_none() {
                    continue;
                }
                patch_turns += 1;
                render_run_record_turn_llm_trace(
                    ui,
                    render_cache,
                    "patch-generation",
                    record_key,
                    turn_index,
                    record,
                    turn,
                    true,
                );
            }
            if patch_turns == 0 {
                cached_kv_id(ui, render_cache, "agent patch artifacts", "not_recorded");
            }
        },
    );
}

fn render_patch_artifact_details(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    id_kind: &'static str,
    record_key: &str,
    turn_index: usize,
    artifact: &ploke_records::agent_turn::PatchArtifactRecord,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("patch artifact")
            .id_salt((scope, id_kind, "patch-artifact", record_key, turn_index))
            .default_open(false),
        |ui| {
            cached_kv_bool(ui, render_cache, "applied", artifact.applied);
            cached_kv_bool(
                ui,
                render_cache,
                "all proposals applied",
                artifact.all_proposals_applied,
            );
            cached_kv_bool(
                ui,
                render_cache,
                "any expected changed",
                artifact.any_expected_file_changed,
            );
            cached_kv_bool(
                ui,
                render_cache,
                "all expected changed",
                artifact.all_expected_files_changed,
            );
            render_patch_proposals(
                ui,
                render_cache,
                scope,
                id_kind,
                record_key,
                turn_index,
                "edit proposals",
                artifact.edit_proposals.as_slice(),
            );
            render_patch_proposals(
                ui,
                render_cache,
                scope,
                id_kind,
                record_key,
                turn_index,
                "create proposals",
                artifact.create_proposals.as_slice(),
            );
            render_expected_file_changes(
                ui,
                render_cache,
                scope,
                id_kind,
                record_key,
                turn_index,
                artifact.expected_file_changes.as_slice(),
            );
        },
    );
}

fn render_patch_proposals(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    id_kind: &'static str,
    record_key: &str,
    turn_index: usize,
    title: &'static str,
    proposals: &[ploke_records::agent_turn::ProposalSnapshotRecord],
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(title)
            .id_salt((scope, id_kind, title, record_key, turn_index))
            .default_open(false),
        |ui| {
            cached_kv_usize(ui, render_cache, "count", proposals.len());
            for (proposal_index, proposal) in proposals.iter().enumerate() {
                let proposal_title = format!("proposal {} {}", proposal_index + 1, proposal.status);
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new(proposal_title)
                        .id_salt((
                            scope,
                            id_kind,
                            title,
                            "proposal",
                            record_key,
                            turn_index,
                            proposal_index,
                        ))
                        .default_open(false),
                    |ui| {
                        cached_kv_id(ui, render_cache, "request", proposal.request_id.as_str());
                        cached_kv_id(ui, render_cache, "call", proposal.call_id.as_str());
                        cached_kv_id(ui, render_cache, "status", proposal.status.as_str());
                        cached_kv_id(ui, render_cache, "preview", proposal.preview_mode.as_str());
                        for file in &proposal.files {
                            cached_kv_path(ui, render_cache, "file", file.as_str());
                        }
                    },
                );
            }
        },
    );
}

fn render_expected_file_changes(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    scope: &'static str,
    id_kind: &'static str,
    record_key: &str,
    turn_index: usize,
    changes: &[ploke_records::agent_turn::ExpectedFileChangeRecord],
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("expected file changes")
            .id_salt((
                scope,
                id_kind,
                "expected-file-changes",
                record_key,
                turn_index,
            ))
            .default_open(false),
        |ui| {
            cached_kv_usize(ui, render_cache, "count", changes.len());
            for (change_index, change) in changes.iter().enumerate() {
                let title = format!("file {} {}", change_index + 1, bool_label(change.changed));
                show_inspector_collapsing(
                    ui,
                    egui::CollapsingHeader::new(title)
                        .id_salt((
                            scope,
                            id_kind,
                            "expected-file-change",
                            record_key,
                            turn_index,
                            change_index,
                        ))
                        .default_open(false),
                    |ui| {
                        cached_kv_path(ui, render_cache, "path", change.path.as_str());
                        cached_kv_bool(ui, render_cache, "existed before", change.existed_before);
                        cached_kv_bool(ui, render_cache, "exists after", change.exists_after);
                        cached_kv_bool(ui, render_cache, "changed", change.changed);
                        if let Some(sha) = change.before_sha256.as_deref() {
                            cached_kv_id(ui, render_cache, "before sha", sha);
                        }
                        if let Some(sha) = change.after_sha256.as_deref() {
                            cached_kv_id(ui, render_cache, "after sha", sha);
                        }
                    },
                );
            }
        },
    );
}

fn render_patch_projection_counts(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    counts: &PatchProjectionCounts,
) {
    cached_kv_usize(
        ui,
        render_cache,
        "patch projection.not_recorded",
        counts.not_recorded,
    );
    cached_kv_usize(
        ui,
        render_cache,
        "patch projection.not_applicable",
        counts.not_applicable,
    );
    cached_kv_usize(ui, render_cache, "patch projection.passed", counts.passed);
    cached_kv_usize(ui, render_cache, "patch projection.failed", counts.failed);
    cached_kv_usize(ui, render_cache, "patch projection.not_run", counts.not_run);
}

fn evidence_state_label(state: EvidenceState) -> &'static str {
    match state {
        EvidenceState::Available => "available",
        EvidenceState::Missing => "missing",
        EvidenceState::NotApplicable => "not_applicable",
    }
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

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_graph_edges")
)]
pub(crate) fn render_graph_edges_for_inspector(
    ui: &mut egui::Ui,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_edges(
        ui,
        render_cache,
        "in",
        sections.graph_edges_in().iter().map(|slot| slot.edge()),
    );
    render_edges(
        ui,
        render_cache,
        "out",
        sections.graph_edges_out().iter().map(|slot| slot.edge()),
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_artifact_edges")
)]
pub(crate) fn render_artifact_edges_for_inspector(
    ui: &mut egui::Ui,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_edges(
        ui,
        render_cache,
        "in",
        sections.artifact_edges_in().iter().map(|slot| slot.edge()),
    );
    render_edges(
        ui,
        render_cache,
        "out",
        sections.artifact_edges_out().iter().map(|slot| slot.edge()),
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_patch_debug")
)]
pub(crate) fn render_patches_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_patches(
        ui,
        render_cache,
        sections
            .patches()
            .iter()
            .filter_map(|slot| slot.resolve(graph)),
        diff_cache,
    );
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_candidate_comparison")
)]
pub(crate) fn render_candidate_comparison_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    let Some(slot) = sections.candidate_comparison() else {
        kv(ui, "candidate comparison", "not_available");
        return;
    };

    cached_kv_id(
        ui,
        render_cache,
        "parent node",
        slot.parent_node_id.as_str(),
    );
    cached_kv_usize(ui, render_cache, "planned children", slot.children().len());
    if let Some(metric_set) = slot.metric_set(graph) {
        cached_kv_id(
            ui,
            render_cache,
            "metric set",
            metric_set.metric_set_id.0.as_str(),
        );
        cached_kv_text(
            ui,
            render_cache,
            "score profile",
            score_profile_label(metric_set.policy.score_profile),
        );
        cached_kv_usize(
            ui,
            render_cache,
            "imp@k budget",
            metric_set.policy.imp_at_k.budget_k,
        );
        cached_kv_i64(
            ui,
            render_cache,
            "imp@k score weight",
            metric_set.policy.imp_at_k.score_points_per_imp_point,
        );
        cached_kv_text(
            ui,
            render_cache,
            "imp@k required",
            bool_label(metric_set.policy.imp_at_k.require_for_score),
        );
    }

    render_candidate_comparison_formula_summary(ui, render_cache, slot.selection_formula(graph));

    egui::ScrollArea::horizontal().show(ui, |ui| {
        egui::Grid::new(("candidate-comparison", slot.parent_node_id.as_str()))
            .striped(true)
            .num_columns(32)
            .show(ui, |ui| {
                cached_label(ui, render_cache, "selected");
                cached_label(ui, render_cache, "child");
                cached_label(ui, render_cache, "candidate");
                cached_label(ui, render_cache, "payload");
                cached_label(ui, render_cache, "outcome");
                cached_label(ui, render_cache, "outcome pts");
                cached_label(ui, render_cache, "operational pts");
                cached_label(ui, render_cache, "protocol pts");
                cached_label(ui, render_cache, "imp@k delta");
                cached_label(ui, render_cache, "performance");
                cached_label(ui, render_cache, "oracle rate");
                cached_label(ui, render_cache, "alpha");
                cached_label(ui, render_cache, "alpha mid");
                cached_label(ui, render_cache, "exploitation");
                cached_label(ui, render_cache, "exploration");
                cached_label(ui, render_cache, "weight");
                cached_label(ui, render_cache, "cumulative");
                cached_label(ui, render_cache, "sample hit");
                cached_label(ui, render_cache, "child count");
                cached_label(ui, render_cache, "selectable");
                cached_label(ui, render_cache, "exclusion");
                cached_label(ui, render_cache, "improvement");
                cached_label(ui, render_cache, "baseline");
                cached_label(ui, render_cache, "best descendant");
                cached_label(ui, render_cache, "scored / descendants");
                cached_label(ui, render_cache, "tool failures delta");
                cached_label(ui, render_cache, "patch failures delta");
                cached_label(ui, render_cache, "valid patch");
                cached_label(ui, render_cache, "converged");
                cached_label(ui, render_cache, "oracle eligible");
                cached_label(ui, render_cache, "protocol reviewed delta");
                cached_label(ui, render_cache, "protocol missing delta");
                ui.end_row();

                for child in slot.children() {
                    if let Some(candidate) = slot.resolve_child(graph, child) {
                        render_candidate_comparison_candidate(ui, render_cache, candidate);
                        ui.end_row();
                    }
                }
            });
    });
}

/// archaeology:score-child-prop-ui
/// proof:docs/active/archaeology/ploke-tree-graph/score-child-prop-ui-spec.md
fn render_candidate_comparison_formula_summary(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    formula: Option<&ploke_tree::graph::SelectionFormulaNode>,
) {
    let Some(formula) = formula else {
        cached_kv_text(ui, render_cache, "selector formula", "not_recorded");
        return;
    };
    match &formula.formula {
        ploke_tree::graph::SelectionFormulaKind::ScoreChildProp(score) => {
            cached_kv_text(ui, render_cache, "selector formula", "score_child_prop");
            cached_kv_id(
                ui,
                render_cache,
                "formula selection entry",
                formula.selection_entry_id.0.as_str(),
            );
            cached_kv_id(
                ui,
                render_cache,
                "formula metric set",
                formula.metric_set_id.0.as_str(),
            );
            cached_kv_u64(ui, render_cache, "seed", score.record.seed);
            cached_kv_usize(ui, render_cache, "top_m", score.record.top_m);
            cached_kv_u32(
                ui,
                render_cache,
                "lambda_millis",
                score.record.lambda_millis,
            );
            cached_kv_f64(ui, render_cache, "lambda", score.record.lambda);
            cached_kv_text(
                ui,
                render_cache,
                "metric inputs",
                score.record.metric_inputs.as_str(),
            );
            cached_kv_text(
                ui,
                render_cache,
                "oracle mode",
                score.record.oracle_mode.as_str(),
            );
            cached_kv_text(
                ui,
                render_cache,
                "oracle required",
                bool_label(score.record.oracle_require_evidence),
            );
            cached_kv_text(
                ui,
                render_cache,
                "alpha source",
                if score.record.oracle_used_for_alpha {
                    "oracle"
                } else {
                    "performance"
                },
            );
            cached_kv_f64(ui, render_cache, "alpha_mid", score.record.alpha_mid);
            cached_kv_f64(ui, render_cache, "total_weight", score.record.total_weight);
            cached_kv_optional_f64(ui, render_cache, "sample", score.record.sample);
            cached_kv_optional_f64(
                ui,
                render_cache,
                "sample threshold",
                score.record.sample_threshold,
            );
            cached_kv_optional_usize(
                ui,
                render_cache,
                "uniform fallback slot",
                score.record.uniform_fallback_slot,
            );
            cached_kv_optional_usize(
                ui,
                render_cache,
                "selected index",
                score.record.selected_index,
            );
            cached_kv_text(
                ui,
                render_cache,
                "selected replay candidate",
                score
                    .record
                    .selected_candidate
                    .as_deref()
                    .unwrap_or("not_recorded"),
            );
        }
    }
}

/// archaeology:score-child-prop-ui
/// proof:docs/active/archaeology/ploke-tree-graph/score-child-prop-ui-spec.md
fn render_candidate_comparison_candidate(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    candidate: crate::ui::inspector::CandidateComparisonCandidate<'_>,
) {
    let row = candidate.selector.and_then(|selector| selector.row);
    cached_label(
        ui,
        render_cache,
        if candidate.selected { "yes" } else { "no" },
    );
    cached_expandable_id(
        ui,
        render_cache,
        (
            "candidate-comparison-child",
            candidate.child.node.node_id.as_str(),
        ),
        candidate.child.node.node_id.as_str(),
    );
    cached_expandable_id(
        ui,
        render_cache,
        (
            "candidate-comparison-candidate",
            candidate.child.node.node_id.as_str(),
        ),
        candidate
            .candidate
            .map(|candidate| candidate.subject.value.as_str())
            .unwrap_or(candidate.child.resolved.branch.candidate_id.as_str()),
    );
    render_optional_usize_value(ui, render_cache, row.map(|row| row.payload_index));
    render_optional_str_value(
        ui,
        render_cache,
        row.map(|row| outcome_label(row.base_outcome)),
    );
    render_optional_i64(ui, render_cache, row.map(|row| row.outcome_points));
    render_optional_i64(ui, render_cache, row.map(|row| row.operational_points));
    render_optional_i64(ui, render_cache, row.map(|row| row.protocol_points));
    render_optional_i64(ui, render_cache, row.and_then(|row| row.imp_at_k_delta));
    render_optional_i64(ui, render_cache, row.and_then(|row| row.performance));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.oracle_rate));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.alpha));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.alpha_mid));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.exploitation));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.exploration));
    render_optional_f64_value(ui, render_cache, row.and_then(|row| row.weight));
    render_optional_f64_range(
        ui,
        render_cache,
        row.and_then(|row| row.cumulative_lower.zip(row.cumulative_upper)),
    );
    render_optional_bool(ui, render_cache, row.map(|row| row.sample_hit));
    render_optional_usize_value(ui, render_cache, row.and_then(|row| row.child_count));
    render_optional_bool(ui, render_cache, row.map(|row| row.selectable));
    render_optional_str_value(
        ui,
        render_cache,
        row.and_then(|row| row.exclusion_reason.as_deref())
            .or(Some("none")),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate
            .metric
            .and_then(|metric| metric.imp_at_k.as_ref())
            .and_then(|imp| imp.improvement),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate
            .metric
            .and_then(|metric| metric.imp_at_k.as_ref())
            .and_then(|imp| imp.baseline_score),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate
            .metric
            .and_then(|metric| metric.imp_at_k.as_ref())
            .and_then(|imp| imp.best_descendant_score),
    );
    render_imp_at_k_counts(ui, render_cache, candidate.metric);
    render_optional_i64(
        ui,
        render_cache,
        candidate.metric.and_then(metric_tool_failures_delta),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate.metric.and_then(metric_patch_failures_delta),
    );
    render_optional_bool(
        ui,
        render_cache,
        candidate.metric.and_then(metric_treatment_valid_patch),
    );
    render_optional_bool(
        ui,
        render_cache,
        candidate.metric.and_then(metric_treatment_converged),
    );
    render_optional_bool(
        ui,
        render_cache,
        candidate.metric.and_then(metric_treatment_oracle_eligible),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate.metric.and_then(metric_protocol_reviewed_delta),
    );
    render_optional_i64(
        ui,
        render_cache,
        candidate.metric.and_then(metric_protocol_missing_delta),
    );
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn score_profile_label(profile: ploke_records::selection::ScoreProfile) -> &'static str {
    match profile {
        ploke_records::selection::ScoreProfile::OperationalQualityV1 => "operational_quality_v1",
    }
}

fn bool_label(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn outcome_label(outcome: ploke_records::selection::Outcome) -> &'static str {
    match outcome {
        ploke_records::selection::Outcome::Accepted => "accepted",
        ploke_records::selection::Outcome::ExploreFrom => "explore_from",
        ploke_records::selection::Outcome::Stop => "stop",
    }
}

fn format_f64(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.6}")
    } else {
        value.to_string()
    }
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn render_imp_at_k_counts(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    metric: Option<&ploke_tree::graph::MetricCandidateNode>,
) {
    let Some(imp) = metric.and_then(|metric| metric.imp_at_k.as_ref()) else {
        cached_label(ui, render_cache, "not_recorded");
        return;
    };
    let mut scored = itoa::Buffer::new();
    let mut descendants = itoa::Buffer::new();
    ui.horizontal(|ui| {
        cached_monospace_label(ui, render_cache, scored.format(imp.scored_descendant_count));
        cached_label(ui, render_cache, "/");
        cached_monospace_label(ui, render_cache, descendants.format(imp.descendant_count));
    });
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_tool_failures_delta(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<i64> {
    metric.compared_runs.iter().fold(None, |acc, run| {
        sum_delta(
            acc,
            run.baseline_metrics.as_ref()?.tool_calls_failed,
            run.treatment_metrics.as_ref()?.tool_calls_failed,
        )
    })
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_patch_failures_delta(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<i64> {
    metric.compared_runs.iter().fold(None, |acc, run| {
        sum_delta(
            acc,
            run.baseline_metrics.as_ref()?.partial_patch_failures,
            run.treatment_metrics.as_ref()?.partial_patch_failures,
        )
    })
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_treatment_valid_patch(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<bool> {
    metric
        .compared_runs
        .iter()
        .filter_map(|run| {
            run.treatment_metrics
                .as_ref()
                .map(|metrics| metrics.nonempty_valid_patch)
        })
        .reduce(|left, right| left && right)
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_treatment_converged(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<bool> {
    metric
        .compared_runs
        .iter()
        .filter_map(|run| {
            run.treatment_metrics
                .as_ref()
                .map(|metrics| metrics.convergence)
        })
        .reduce(|left, right| left && right)
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_treatment_oracle_eligible(
    metric: &ploke_tree::graph::MetricCandidateNode,
) -> Option<bool> {
    metric
        .compared_runs
        .iter()
        .filter_map(|run| {
            run.treatment_metrics
                .as_ref()
                .map(|metrics| metrics.oracle_eligible)
        })
        .reduce(|left, right| left && right)
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_protocol_reviewed_delta(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<i64> {
    metric.compared_runs.iter().fold(None, |acc, run| {
        sum_delta(
            acc,
            run.baseline_protocol.as_ref()?.reviewed_call_count as u64,
            run.treatment_protocol.as_ref()?.reviewed_call_count as u64,
        )
    })
}

/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
fn metric_protocol_missing_delta(metric: &ploke_tree::graph::MetricCandidateNode) -> Option<i64> {
    metric.compared_runs.iter().fold(None, |acc, run| {
        sum_delta(
            acc,
            run.baseline_protocol.as_ref()?.missing_call_count as u64,
            run.treatment_protocol.as_ref()?.missing_call_count as u64,
        )
    })
}

fn sum_delta(acc: Option<i64>, baseline: u64, treatment: u64) -> Option<i64> {
    Some(
        acc.unwrap_or_default()
            .saturating_add((treatment as i64).saturating_sub(baseline as i64)),
    )
}

fn render_optional_i64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<i64>,
) {
    if let Some(value) = value {
        let mut buffer = itoa::Buffer::new();
        cached_monospace_label(ui, render_cache, buffer.format(value));
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_optional_usize_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<usize>,
) {
    if let Some(value) = value {
        let mut buffer = itoa::Buffer::new();
        cached_monospace_label(ui, render_cache, buffer.format(value));
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_optional_f64_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<f64>,
) {
    if let Some(value) = value {
        cached_monospace_label(ui, render_cache, format_f64(value).as_str());
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_optional_f64_range(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<(f64, f64)>,
) {
    if let Some((lower, upper)) = value {
        cached_monospace_label(
            ui,
            render_cache,
            format!("{}..{}", format_f64(lower), format_f64(upper)).as_str(),
        );
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_optional_str_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<&str>,
) {
    if let Some(value) = value {
        cached_monospace_label(ui, render_cache, value);
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_optional_bool(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    value: Option<bool>,
) {
    if let Some(value) = value {
        cached_label(ui, render_cache, bool_label(value));
    } else {
        cached_label(ui, render_cache, "not_recorded");
    }
}

fn render_unavailable(ui: &mut egui::Ui, reason: UnavailableReason) {
    kv(ui, reason.subject(), reason.state());
}

#[cfg(all(test, feature = "native-benchmark"))]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tracing::field::{Field, Visit};
    use tracing::{Event, Id, Subscriber};
    use tracing_subscriber::layer::{Context, SubscriberExt};
    use tracing_subscriber::registry::LookupSpan;
    use tracing_subscriber::{Layer, Registry};

    use crate::benchmark::{STANDARD_RUN_ROOT, StartupProfile, load_graph_with_startup_profile};
    use crate::ui::diff::PatchDiffCache;
    use crate::ui::inspector::{
        GraphRevision, InspectorCache, SelectionInspector, default_selections,
    };
    use crate::ui::view::GraphSelectionRef;
    use ploke_records::history::{ArtifactRefRecord, SubjectRefRecord, TreeKeyHashRecord};
    use ploke_records::ids::{ArtifactId, EntryId, HistoryHash};
    use ploke_tree::graph::{
        ArtifactIdentity, ArtifactIds, ArtifactIndex, ArtifactKey, ArtifactNode, CandidateIndex,
        CandidateNode, CandidateSource,
    };
    use ploke_tree::{
        AuthorityLabel, Diagnostic, EvidenceRef, Lanes, NodeKey, NodeKind, PassiveEvidence, Phase,
        Progress, ResultClass, RunForest, Terminality, TreeNode,
    };

    #[derive(Clone, Default)]
    struct TraceLines(Arc<Mutex<Vec<String>>>);

    impl TraceLines {
        fn push(&self, line: String) {
            self.0.lock().expect("trace lock").push(line);
        }

        fn snapshot(&self) -> Vec<String> {
            self.0.lock().expect("trace lock").clone()
        }
    }

    #[derive(Default)]
    struct TraceFields {
        values: Vec<String>,
    }

    impl TraceFields {
        fn push(&mut self, field: &Field, value: impl Into<String>) {
            self.values
                .push(format!("{}={}", field.name(), value.into()));
        }

        fn finish(self) -> String {
            self.values.join(" ")
        }
    }

    impl Visit for TraceFields {
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.push(field, value.to_string());
        }

        fn record_str(&mut self, field: &Field, value: &str) {
            self.push(field, value.to_string());
        }

        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.push(field, format!("{value:?}"));
        }
    }

    struct TraceLayer {
        lines: TraceLines,
    }

    impl<S> Layer<S> for TraceLayer
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            _id: &Id,
            _ctx: Context<'_, S>,
        ) {
            let mut fields = TraceFields::default();
            attrs.record(&mut fields);
            self.lines.push(format!(
                "span:{} {}",
                attrs.metadata().name(),
                fields.finish()
            ));
        }

        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            let mut fields = TraceFields::default();
            event.record(&mut fields);
            self.lines.push(format!(
                "event:{} {}",
                event.metadata().target(),
                fields.finish()
            ));
        }
    }

    fn collect_traces<T>(f: impl FnOnce() -> T) -> (T, Vec<String>) {
        let lines = TraceLines::default();
        let subscriber = Registry::default().with(TraceLayer {
            lines: lines.clone(),
        });
        let output = tracing::subscriber::with_default(subscriber, f);
        (output, lines.snapshot())
    }

    fn test_run_forest_node(key: &str, source_artifact: &str) -> TreeNode {
        TreeNode {
            key: NodeKey::from(key),
            kind: NodeKind::SchedulerSearchNode,
            authority: AuthorityLabel::MutableProjection,
            parent: None,
            children: Vec::new(),
            generation: 0,
            branch_id: "branch".to_owned(),
            parent_branch_id: None,
            candidate_id: key.to_owned(),
            instance_id: "instance".to_owned(),
            source_state_id: source_artifact.to_owned(),
            target_relpath: ".ploke/prototype1/parent_identity.json".to_owned(),
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            progress: Progress {
                phase: Phase::Running,
                terminality: Terminality::NonTerminal,
                result_class: ResultClass::Unknown,
            },
            created_at: "2026-05-15T00:00:00Z".to_owned(),
            updated_at: "2026-05-15T00:00:00Z".to_owned(),
            evidence: vec![EvidenceRef {
                kind: ploke_tree::EvidenceKind::SchedulerNode,
                authority: AuthorityLabel::MutableProjection,
                node_key: Some(NodeKey::from(key)),
                runtime_id: None,
                recorded_at: Some("2026-05-15T00:00:00Z".to_owned()),
                detail: None,
            }],
            diagnostics: Vec::<Diagnostic>::new(),
        }
    }

    #[test]
    fn artifact_id_section_traces_expected_compact_rows() {
        let history_ref = ArtifactRefRecord::from_artifact_id(ArtifactId(
            "artifact:git-commit:deadbeefcafebabe".to_owned(),
        ));
        let artifact_id =
            ArtifactId("text-file-sha256:f6f73d0a2259c38d377144ed14f53be3".to_owned());
        let tree_key = TreeKeyHashRecord {
            hash: HistoryHash("tree:abcdef0123456789fedcba".to_owned()),
        };
        let node = ArtifactNode {
            key: ArtifactKey::HistoryRef {
                id: history_ref.id().0.clone(),
            },
            identity: ArtifactIdentity::HistoryRef(history_ref.clone()),
            ids: ArtifactIds {
                artifact_ids: vec![artifact_id.clone()],
                artifact_refs: vec![history_ref.clone()],
                tree_keys: vec![tree_key.clone()],
            },
            evidence: Vec::new(),
        };
        let candidate_artifact_after = ArtifactId(history_ref.graph_entity_key().to_owned());
        let graph = Graph {
            artifacts: ArtifactIndex {
                artifacts: BTreeMap::from([(node.key.clone(), node)]),
            },
            candidates: CandidateIndex {
                candidates: vec![CandidateNode {
                    selection_entry_id: EntryId("entry-artifact".to_owned()),
                    payload_index: 0,
                    subject: SubjectRefRecord {
                        value: "candidate:artifact".to_owned(),
                    },
                    source: Some(CandidateSource::CurrentGeneration),
                    occurrence_id: None,
                    membership_id: None,
                    membership_key: None,
                    node_id: Some("node-artifact".to_owned()),
                    branch_id: Some("branch-artifact".to_owned()),
                    generation: Some(0),
                    primary_runtime_id: None,
                    artifact_after: Some(candidate_artifact_after),
                    patch_id: None,
                    evidence: Vec::new(),
                }],
                ..Default::default()
            },
            ..Default::default()
        };
        let selection_key = graph
            .artifact_tree()
            .nodes
            .values()
            .next()
            .expect("artifact tree should contain the test node")
            .key
            .as_str()
            .to_owned();
        let selection = GraphSelectionRef::Artifact { key: selection_key };
        let mut cache = InspectorCache::default();
        let sections = cache
            .sections(&graph, GraphRevision::default(), Some(&selection))
            .expect("artifact selection cached");
        let mut render_cache = InspectorRenderCache::default();
        let mut diff_cache = PatchDiffCache::default();

        let (_, traces) = collect_traces(|| {
            egui::__run_test_ui(|ui| {
                render_right_inspector(
                    ui,
                    &graph,
                    Some(&selection),
                    Some("artifact"),
                    Some("A1"),
                    Some(sections),
                    &mut render_cache,
                    &mut diff_cache,
                    InspectorOpenState::default(),
                    None,
                );
                render_artifact_ids_for_inspector(ui, &graph, sections, &mut render_cache);
            });
        });

        assert!(traces.iter().any(|line| {
            line.contains("span:ploke_egui.inspector.artifact_ids_section")
                && line.contains("selection_kind=artifact")
                && line.contains("state=rendered")
        }));
        assert!(traces.iter().any(|line| line.contains(
            "span:ploke_egui.inspector.render_artifact_ids primary_artifact_id=text-file-sha256:f6f73d0a2259c38d377144ed14f53be3"
        )));
        assert!(traces.iter().any(|line| {
            line.contains(
                "span:ploke_egui.inspector.render_artifact_id_row slot=artifact id label=text-file-sha256"
            ) && line.contains("compact=f6f73d0a")
                && line.contains("expandable=true")
        }));
        assert!(traces.iter().any(|line| line.contains(
            "span:ploke_egui.inspector.render_artifact_id_row slot=artifact ref label=artifact:git-commit"
        ) && line.contains("compact=deadbeef")
            && line.contains("expandable=true")));
        assert!(traces.iter().any(|line| {
            line.contains(
                "span:ploke_egui.inspector.render_artifact_id_row slot=tree key label=tree",
            ) && line.contains("compact=abcdef01")
                && line.contains("expandable=true")
        }));
        assert!(traces.iter().any(|line| line.contains(
            "span:ploke_egui.id_display.show_compact full=text-file-sha256:f6f73d0a2259c38d377144ed14f53be3 compact=f6f73d0a expandable=true"
        )));
    }

    #[test]
    fn benchmark_inspector_open_state_forces_target_section() {
        let state = InspectorOpenState::benchmark(
            Some(crate::benchmark::BenchmarkInspectorSection::GraphEdges),
            false,
        );

        assert_eq!(state.open(InspectorPanelSection::GraphEdges), Some(true));
        assert_eq!(state.open(InspectorPanelSection::LlmCalls), None);
        assert_eq!(state.open(InspectorPanelSection::RunRecords), None);
        assert_eq!(state.open(InspectorPanelSection::PatchDebug), None);
    }

    #[test]
    fn benchmark_inspector_open_state_can_force_only_target_section() {
        let state = InspectorOpenState::benchmark(
            Some(crate::benchmark::BenchmarkInspectorSection::GraphEdges),
            true,
        );

        assert_eq!(state.open(InspectorPanelSection::GraphEdges), Some(true));
        assert_eq!(state.open(InspectorPanelSection::LlmCalls), Some(false));
        assert_eq!(state.open(InspectorPanelSection::RunRecords), Some(false));
        assert_eq!(state.open(InspectorPanelSection::PatchDebug), Some(false));
    }

    #[test]
    fn patch_debug_diff_scroll_areas_have_unique_ids() {
        let run_root = Path::new(STANDARD_RUN_ROOT);
        assert!(
            run_root.join("scheduler.json").is_file(),
            "standard benchmark run root missing: {}",
            run_root.display()
        );
        let (graph, startup) = load_graph_with_startup_profile(run_root, StartupProfile::default())
            .expect("load standard benchmark graph from typed records");
        assert!(
            !startup.compressed_run_records.is_empty(),
            "standard benchmark should deserialize compressed run records"
        );

        let selection = default_selections(&graph)
            .into_iter()
            .find(|selection| {
                matches!(
                    SelectionInspector::from_graph(&graph, selection),
                    SelectionInspector::Artifact(artifact) if artifact.patches.len() >= 2
                )
            })
            .expect("standard benchmark graph should contain an artifact with multiple patches");
        let mut inspector_cache = InspectorCache::default();
        let sections = inspector_cache
            .sections(&graph, GraphRevision::default(), Some(&selection.reference))
            .expect("real benchmark artifact selection has inspector sections");
        assert!(
            sections.patches().len() >= 2,
            "real benchmark selection should render multiple patch diffs"
        );
        let mut render_cache = InspectorRenderCache::default();
        let mut diff_cache = PatchDiffCache::default();
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::empty());

        let output = ctx.run_ui(Default::default(), |ui| {
            render_patches_for_inspector(ui, &graph, sections, &mut render_cache, &mut diff_cache);
        });

        let warning_texts: Vec<_> = clipped_shape_texts(&output.shapes)
            .into_iter()
            .filter(|text| text.contains("ScrollArea ID"))
            .collect();
        assert!(
            warning_texts.is_empty(),
            "unexpected egui ScrollArea ID clash warnings: {warning_texts:?}"
        );
    }

    #[test]
    fn artifact_id_section_traces_not_applicable_for_run_forest_selection() {
        let node = test_run_forest_node("node-f1fbab3a2bb5e7e5", "artifact:source");
        let selection = GraphSelectionRef::RunForestNode {
            key: node.key.as_str().to_owned(),
        };
        let graph = Graph {
            forest: Some(RunForest {
                campaign: ploke_tree::CampaignRef {
                    campaign_id: "campaign".to_owned(),
                    updated_at: "now".to_owned(),
                },
                roots: vec![node.key.clone()],
                nodes: vec![node],
                lanes: Lanes {
                    frontier: Vec::new(),
                    completed: Vec::new(),
                    failed: Vec::new(),
                },
                passive_evidence: PassiveEvidence::default(),
                diagnostics: Vec::new(),
            }),
            ..Default::default()
        };
        let mut cache = InspectorCache::default();
        let sections = cache
            .sections(&graph, GraphRevision::default(), Some(&selection))
            .expect("run forest selection cached");

        let (_, traces) = collect_traces(|| {
            egui::__run_test_ui(|ui| {
                let mut render_cache = InspectorRenderCache::default();
                render_artifact_ids_for_inspector(ui, &graph, sections, &mut render_cache);
            });
        });

        assert!(traces.iter().any(|line| line.contains(
            "span:ploke_egui.inspector.artifact_ids_section selection_kind=run_forest_node state=not_applicable selection_key=node-f1fbab3a2bb5e7e5"
        )));
    }

    fn clipped_shape_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        let mut texts = Vec::new();
        for shape in shapes {
            collect_shape_texts(&shape.shape, &mut texts);
        }
        texts
    }

    fn collect_shape_texts(shape: &egui::epaint::Shape, texts: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(text) => texts.push(text.galley.text().to_owned()),
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_shape_texts(shape, texts);
                }
            }
            _ => {}
        }
    }
}

fn render_cached_code_block(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_salt: impl std::hash::Hash,
    text: &str,
) -> egui::Response {
    let galley = render_cache.text_galley(ui, text, CachedTextKind::MonospaceBlock);
    render_diff_galley(ui, id_salt, galley)
}

fn render_agent_turns<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turns: impl IntoIterator<Item = &'a AgentTurnArtifactMetadata>,
) {
    let mut rendered = false;
    for (index, turn) in turns.into_iter().enumerate() {
        rendered = true;
        show_inspector_collapsing(
            ui,
            egui::CollapsingHeader::new(format!("turn {}", index + 1)).default_open(index == 0),
            |ui| {
                let _span = tracing::trace_span!(scope::INSPECTOR_AGENT_TURN).entered();
                cached_kv_id(ui, render_cache, "model", turn.selected_model.as_str());
                if let Some(outcome) = turn.terminal_outcome.as_deref() {
                    cached_kv_id(ui, render_cache, "outcome", outcome);
                }
                cached_kv_usize(ui, render_cache, "events", turn.event_count);
                cached_kv_usize(
                    ui,
                    render_cache,
                    "prompt messages",
                    turn.llm_prompt_message_count,
                );
                render_agent_turn_tools(ui, render_cache, turn);
                cached_kv_id(
                    ui,
                    render_cache,
                    "patch",
                    if turn.patch_applied {
                        "applied"
                    } else {
                        "not_applied"
                    },
                );
                cached_expandable_id(
                    ui,
                    render_cache,
                    ("agent-turn", turn.task_id.as_str()),
                    turn.task_id.as_str(),
                );
            },
        );
    }
    if !rendered {
        kv(ui, "llm", "not_recorded");
    }
}

fn render_agent_turn_tools(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turn: &AgentTurnArtifactMetadata,
) {
    let mut requested = itoa::Buffer::new();
    let mut completed = itoa::Buffer::new();
    let mut failed = itoa::Buffer::new();
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "tools");
        cached_monospace_label(
            ui,
            render_cache,
            requested.format(turn.tool_request_event_count),
        );
        cached_monospace_label(ui, render_cache, "requested,");
        cached_monospace_label(
            ui,
            render_cache,
            completed.format(turn.tool_completed_event_count),
        );
        cached_monospace_label(ui, render_cache, "completed,");
        cached_monospace_label(
            ui,
            render_cache,
            failed.format(turn.tool_failed_event_count),
        );
        cached_monospace_label(ui, render_cache, "failed");
    });
}

fn render_edges<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    direction: &str,
    edges: impl IntoIterator<Item = SelectionEdge<'a>>,
) {
    let mut rendered = false;
    for edge in edges {
        rendered = true;
        ui.horizontal(|ui| {
            cached_label(ui, render_cache, direction);
            cached_monospace_label(ui, render_cache, edge.relation.label());
            cached_expandable_id(
                ui,
                render_cache,
                ("edge-from", direction, edge.relation.label(), edge.from),
                edge.from,
            );
            cached_label(ui, render_cache, "->");
            cached_expandable_id(
                ui,
                render_cache,
                ("edge-to", direction, edge.relation.label(), edge.to),
                edge.to,
            );
            render_count_parens(ui, render_cache, edge.source_count);
        });
    }
    if !rendered {
        kv(ui, direction, "none");
    }
}

fn render_patches<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    patches: impl IntoIterator<Item = PatchInspection<'a>>,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) {
    let mut rendered = false;
    for patch in patches {
        rendered = true;
        render_patch(ui, render_cache, patch, diff_cache);
    }
    if !rendered {
        kv(ui, "patch", "not_available");
    }
}

fn render_patch(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    patch: PatchInspection<'_>,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) {
    let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_PATCH).entered();
    {
        let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_HEADER).entered();
        ui.horizontal(|ui| {
            cached_label(ui, render_cache, "patch");
            cached_expandable_id(
                ui,
                render_cache,
                ("patch", patch.patch_id()),
                patch.patch_id(),
            );
        });
        cached_label(ui, render_cache, "diff");
    }
    render_diff(ui, patch, diff_cache);

    {
        let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DETAILS_HEADER).entered();
        egui::CollapsingHeader::new("Details")
            .id_salt(("patch-details", patch.patch_id()))
            .default_open(false)
            .show(ui, |ui| {
                let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DETAILS).entered();
                render_patch_details(ui, render_cache, patch);
            });
    }
}

fn render_patch_details(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    patch: PatchInspection<'_>,
) {
    cached_kv_path(ui, render_cache, "target", patch.target_relpath());
    cached_kv_id(ui, render_cache, "branch", patch.branch_id());
    cached_kv_id(ui, render_cache, "candidate", patch.candidate_id());
    cached_kv_id(ui, render_cache, "source hash", patch.source_content_hash());
    cached_kv_id(
        ui,
        render_cache,
        "proposed hash",
        patch.proposed_content_hash(),
    );
    if let Some(base) = patch.base_artifact() {
        cached_kv_id(ui, render_cache, "base artifact", base);
    }
    if let Some(derived) = patch.derived_artifact() {
        cached_kv_id(ui, render_cache, "derived artifact", derived);
    }
    if let Some(check) = patch.check_status() {
        cached_kv_id(ui, render_cache, "check", surface_check_status_label(check));
    }
    if let Some(apply) = patch.apply_status() {
        cached_kv_id(ui, render_cache, "apply", surface_apply_status_label(apply));
    }

    let mut touched = false;
    for touch in patch.touches() {
        let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_TOUCH).entered();
        touched = true;
        render_patch_touch_label(ui, render_cache, touch);
        cached_wrapped_monospace_label(ui, render_cache, touch.replacement);
    }
    if !touched {
        kv(ui, "touches", "none");
    }
}

fn render_count_parens(ui: &mut egui::Ui, render_cache: &mut InspectorRenderCache, count: usize) {
    let mut buffer = itoa::Buffer::new();
    ui.horizontal(|ui| {
        cached_monospace_label(ui, render_cache, "(");
        cached_monospace_label(ui, render_cache, buffer.format(count));
        cached_monospace_label(ui, render_cache, ")");
    });
}

fn render_patch_touch_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    touch: crate::ui::inspector::PatchTouch<'_>,
) {
    let mut index = itoa::Buffer::new();
    let mut start = itoa::Buffer::new();
    let mut end = itoa::Buffer::new();
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "touch");
        cached_monospace_label(ui, render_cache, index.format(touch.index));
        cached_monospace_label(ui, render_cache, touch.relpath);
        cached_monospace_label(ui, render_cache, start.format(touch.start));
        cached_label(ui, render_cache, "-");
        cached_monospace_label(ui, render_cache, end.format(touch.end));
    });
}

fn render_diff(
    ui: &mut egui::Ui,
    patch: PatchInspection<'_>,
    diff_cache: &mut crate::ui::diff::PatchDiffCache,
) -> egui::Response {
    let galley = {
        let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DIFF_CACHE).entered();
        diff_cache.highlighted_patch_galley(ui, patch)
    };
    render_diff_galley(
        ui,
        (
            "ploke_egui.patch_debug.diff",
            patch.child.node.node_id.as_str(),
            patch.patch_id(),
        ),
        galley,
    )
}

fn render_diff_galley(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    galley: Arc<egui::Galley>,
) -> egui::Response {
    let _span = tracing::trace_span!(scope::INSPECTOR_PATCH_DEBUG_DIFF_WIDGET).entered();
    let width = ui.available_width().max(240.0);
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt(id_salt)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_min_width(width);
                    ui.add(egui::Label::new(galley).selectable(true));
                });
        })
        .response
}
