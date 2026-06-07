use crate::allocation::scope;
use crate::ui::id_display::{CopyableId, CopyablePath, ShortId};
use crate::ui::inspector::{
    InspectorSections, RunRecordInspection, RunRecordSlot, RunRecordTurnInspection,
    response_finish_reason_label, tool_execution_name, tool_execution_status_label,
    tool_execution_summary, turn_outcome_elapsed_secs, turn_outcome_error, turn_outcome_label,
    turn_outcome_tool_count,
};
use crate::ui::render::text::*;
use eframe::egui;
use ploke_tree::Graph;

use ploke_records::tool_contracts::{
    PersistedToolCallArguments, PersistedToolResultContent, ToolCallArguments, ToolResultContent,
};

use super::context_strip::render_run_evidence_context_strip;
use super::fields::*;
use super::run_dashboard::render_tool_step_outcome_histogram;
use super::{
    InspectorRenderCache, render_cached_code_block, render_unavailable, show_inspector_collapsing,
};

fn tool_steps_id_salt(
    render_cache: &InspectorRenderCache,
    part: impl std::hash::Hash,
) -> (u32, impl std::hash::Hash) {
    (render_cache.active_tool_steps_disclosure_generation(), part)
}

fn show_tool_steps_collapsing<R>(
    ui: &mut egui::Ui,
    header: egui::CollapsingHeader,
    default_open: bool,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) {
    show_inspector_collapsing(ui, header.default_open(default_open), add_body);
}
use crate::ui::provenance::{
    EvidenceLane, RunRecordProvenanceCtx, detail_for_lane_at_step, detail_for_tool_failure,
    render_evidence_lane_chip_with_inspect, render_provenance_inspect_button,
    tool_failure_headline,
};

/// archaeology:run-record-branch-output
/// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_run_records")
)]
pub(crate) fn render_run_records_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    render_run_records(
        ui,
        render_cache,
        sections.run_records().iter().filter_map(|slot| {
            let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_RESOLVE_SLOT).entered();
            slot.resolve(graph)
        }),
    );
}

fn render_run_records<'a>(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    records: impl IntoIterator<Item = RunRecordInspection<'a>>,
) {
    render_run_evidence_context_strip(
        ui,
        render_cache.run_evidence_source_label(),
        render_cache.run_evidence_catalog_error(),
    );
    let mut rendered = false;
    for record in records {
        let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ROW).entered();
        rendered = true;
        ui.separator();
        run_record_kv_id(
            ui,
            render_cache,
            "arm",
            compared_run_arm_label(record.record_ref.arm),
        );
        run_record_kv_id(
            ui,
            render_cache,
            "instance",
            record.record_ref.instance_id.as_str(),
        );
        run_record_kv_path(
            ui,
            render_cache,
            "record",
            record
                .record_ref
                .record_path
                .to_str()
                .unwrap_or("non_utf8_path"),
        );
        run_record_kv_id(
            ui,
            render_cache,
            "manifest",
            record.record.manifest_id.as_str(),
        );
        if let Some(model) = record.record.metadata.agent.model_id.as_deref() {
            run_record_kv_id(ui, render_cache, "model", model);
        }
        if let Some(provider) = record.record.metadata.agent.provider.as_deref() {
            run_record_kv_id(ui, render_cache, "provider", provider);
        }
        run_record_kv_path(
            ui,
            render_cache,
            "repo root",
            record
                .record
                .metadata
                .benchmark
                .repo_root
                .to_str()
                .unwrap_or("non_utf8_path"),
        );
        run_record_kv_usize(ui, render_cache, "turns", record.stats.turn_count);
        run_record_kv_usize(ui, render_cache, "tool calls", record.stats.tool_call_count);
        run_record_kv_usize(
            ui,
            render_cache,
            "failed tool calls",
            record.stats.failed_tool_call_count,
        );
        if let Some(packaging) = record.record.phases.packaging.as_ref() {
            run_record_kv_id(
                ui,
                render_cache,
                "submission",
                submission_artifact_state_label(packaging.submission_artifact_state),
            );
            run_record_kv_id(
                ui,
                render_cache,
                "patch projection",
                patch_projection_check_state_label(packaging.patch_projection_check_state),
            );
        }
    }
    if !rendered {
        kv(ui, "run records", "none");
    }
}

fn run_record_kv_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_WIDGET_ROW).entered();
    ui.horizontal(|ui| {
        run_record_label(ui, render_cache, key);
        run_record_expandable_id(ui, render_cache, ("kv", key, value), value);
    });
}

fn run_record_kv_path(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_WIDGET_ROW).entered();
    ui.horizontal(|ui| {
        run_record_label(ui, render_cache, key);
        run_record_path_value(ui, render_cache, ("path", key, value), value);
    });
}

fn run_record_kv_usize(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: usize,
) {
    let mut buffer = itoa::Buffer::new();
    run_record_kv_id(ui, render_cache, key, buffer.format(value));
}

fn run_record_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = {
        let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_TEXT_GALLEY).entered();
        render_cache.run_record_text_galley(ui, text, CachedTextKind::Plain)
    };
    let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_LABEL_WIDGET).entered();
    add_cached_theme_galley(ui, galley, egui::Sense::hover())
}

fn run_record_expandable_id(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let Some(_) = ShortId::new(full) else {
        return run_record_monospace_label(ui, render_cache, full);
    };

    let value = CopyableId::new(full);
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        true,
        |cache, ui, expanded| {
            let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ID_GALLEY).entered();
            cache.run_record_id_galley(ui, full, expanded)
        },
    )
}

fn run_record_path_value(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_source: impl std::hash::Hash,
    full: &str,
) -> egui::Response {
    let value = CopyablePath::new(full);
    let expandable = value.is_expandable();
    cached_copyable_value(
        ui,
        render_cache,
        id_source,
        &value,
        expandable,
        |cache, ui, expanded| {
            let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_ID_GALLEY).entered();
            let label = if expanded { full } else { value.tail() };
            cache.run_record_text_galley(ui, label, CachedTextKind::Monospace)
        },
    )
}

fn run_record_monospace_label(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    text: &str,
) -> egui::Response {
    let galley = {
        let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_TEXT_GALLEY).entered();
        render_cache.run_record_text_galley(ui, text, CachedTextKind::Monospace)
    };
    let _span = tracing::trace_span!(scope::INSPECTOR_RUN_RECORDS_LABEL_WIDGET).entered();
    add_cached_theme_galley(ui, galley, egui::Sense::hover())
}

fn compared_run_arm_label(arm: ploke_tree::ComparedRunArm) -> &'static str {
    match arm {
        ploke_tree::ComparedRunArm::Baseline => "baseline",
        ploke_tree::ComparedRunArm::Treatment => "treatment",
    }
}

pub(super) fn submission_artifact_state_label(
    state: ploke_records::run_record::SubmissionArtifactState,
) -> &'static str {
    match state {
        ploke_records::run_record::SubmissionArtifactState::NotRecorded => "not_recorded",
        ploke_records::run_record::SubmissionArtifactState::NotApplicable => "not_applicable",
        ploke_records::run_record::SubmissionArtifactState::Missing => "missing",
        ploke_records::run_record::SubmissionArtifactState::Empty => "empty",
        ploke_records::run_record::SubmissionArtifactState::Nonempty => "nonempty",
    }
}

pub(super) fn patch_projection_check_state_label(
    state: ploke_records::evaluation::PatchProjectionCheckState,
) -> &'static str {
    match state {
        ploke_records::evaluation::PatchProjectionCheckState::NotRecorded => "not_recorded",
        ploke_records::evaluation::PatchProjectionCheckState::NotApplicable => "not_applicable",
        ploke_records::evaluation::PatchProjectionCheckState::Passed => "passed",
        ploke_records::evaluation::PatchProjectionCheckState::Failed => "failed",
        ploke_records::evaluation::PatchProjectionCheckState::NotRun => "not_run",
    }
}

/// archaeology:run-record-branch-output
/// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
pub(super) fn render_run_record_turns(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    graph: &Graph,
    run_record_slots: &[RunRecordSlot],
) -> bool {
    let treatment = render_run_record_arm_turns(
        ui,
        render_cache,
        graph,
        run_record_slots,
        ploke_tree::ComparedRunArm::Treatment,
        true,
    );
    let baseline = render_run_record_arm_turns(
        ui,
        render_cache,
        graph,
        run_record_slots,
        ploke_tree::ComparedRunArm::Baseline,
        false,
    );
    treatment || baseline
}

fn has_run_record_arm_turns(
    graph: &Graph,
    run_record_slots: &[RunRecordSlot],
    arm: ploke_tree::ComparedRunArm,
) -> bool {
    run_record_slots
        .iter()
        .filter_map(|slot| slot.resolve(graph))
        .any(|record| record.record_ref.arm == arm && record.record.turn_count() > 0)
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_run_record_arm")
)]
fn render_run_record_arm_turns(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    graph: &Graph,
    run_record_slots: &[RunRecordSlot],
    arm: ploke_tree::ComparedRunArm,
    default_open: bool,
) -> bool {
    if !has_run_record_arm_turns(graph, run_record_slots, arm) {
        return false;
    }

    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new(compared_run_arm_label(arm)).default_open(default_open),
        |ui| {
            cached_kv_id(ui, render_cache, "evidence", "branch_run_record");
            for record in run_record_slots
                .iter()
                .filter_map(|slot| slot.resolve(graph))
                .filter(|record| record.record_ref.arm == arm)
            {
                for turn in record.turns() {
                    render_run_record_turn(ui, render_cache, turn);
                }
            }
        },
    );
    true
}

fn render_run_record_turn(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turn: RunRecordTurnInspection<'_>,
) {
    ui.separator();
    render_run_record_turn_summary_card(ui, render_cache, turn);
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Turn details").default_open(false),
        |ui| {
            render_run_record_turn_details(ui, render_cache, turn);
        },
    );
    render_run_record_agent_turn_artifact(ui, render_cache, turn);
    let record_ctx = run_record_provenance_ctx(turn);
    render_run_record_tool_steps(
        ui,
        render_cache,
        Some(record_ctx),
        turn.turn_index,
        turn.turn.tool_calls.as_slice(),
    );
}

fn run_record_provenance_ctx(turn: RunRecordTurnInspection<'_>) -> RunRecordProvenanceCtx<'_> {
    RunRecordProvenanceCtx {
        manifest_id: turn.record.manifest_id.as_str(),
        record_path: turn
            .record_ref
            .record_path
            .to_str()
            .unwrap_or("non_utf8_path"),
    }
}

fn render_run_record_turn_summary_card(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turn: RunRecordTurnInspection<'_>,
) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                cached_label(ui, render_cache, "turn");
                let mut turn_number = itoa::Buffer::new();
                cached_monospace_label(ui, render_cache, turn_number.format(turn.turn.turn_number));
                cached_label(ui, render_cache, "outcome");
                cached_monospace_label(ui, render_cache, turn_outcome_label(&turn.turn.outcome));
                cached_label(ui, render_cache, "tool steps");
                let mut tool_steps = itoa::Buffer::new();
                cached_monospace_label(
                    ui,
                    render_cache,
                    tool_steps.format(turn.turn.tool_calls.len()),
                );
                if let Some(count) = turn_outcome_tool_count(&turn.turn.outcome) {
                    cached_label(ui, render_cache, "outcome tools");
                    let mut outcome_tools = itoa::Buffer::new();
                    cached_monospace_label(ui, render_cache, outcome_tools.format(count));
                }
            });
        });
}

fn render_run_record_turn_details(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turn: RunRecordTurnInspection<'_>,
) {
    cached_kv_id(
        ui,
        render_cache,
        "arm",
        compared_run_arm_label(turn.record_ref.arm),
    );
    cached_kv_id(
        ui,
        render_cache,
        "instance",
        turn.record_ref.instance_id.as_str(),
    );
    if let Some(model) = turn
        .turn
        .llm_request
        .as_ref()
        .map(|request| request.model.as_str())
        .or(turn.record.metadata.agent.model_id.as_deref())
    {
        cached_kv_id(ui, render_cache, "model", model);
    }
    if let Some(provider) = turn.record.metadata.agent.provider.as_deref() {
        cached_kv_id(ui, render_cache, "provider", provider);
    }
    if let Some(message) = turn_outcome_error(&turn.turn.outcome) {
        cached_kv_text(ui, render_cache, "outcome error", message);
    }
    if let Some(elapsed) = turn_outcome_elapsed_secs(&turn.turn.outcome) {
        cached_kv_u64(ui, render_cache, "elapsed secs", elapsed);
    }
    cached_kv_usize(
        ui,
        render_cache,
        "prompt messages",
        turn.turn
            .llm_request
            .as_ref()
            .map_or(0, |request| request.messages.len()),
    );
    if let Some(response) = turn.turn.llm_response.as_ref() {
        cached_kv_id(ui, render_cache, "response", "present");
        if let Some(reason) = response.finish_reason.as_ref() {
            cached_kv_id(
                ui,
                render_cache,
                "finish reason",
                response_finish_reason_label(reason),
            );
        }
        if let Some(usage) = response.usage {
            render_token_usage(ui, render_cache, usage);
        }
    } else {
        cached_kv_id(ui, render_cache, "response", "missing");
    }
}

pub(super) fn render_token_usage(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    usage: ploke_records::agent_turn::TokenUsageRecord,
) {
    ui.horizontal(|ui| {
        let mut prompt = itoa::Buffer::new();
        let mut completion = itoa::Buffer::new();
        let mut total = itoa::Buffer::new();
        cached_label(ui, render_cache, "usage");
        cached_monospace_label(ui, render_cache, "prompt=");
        cached_monospace_label(ui, render_cache, prompt.format(usage.prompt_tokens));
        cached_monospace_label(ui, render_cache, " completion=");
        cached_monospace_label(ui, render_cache, completion.format(usage.completion_tokens));
        cached_monospace_label(ui, render_cache, " total=");
        cached_monospace_label(ui, render_cache, total.format(usage.total_tokens));
    });
}

fn render_run_record_agent_turn_artifact(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    turn: RunRecordTurnInspection<'_>,
) {
    let Some(artifact) = turn.turn.agent_turn_artifact.as_ref() else {
        cached_kv_id(ui, render_cache, "agent-turn artifact", "not_recorded");
        return;
    };

    cached_kv_usize(ui, render_cache, "artifact events", artifact.events.len());
    if let Some(terminal) = artifact.terminal_record.as_ref() {
        cached_kv_id(
            ui,
            render_cache,
            "terminal outcome",
            terminal.outcome.as_str(),
        );
        cached_kv_u32(ui, render_cache, "terminal attempts", terminal.attempts);
        cached_kv_text(
            ui,
            render_cache,
            "terminal summary",
            terminal.summary.as_str(),
        );
    } else {
        cached_kv_id(ui, render_cache, "terminal", "not_recorded");
    }
    ui.horizontal(|ui| {
        let mut edits = itoa::Buffer::new();
        let mut creates = itoa::Buffer::new();
        let mut expected = itoa::Buffer::new();
        cached_label(ui, render_cache, "patch proposals");
        cached_monospace_label(ui, render_cache, "edits=");
        cached_monospace_label(
            ui,
            render_cache,
            edits.format(artifact.patch_artifact.edit_proposals.len()),
        );
        cached_monospace_label(ui, render_cache, " creates=");
        cached_monospace_label(
            ui,
            render_cache,
            creates.format(artifact.patch_artifact.create_proposals.len()),
        );
        cached_monospace_label(ui, render_cache, " expected_files=");
        cached_monospace_label(
            ui,
            render_cache,
            expected.format(artifact.patch_artifact.expected_file_changes.len()),
        );
    });
}

#[ploke_egui_macros::profile_scope(crate::allocation::scope::INSPECTOR_RUN_RECORD_TOOL_STEPS)]
pub(super) fn render_run_record_tool_steps(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    turn_index: usize,
    tools: &[ploke_records::run_record::ToolExecutionRecord],
) {
    if tools.is_empty() {
        cached_kv_id(ui, render_cache, "tool steps", "none");
        return;
    }

    let disclosure_scope = record_ctx
        .map(|ctx| InspectorRenderCache::tool_steps_disclosure_scope(ctx.record_path, turn_index));

    render_cache.with_active_tool_steps_disclosure(disclosure_scope.clone(), |render_cache| {
        let mut failed = Vec::new();
        let mut succeeded = Vec::new();
        for (index, tool) in tools.iter().enumerate() {
            if matches!(
                tool.result,
                ploke_records::run_record::ToolResult::Failed(_)
            ) {
                failed.push((index, tool));
            } else {
                succeeded.push((index, tool));
            }
        }

        ui.horizontal(|ui| {
            cached_label(ui, render_cache, "tool steps");
            if let Some(scope) = disclosure_scope {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("Expand all")
                        .on_hover_text("Expand every tool step and nested section in this turn")
                        .clicked()
                    {
                        render_cache.tool_steps_expand_all(scope.clone());
                    }
                    if ui
                        .small_button("Collapse all")
                        .on_hover_text("Collapse every tool step and nested section in this turn")
                        .clicked()
                    {
                        render_cache.tool_steps_collapse_all(scope.clone());
                    }
                });
            }
        });
        render_tool_step_outcome_histogram(ui, tools.len(), failed.len());

        for (index, tool) in failed {
            render_run_record_tool_step(ui, render_cache, record_ctx, index, tool, true);
        }

        if !succeeded.is_empty() {
            show_tool_steps_collapsing(
                ui,
                egui::CollapsingHeader::new("Successful tool steps").id_salt(tool_steps_id_salt(
                    render_cache,
                    "run-record-tool-steps-succeeded",
                )),
                render_cache.tool_steps_default_open(false),
                |ui| {
                    cached_kv_usize(ui, render_cache, "successful", succeeded.len());
                    for (index, tool) in succeeded {
                        render_run_record_tool_step(
                            ui,
                            render_cache,
                            record_ctx,
                            index,
                            tool,
                            false,
                        );
                    }
                },
            );
        }
    });
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_run_record_tool_step")
)]
fn render_run_record_tool_step(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
    default_open: bool,
) {
    let call_id = tool.request.call_id.as_str();
    let header_id = ui.make_persistent_id(tool_steps_id_salt(
        render_cache,
        ("run-record-tool-step", index, call_id),
    ));
    egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        header_id,
        render_cache.tool_steps_default_open(default_open),
    )
    .show_header(ui, |ui| {
        render_run_record_tool_step_header(ui, render_cache, record_ctx, index, tool);
    })
    .body(|ui| {
        render_run_record_tool_step_details(ui, render_cache, record_ctx, index, tool);
    });
}

fn render_run_record_tool_step_header(
    ui: &mut egui::Ui,
    _render_cache: &mut InspectorRenderCache,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    let call_id = tool.request.call_id.as_str();
    ui.horizontal(|ui| {
        let mut step = itoa::Buffer::new();
        fresh_monospace_label(ui, step.format(index + 1));
        fresh_monospace_label(ui, tool_execution_name(tool));
        render_tool_execution_status_badge(ui, tool);
        let detail = detail_for_tool_failure(tool, index, record_ctx);
        render_evidence_lane_chip_with_inspect(
            ui,
            EvidenceLane::lane_for_tool_result(tool),
            detail,
            ("run-record-tool-step-lane", index, call_id),
        );
    });
}

fn render_run_record_tool_step_details(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    let call_id = tool.request.call_id.as_str();
    let mut latency = itoa::Buffer::new();

    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "call id");
        cached_expandable_id(
            ui,
            render_cache,
            ("run-record-tool-call-id", index, call_id),
            call_id,
        );
    });
    ui.horizontal(|ui| {
        cached_label(ui, render_cache, "latency");
        cached_monospace_label(ui, render_cache, latency.format(tool.latency_ms));
        cached_monospace_label(ui, render_cache, "ms");
    });
    if matches!(
        tool.result,
        ploke_records::run_record::ToolResult::Failed(_)
    ) {
        ui.horizontal(|ui| {
            cached_label(ui, render_cache, "Harness error (recorded)");
            let detail = detail_for_tool_failure(tool, index, record_ctx);
            render_provenance_inspect_button(
                ui,
                ("run-record-tool-harness", index, call_id),
                detail,
            );
        });
        cached_wrapped_monospace_label(ui, render_cache, tool_failure_headline(tool));
    } else {
        cached_label(ui, render_cache, "summary");
        cached_wrapped_monospace_label(ui, render_cache, tool_execution_summary(tool));
    }
    render_tool_arguments_section(ui, render_cache, record_ctx, index, tool);
    if let Some(payload) = tool_execution_ui_payload(tool) {
        render_tool_ui_payload(ui, render_cache, record_ctx, index, tool, payload);
    }
    render_tool_result_section(ui, render_cache, record_ctx, index, tool);
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_arguments")
)]
fn render_tool_arguments_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    let call_id = tool.request.call_id.as_str();
    show_tool_steps_collapsing(
        ui,
        egui::CollapsingHeader::new("tool call arguments").id_salt(tool_steps_id_salt(
            render_cache,
            ("run-record-tool-arguments", index, call_id),
        )),
        render_cache.tool_steps_default_open(true),
        |ui| {
            let decoded = render_cache.tool_arguments(
                call_id,
                tool.request.tool.as_str(),
                tool.request.arguments.as_str(),
            );
            render_decoded_tool_arguments(
                ui,
                render_cache,
                decoded.as_ref(),
                record_ctx,
                Some((index, tool)),
            );

            show_tool_steps_collapsing(
                ui,
                egui::CollapsingHeader::new("raw arguments").id_salt(tool_steps_id_salt(
                    render_cache,
                    ("run-record-tool-raw-arguments", index, call_id),
                )),
                render_cache.tool_steps_default_open(false),
                |ui| {
                    render_tool_raw_arguments_section(ui, render_cache, index, call_id, tool);
                },
            );
        },
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_raw_arguments")
)]
fn render_tool_raw_arguments_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    index: usize,
    call_id: &str,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    render_cached_code_block(
        ui,
        render_cache,
        ("run-record-tool-arguments-block", index, call_id),
        tool.request.arguments.as_str(),
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_result")
)]
fn render_tool_result_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    let call_id = tool.request.call_id.as_str();
    let raw_content = tool_execution_content(tool);
    show_tool_steps_collapsing(
        ui,
        egui::CollapsingHeader::new("tool call content").id_salt(tool_steps_id_salt(
            render_cache,
            ("run-record-tool-content", index, call_id),
        )),
        render_cache.tool_steps_default_open(false),
        |ui| match &tool.result {
            ploke_records::run_record::ToolResult::Completed(_) => {
                let decoded =
                    render_cache.tool_result(call_id, tool_execution_name(tool), raw_content);
                render_decoded_tool_result(
                    ui,
                    render_cache,
                    decoded.as_ref(),
                    record_ctx,
                    Some((index, tool)),
                );

                show_tool_steps_collapsing(
                    ui,
                    egui::CollapsingHeader::new("raw content").id_salt(tool_steps_id_salt(
                        render_cache,
                        ("run-record-tool-content-raw", index, call_id),
                    )),
                    render_cache.tool_steps_default_open(false),
                    |ui| {
                        let detail = detail_for_lane_at_step(
                            EvidenceLane::RecordedRawPayload,
                            record_ctx,
                            Some(call_id),
                            Some(tool.request.tool.as_str()),
                            Some(index),
                        );
                        render_evidence_lane_chip_with_inspect(
                            ui,
                            EvidenceLane::RecordedRawPayload,
                            detail,
                            ("run-record-tool-content-raw-lane", index, call_id),
                        );
                        render_tool_raw_result_section(
                            ui,
                            render_cache,
                            ("run-record-tool-content-block", index, call_id),
                            raw_content,
                        );
                    },
                );
            }
            ploke_records::run_record::ToolResult::Failed(_) => {
                render_tool_failure_content(ui, render_cache, tool);
                show_tool_steps_collapsing(
                    ui,
                    egui::CollapsingHeader::new("raw error").id_salt(tool_steps_id_salt(
                        render_cache,
                        ("run-record-tool-error-raw", index, call_id),
                    )),
                    render_cache.tool_steps_default_open(false),
                    |ui| {
                        let detail = detail_for_lane_at_step(
                            EvidenceLane::RecordedRawPayload,
                            record_ctx,
                            Some(call_id),
                            Some(tool.request.tool.as_str()),
                            Some(index),
                        );
                        render_evidence_lane_chip_with_inspect(
                            ui,
                            EvidenceLane::RecordedRawPayload,
                            detail,
                            ("run-record-tool-error-raw-lane", index, call_id),
                        );
                        render_tool_raw_result_section(
                            ui,
                            render_cache,
                            ("run-record-tool-error-block", index, call_id),
                            raw_content,
                        );
                    },
                );
            }
        },
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_raw_result")
)]
fn render_tool_raw_result_section(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    id_salt: impl std::hash::Hash,
    raw_content: &str,
) {
    render_cached_code_block(ui, render_cache, id_salt, raw_content);
}

fn render_run_record_decode_lane_chip(
    ui: &mut egui::Ui,
    lane: EvidenceLane,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    tool_step: Option<(usize, &ploke_records::run_record::ToolExecutionRecord)>,
    id_kind: &'static str,
) {
    let Some((index, tool)) = tool_step else {
        use crate::ui::provenance::render_evidence_lane_chip;
        render_evidence_lane_chip(ui, lane);
        return;
    };
    let call_id = tool.request.call_id.as_str();
    if let Some(ctx) = record_ctx {
        let detail = detail_for_lane_at_step(
            lane,
            Some(ctx),
            Some(call_id),
            Some(tool.request.tool.as_str()),
            Some(index),
        );
        render_evidence_lane_chip_with_inspect(ui, lane, detail, (id_kind, index, call_id));
    } else {
        use crate::ui::provenance::render_evidence_lane_chip;
        render_evidence_lane_chip(ui, lane);
    }
}

fn render_tool_execution_status_badge(
    ui: &mut egui::Ui,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    let label = tool_execution_status_label(tool);
    let tokens = crate::ui::theme::tokens_from_ui(ui);
    let (fill, text_color) = match &tool.result {
        ploke_records::run_record::ToolResult::Completed(_) => (tokens.success, tokens.badge_text),
        ploke_records::run_record::ToolResult::Failed(_) => (tokens.error, tokens.badge_text),
    };
    ui.label(
        egui::RichText::new(label)
            .background_color(fill)
            .color(text_color)
            .monospace(),
    );
}

fn tool_execution_content(tool: &ploke_records::run_record::ToolExecutionRecord) -> &str {
    match &tool.result {
        ploke_records::run_record::ToolResult::Completed(result) => result.content.as_str(),
        ploke_records::run_record::ToolResult::Failed(result) => result.error.as_str(),
    }
}

fn tool_execution_ui_payload(
    tool: &ploke_records::run_record::ToolExecutionRecord,
) -> Option<&ploke_records::agent_turn::ToolUiPayloadRecord> {
    match &tool.result {
        ploke_records::run_record::ToolResult::Completed(result) => result.ui_payload.as_ref(),
        ploke_records::run_record::ToolResult::Failed(result) => result.ui_payload.as_ref(),
    }
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_ui_payload")
)]
fn render_tool_ui_payload(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
    payload: &ploke_records::agent_turn::ToolUiPayloadRecord,
) {
    let call_id = tool.request.call_id.as_str();
    show_tool_steps_collapsing(
        ui,
        egui::CollapsingHeader::new("tool ui payload").id_salt(tool_steps_id_salt(
            render_cache,
            ("run-record-tool-ui-payload", index, call_id),
        )),
        render_cache.tool_steps_default_open(true),
        |ui| {
            ui.label(
                egui::RichText::new("Exported tool UI witness")
                    .small()
                    .weak(),
            );
            tool_kv_text(ui, render_cache, "tool", payload.tool.as_str());
            tool_kv_text(ui, render_cache, "call id", payload.call_id.as_str());
            if let Some(request_id) = payload.request_id.as_deref() {
                tool_kv_text(ui, render_cache, "request id", request_id);
            }
            if let Some(proposal_id) = payload.proposal_id.as_deref() {
                tool_kv_text(ui, render_cache, "proposal id", proposal_id);
            }
            tool_kv_text(ui, render_cache, "summary", payload.summary.as_str());
            tool_kv_debug(ui, render_cache, "verbosity", payload.verbosity);
            for field in &payload.fields {
                if is_pathish_key(field.name.as_str()) {
                    tool_kv_path(ui, render_cache, field.name.as_str(), field.value.as_str());
                } else {
                    tool_kv_text(ui, render_cache, field.name.as_str(), field.value.as_str());
                }
            }
            if let Some(details) = payload.details.as_deref() {
                show_tool_steps_collapsing(
                    ui,
                    egui::CollapsingHeader::new("details").id_salt(tool_steps_id_salt(
                        render_cache,
                        ("run-record-tool-ui-details", index, call_id),
                    )),
                    render_cache.tool_steps_default_open(false),
                    |ui| {
                        render_tool_ui_payload_details(ui, render_cache, details);
                    },
                );
            }
            if let Some(error) = payload.error.as_ref() {
                render_tool_error_wire(ui, render_cache, record_ctx, index, tool, error);
            }
        },
    );
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_ui_details")
)]
fn render_tool_ui_payload_details(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    details: &str,
) {
    cached_wrapped_monospace_label(ui, render_cache, details);
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), feature = "native-benchmark"),
    tracing::instrument(skip_all, name = "inspector_tool_error")
)]
fn render_tool_error_wire(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    index: usize,
    tool: &ploke_records::run_record::ToolExecutionRecord,
    error: &ploke_records::agent_turn::ToolErrorWireRecord,
) {
    let call_id = tool.request.call_id.as_str();
    show_tool_steps_collapsing(
        ui,
        egui::CollapsingHeader::new("typed error").id_salt(tool_steps_id_salt(
            render_cache,
            ("run-record-tool-typed-error-section", index, call_id),
        )),
        render_cache.tool_steps_default_open(true),
        |ui| {
            let detail = detail_for_lane_at_step(
                EvidenceLane::RecordedTypedErrorWire,
                record_ctx,
                Some(call_id),
                Some(tool.request.tool.as_str()),
                Some(index),
            );
            render_evidence_lane_chip_with_inspect(
                ui,
                EvidenceLane::RecordedTypedErrorWire,
                detail,
                ("run-record-tool-typed-error", index, call_id),
            );
            tool_kv_text(ui, render_cache, "user", error.user.as_str());
            tool_kv_text(ui, render_cache, "system", error.system.as_str());
            tool_kv_bool(ui, render_cache, "ok", error.llm.ok);
            tool_kv_text(ui, render_cache, "tool", error.llm.tool.as_str());
            tool_kv_debug(ui, render_cache, "code", error.llm.code);
            if let Some(field) = error.llm.field.as_deref() {
                tool_kv_text(ui, render_cache, "field", field);
            }
            if let Some(expected) = error.llm.expected.as_deref() {
                tool_kv_text(ui, render_cache, "expected", expected);
            }
            if let Some(received) = error.llm.received.as_deref() {
                tool_kv_text(ui, render_cache, "received", received);
            }
            tool_kv_text(ui, render_cache, "message", error.llm.message.as_str());
            if let Some(hint) = error.llm.retry_hint.as_deref() {
                tool_kv_text(ui, render_cache, "retry hint", hint);
            }
        },
    );
}

fn render_tool_failure_content(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    tool: &ploke_records::run_record::ToolExecutionRecord,
) {
    if let ploke_records::run_record::ToolResult::Failed(result) = &tool.result {
        tool_kv_text(ui, render_cache, "status", "failed");
        if let Some(tool_name) = result.tool.as_deref() {
            tool_kv_text(ui, render_cache, "tool", tool_name);
        }
        tool_kv_text(ui, render_cache, "error", result.error.as_str());
    }
}

pub(super) fn render_decoded_tool_arguments(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    decoded: &PersistedToolCallArguments,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    tool_step: Option<(usize, &ploke_records::run_record::ToolExecutionRecord)>,
) {
    match decoded {
        PersistedToolCallArguments::Decoded(arguments) => {
            render_run_record_decode_lane_chip(
                ui,
                EvidenceLane::UiDecodeOk,
                record_ctx,
                tool_step,
                "run-record-tool-args-decode-ok",
            );
            render_tool_call_arguments(ui, render_cache, arguments);
        }
        PersistedToolCallArguments::ParseFailure(failure) => {
            render_run_record_decode_lane_chip(
                ui,
                EvidenceLane::UiDecodeFailed,
                record_ctx,
                tool_step,
                "run-record-tool-args-decode-fail",
            );
            tool_kv_text(ui, render_cache, "tool", failure.tool.as_str());
            tool_kv_debug(ui, render_cache, "error", &failure.error);
        }
    }
}

fn render_tool_call_arguments(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    arguments: &ToolCallArguments,
) {
    match arguments {
        ToolCallArguments::RequestCodeContext(args) => {
            render_optional_u32(
                ui,
                render_cache,
                "token budget per result",
                args.token_budget_per_result,
            );
            render_optional_u32(
                ui,
                render_cache,
                "token budget total",
                args.token_budget_total,
            );
            render_optional_str(ui, render_cache, "search term", args.search_term.as_deref());
        }
        ToolCallArguments::ApplyCodeEdit(args) => {
            render_optional_f32(ui, render_cache, "confidence", args.confidence);
            tool_kv_usize(ui, render_cache, "edits", args.edits.len());
            for (index, edit) in args.edits.iter().enumerate() {
                show_tool_steps_collapsing(
                    ui,
                    egui::CollapsingHeader::new(format!("edit {}", index + 1)).id_salt(
                        tool_steps_id_salt(render_cache, ("run-record-tool-edit", index)),
                    ),
                    render_cache.tool_steps_default_open(index == 0),
                    |ui| {
                        let _span =
                            tracing::trace_span!(scope::INSPECTOR_TOOL_ARGUMENT_EDIT).entered();
                        tool_kv_path(ui, render_cache, "file", edit.file.as_str());
                        tool_kv_text(ui, render_cache, "canon", edit.canon.as_str());
                        tool_kv_debug(ui, render_cache, "node type", edit.node_type);
                        tool_kv_text_size_summary(ui, render_cache, "code", edit.code.as_str());
                    },
                );
            }
        }
        ToolCallArguments::InsertRustItem(args) => {
            tool_kv_path(ui, render_cache, "file", args.file.as_str());
            tool_kv_debug(ui, render_cache, "container kind", args.container_kind);
            render_optional_str(
                ui,
                render_cache,
                "container canon",
                args.container_canon.as_deref(),
            );
            tool_kv_debug(ui, render_cache, "item kind", args.item_kind);
            render_optional_f32(ui, render_cache, "confidence", args.confidence);
            tool_kv_text_size_summary(ui, render_cache, "code", args.code.as_str());
        }
        ToolCallArguments::CreateFile(args) => {
            tool_kv_path(ui, render_cache, "file path", args.file_path.as_str());
            render_optional_str(ui, render_cache, "on exists", args.on_exists.as_deref());
            tool_kv_bool(ui, render_cache, "create parents", args.create_parents);
            tool_kv_text_size_summary(ui, render_cache, "content", args.content.as_str());
        }
        ToolCallArguments::NsPatch(args) => {
            render_optional_f32(ui, render_cache, "confidence", args.confidence);
            tool_kv_usize(ui, render_cache, "patches", args.patches.len());
            for (index, patch) in args.patches.iter().enumerate() {
                show_tool_steps_collapsing(
                    ui,
                    egui::CollapsingHeader::new(format!("patch {}", index + 1)).id_salt(
                        tool_steps_id_salt(render_cache, ("run-record-tool-patch", index)),
                    ),
                    render_cache.tool_steps_default_open(index == 0),
                    |ui| {
                        let _span =
                            tracing::trace_span!(scope::INSPECTOR_TOOL_ARGUMENT_PATCH).entered();
                        tool_kv_path(ui, render_cache, "file", patch.file.as_str());
                        tool_kv_text(ui, render_cache, "reasoning", patch.reasoning.as_str());
                        tool_kv_text_size_summary(ui, render_cache, "diff", patch.diff.as_str());
                    },
                );
            }
        }
        ToolCallArguments::NsRead(args) => {
            tool_kv_path(ui, render_cache, "file", args.file.as_str());
            render_optional_u32(ui, render_cache, "start line", args.start_line);
            render_optional_u32(ui, render_cache, "end line", args.end_line);
            render_optional_u32(ui, render_cache, "max bytes", args.max_bytes);
        }
        ToolCallArguments::CodeItemLookup(args) => {
            render_code_item_query(
                ui,
                render_cache,
                args.item_name.as_str(),
                args.file_path.as_str(),
                args.node_kind.as_str(),
                args.module_path.as_str(),
            );
        }
        ToolCallArguments::CodeItemEdges(args) => {
            render_code_item_query(
                ui,
                render_cache,
                args.item_name.as_str(),
                args.file_path.as_str(),
                args.node_kind.as_str(),
                args.module_path.as_str(),
            );
        }
        ToolCallArguments::Cargo(args) => {
            tool_kv_debug(ui, render_cache, "command", args.command);
            tool_kv_debug(ui, render_cache, "scope", args.scope);
            render_optional_str(ui, render_cache, "package", args.package.as_deref());
            render_optional_string_list(ui, render_cache, "features", args.features.as_deref());
            tool_kv_bool(ui, render_cache, "all features", args.all_features);
            tool_kv_bool(
                ui,
                render_cache,
                "no default features",
                args.no_default_features,
            );
            render_optional_str(ui, render_cache, "target", args.target.as_deref());
            render_optional_str(ui, render_cache, "profile", args.profile.as_deref());
            tool_kv_bool(ui, render_cache, "release", args.release);
            tool_kv_bool(ui, render_cache, "lib", args.lib);
            tool_kv_bool(ui, render_cache, "tests", args.tests);
            tool_kv_bool(ui, render_cache, "bins", args.bins);
            tool_kv_bool(ui, render_cache, "examples", args.examples);
            tool_kv_bool(ui, render_cache, "benches", args.benches);
            render_optional_string_list(ui, render_cache, "test args", args.test_args.as_deref());
        }
        ToolCallArguments::ListDir(args) => {
            tool_kv_path(ui, render_cache, "dir", args.dir.as_str());
            tool_kv_bool(ui, render_cache, "include hidden", args.include_hidden);
            render_optional_str(ui, render_cache, "sort", args.sort.as_deref());
            render_optional_u32(ui, render_cache, "max entries", args.max_entries);
        }
        ToolCallArguments::SearchCode(args)
        | ToolCallArguments::SearchSymbols(args)
        | ToolCallArguments::QueryCodebase(args) => {
            render_optional_str(ui, render_cache, "search term", args.search_term.as_deref());
            render_optional_str(ui, render_cache, "query", args.query.as_deref());
        }
    }
}

pub(super) fn render_decoded_tool_result(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    decoded: &PersistedToolResultContent,
    record_ctx: Option<RunRecordProvenanceCtx<'_>>,
    tool_step: Option<(usize, &ploke_records::run_record::ToolExecutionRecord)>,
) {
    match decoded {
        PersistedToolResultContent::Decoded(result) => {
            render_run_record_decode_lane_chip(
                ui,
                EvidenceLane::UiDecodeOk,
                record_ctx,
                tool_step,
                "run-record-tool-content-decode-ok",
            );
            render_tool_result_content(ui, render_cache, result);
        }
        PersistedToolResultContent::ParseFailure(failure) => {
            render_run_record_decode_lane_chip(
                ui,
                EvidenceLane::UiDecodeFailed,
                record_ctx,
                tool_step,
                "run-record-tool-content-decode-fail",
            );
            tool_kv_text(ui, render_cache, "tool", failure.tool.as_str());
            tool_kv_debug(ui, render_cache, "error", &failure.error);
        }
    }
}

fn render_tool_result_content(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    result: &ToolResultContent,
) {
    match result {
        ToolResultContent::RequestCodeContext(result) => {
            tool_kv_bool(ui, render_cache, "ok", result.ok);
            tool_kv_text(ui, render_cache, "search term", result.search_term.as_str());
            tool_kv_usize(ui, render_cache, "top k", result.top_k);
            tool_kv_debug(ui, render_cache, "kind", result.kind);
            render_optional_str(ui, render_cache, "note", result.note.as_deref());
            render_string_list(ui, render_cache, "next steps", &result.next_steps);
            tool_kv_usize(ui, render_cache, "context items", result.context.len());
            for (index, item) in result.context.iter().take(3).enumerate() {
                render_concise_context(ui, render_cache, index, item);
            }
        }
        ToolResultContent::ApplyCodeEdit(result) | ToolResultContent::InsertRustItem(result) => {
            render_patch_like_result(
                ui,
                render_cache,
                result.ok,
                result.staged,
                result.applied,
                &result.files,
                result.preview_mode.as_str(),
                result.auto_confirmed,
            );
        }
        ToolResultContent::CreateFile(result) => {
            render_patch_like_result(
                ui,
                render_cache,
                result.ok,
                result.staged,
                result.applied,
                &result.files,
                result.preview_mode.as_str(),
                result.auto_confirmed,
            );
        }
        ToolResultContent::NsPatch(result) => {
            render_patch_like_result(
                ui,
                render_cache,
                result.ok,
                result.staged,
                result.applied,
                &result.files,
                result.preview_mode.as_str(),
                result.auto_confirmed,
            );
        }
        ToolResultContent::NsRead(result) => {
            tool_kv_bool(ui, render_cache, "ok", result.ok);
            tool_kv_path(ui, render_cache, "file path", result.file_path.as_str());
            tool_kv_bool(ui, render_cache, "exists", result.exists);
            render_optional_u64(ui, render_cache, "byte len", result.byte_len);
            render_optional_u32(ui, render_cache, "start line", result.start_line);
            render_optional_u32(ui, render_cache, "end line", result.end_line);
            tool_kv_bool(ui, render_cache, "truncated", result.truncated);
            if let Some(hash) = result.file_hash.as_ref() {
                tool_kv_debug(ui, render_cache, "file hash", hash);
            }
            if let Some(content) = result.content.as_deref() {
                tool_kv_text_size_summary(ui, render_cache, "content", content);
            }
        }
        ToolResultContent::CodeItemLookup(result) => {
            render_concise_context(ui, render_cache, 0, result);
        }
        ToolResultContent::Cargo(result) => {
            tool_kv_bool(ui, render_cache, "ok", result.ok);
            tool_kv_debug(ui, render_cache, "status", result.status_reason);
            tool_kv_debug(ui, render_cache, "command", result.command);
            tool_kv_debug(ui, render_cache, "scope", result.scope);
            tool_kv_path(ui, render_cache, "manifest", result.manifest_path.as_str());
            render_optional_i32(ui, render_cache, "exit code", result.exit_code);
            tool_kv_u64(ui, render_cache, "duration ms", result.duration_ms);
            tool_kv_u32(ui, render_cache, "errors", result.summary.errors);
            tool_kv_u32(ui, render_cache, "warnings", result.summary.warnings);
            tool_kv_u32(ui, render_cache, "notes", result.summary.notes);
            tool_kv_usize(ui, render_cache, "diagnostics", result.diagnostics.len());
            tool_kv_bool(ui, render_cache, "truncated", result.raw_messages_truncated);
        }
        ToolResultContent::ListDir(result) => {
            tool_kv_usize(ui, render_cache, "entries", result.entries.len());
            show_tool_steps_collapsing(
                ui,
                egui::CollapsingHeader::new("details").id_salt(tool_steps_id_salt(
                    render_cache,
                    "run-record-tool-list-dir-details",
                )),
                render_cache.tool_steps_default_open(false),
                |ui| {
                    let _span = tracing::trace_span!(scope::INSPECTOR_TOOL_RESULT_LIST_DIR_DETAILS)
                        .entered();
                    tool_kv_bool(ui, render_cache, "ok", result.ok);
                    tool_kv_path(ui, render_cache, "dir", result.dir.as_str());
                    tool_kv_bool(ui, render_cache, "exists", result.exists);
                    tool_kv_bool(ui, render_cache, "truncated", result.truncated);
                },
            );
            for (index, entry) in result.entries.iter().take(8).enumerate() {
                show_tool_steps_collapsing(
                    ui,
                    egui::CollapsingHeader::new(entry.name.as_str()).id_salt(tool_steps_id_salt(
                        render_cache,
                        ("run-record-tool-list-dir-entry", index, entry.path.as_str()),
                    )),
                    render_cache.tool_steps_default_open(index == 0),
                    |ui| {
                        let _span =
                            tracing::trace_span!(scope::INSPECTOR_TOOL_RESULT_LIST_DIR_ENTRY)
                                .entered();
                        tool_kv_path(ui, render_cache, "path", entry.path.as_str());
                        tool_kv_text(ui, render_cache, "kind", entry.kind.as_str());
                        render_optional_u64(ui, render_cache, "size bytes", entry.size_bytes);
                    },
                );
            }
        }
    }
}

fn render_patch_like_result(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    ok: bool,
    staged: usize,
    applied: usize,
    files: &[String],
    preview_mode: &str,
    auto_confirmed: bool,
) {
    tool_kv_bool(ui, render_cache, "ok", ok);
    tool_kv_usize(ui, render_cache, "staged", staged);
    tool_kv_usize(ui, render_cache, "applied", applied);
    tool_kv_text(ui, render_cache, "preview mode", preview_mode);
    tool_kv_bool(ui, render_cache, "auto confirmed", auto_confirmed);
    render_path_list(ui, render_cache, "files", files);
}

fn render_code_item_query(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    item_name: &str,
    file_path: &str,
    node_kind: &str,
    module_path: &str,
) {
    tool_kv_text(ui, render_cache, "item name", item_name);
    tool_kv_path(ui, render_cache, "file path", file_path);
    tool_kv_text(ui, render_cache, "node kind", node_kind);
    tool_kv_text(ui, render_cache, "module path", module_path);
}

fn render_concise_context(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    index: usize,
    context: &ploke_records::tool_contracts::ConciseContext,
) {
    show_tool_steps_collapsing(
        ui,
        egui::CollapsingHeader::new(format!("context {}", index + 1)).id_salt(tool_steps_id_salt(
            render_cache,
            ("run-record-tool-context", index),
        )),
        render_cache.tool_steps_default_open(index == 0),
        |ui| {
            let _span = tracing::trace_span!(scope::INSPECTOR_CONTEXT).entered();
            tool_kv_path(ui, render_cache, "file", context.file_path.as_ref());
            tool_kv_text(ui, render_cache, "canon", context.canon_path.as_ref());
            tool_kv_text_size_summary(ui, render_cache, "snippet", context.snippet.as_str());
        },
    );
}

fn render_optional_str(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        tool_kv_text(ui, render_cache, key, value);
    }
}

fn render_optional_string_list(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<&[String]>,
) {
    if let Some(value) = value {
        render_string_list(ui, render_cache, key, value);
    }
}

fn render_string_list(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    values: &[String],
) {
    tool_kv_usize(ui, render_cache, key, values.len());
    for (index, value) in values.iter().take(8).enumerate() {
        tool_kv_text(ui, render_cache, list_item_key(index), value.as_str());
    }
}

fn render_path_list(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    values: &[String],
) {
    tool_kv_usize(ui, render_cache, key, values.len());
    for (index, value) in values.iter().take(8).enumerate() {
        tool_kv_path(ui, render_cache, list_item_key(index), value.as_str());
    }
}

fn render_optional_u32(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<u32>,
) {
    if let Some(value) = value {
        tool_kv_u32(ui, render_cache, key, value);
    }
}

fn render_optional_u64(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<u64>,
) {
    if let Some(value) = value {
        tool_kv_u64(ui, render_cache, key, value);
    }
}

fn render_optional_i32(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<i32>,
) {
    if let Some(value) = value {
        tool_kv_owned(ui, render_cache, key, value.to_string());
    }
}

fn render_optional_f32(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: Option<f32>,
) {
    if let Some(value) = value {
        tool_kv_owned(ui, render_cache, key, format!("{value:.2}"));
    }
}

fn tool_kv_text(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal_wrapped(|ui| {
        cached_label(ui, render_cache, key);
        cached_monospace_label(ui, render_cache, value);
    });
}

fn tool_kv_path(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: &str,
) {
    ui.horizontal_wrapped(|ui| {
        cached_label(ui, render_cache, key);
        cached_path_value(ui, render_cache, ("tool-path", key, value), value);
    });
}

fn is_pathish_key(key: &str) -> bool {
    matches!(
        key,
        "dir" | "file" | "file path" | "manifest" | "path" | "record" | "repo root"
    ) || key.ends_with(" path")
}

fn tool_kv_owned(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: String,
) {
    tool_kv_text(ui, render_cache, key, value.as_str());
}

fn tool_kv_bool(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: bool,
) {
    tool_kv_text(ui, render_cache, key, if value { "true" } else { "false" });
}

fn tool_kv_u32(ui: &mut egui::Ui, render_cache: &mut InspectorRenderCache, key: &str, value: u32) {
    let mut buffer = itoa::Buffer::new();
    tool_kv_text(ui, render_cache, key, buffer.format(value));
}

fn tool_kv_u64(ui: &mut egui::Ui, render_cache: &mut InspectorRenderCache, key: &str, value: u64) {
    let mut buffer = itoa::Buffer::new();
    tool_kv_text(ui, render_cache, key, buffer.format(value));
}

fn tool_kv_usize(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: usize,
) {
    let mut buffer = itoa::Buffer::new();
    tool_kv_text(ui, render_cache, key, buffer.format(value));
}

fn tool_kv_debug(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    value: impl std::fmt::Debug,
) {
    tool_kv_owned(ui, render_cache, key, format!("{value:?}"));
}

fn tool_kv_text_size_summary(
    ui: &mut egui::Ui,
    render_cache: &mut InspectorRenderCache,
    key: &str,
    text: &str,
) {
    let value = render_cache.text_size_summary(text);
    tool_kv_text(ui, render_cache, key, value.as_ref());
}

fn list_item_key(index: usize) -> &'static str {
    match index {
        0 => "1",
        1 => "2",
        2 => "3",
        3 => "4",
        4 => "5",
        5 => "6",
        6 => "7",
        _ => "8",
    }
}

#[cfg(test)]
mod tests {
    use super::{InspectorRenderCache, render_tool_ui_payload};
    use eframe::egui;

    #[test]
    fn tool_ui_payload_renderer_keeps_cached_payload_labels_visible() {
        use ploke_records::agent_turn::{
            ToolUiFieldRecord, ToolUiPayloadRecord, ToolVerbosityRecord,
        };
        use ploke_records::tool_contracts::ToolName;

        let payload = ToolUiPayloadRecord {
            tool: ToolName::ApplyCodeEdit,
            call_id: "call-1".to_owned(),
            request_id: Some("request-1".to_owned()),
            proposal_id: Some("proposal-1".to_owned()),
            summary: "edit staged".to_owned(),
            fields: vec![ToolUiFieldRecord {
                name: "status".to_owned(),
                value: "staged".to_owned(),
            }],
            details: Some("Ready to apply".to_owned()),
            verbosity: ToolVerbosityRecord::Normal,
            error: None,
            error_code: None,
        };
        use ploke_records::agent_turn::ToolCompletedRecord;
        use ploke_records::agent_turn::ToolRequestRecord;
        use ploke_records::run_record::{ToolExecutionRecord, ToolResult};
        let tool = ToolExecutionRecord {
            request: ToolRequestRecord {
                request_id: "request-1".to_owned(),
                parent_id: "parent-1".to_owned(),
                call_id: "call-1".to_owned(),
                tool: "apply_code_edit".to_owned(),
                arguments: "{}".into(),
            },
            result: ToolResult::Completed(ToolCompletedRecord {
                request_id: "request-1".to_owned(),
                parent_id: "parent-1".to_owned(),
                call_id: "call-1".to_owned(),
                tool: "apply_code_edit".to_owned(),
                content: "{}".to_owned(),
                ui_payload: Some(payload.clone()),
                latency_ms: 1,
            }),
            latency_ms: 1,
        };
        let mut cache = InspectorRenderCache::default();
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::empty());

        let output = ctx.run_ui(Default::default(), |ui| {
            render_tool_ui_payload(ui, &mut cache, None, 0, &tool, &payload);
        });
        let texts = clipped_shape_texts(&output.shapes);

        assert!(texts.iter().any(|text| text.contains("tool ui payload")));
        assert!(texts.iter().any(|text| text.contains("tool")));
        assert!(texts.iter().any(|text| text.contains("apply_code_edit")));
        assert!(texts.iter().any(|text| text.contains("call id")));
        assert!(texts.iter().any(|text| text.contains("call-1")));
        assert!(texts.iter().any(|text| text.contains("summary")));
        assert!(texts.iter().any(|text| text.contains("edit staged")));
        assert!(texts.iter().any(|text| text.contains("status")));
        assert!(texts.iter().any(|text| text.contains("staged")));
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
