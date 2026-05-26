use crate::allocation::scope;
use crate::ui::inspector::{
    InspectorSections, RunRecordSlot, surface_apply_status_label, surface_check_status_label,
};
use eframe::egui;
use ploke_tree::Graph;
use ploke_tree::graph::{ParentCreateAttempt, ParentCreateLookup};

use super::cache::ParentCreateRowsKey;
use super::fields::*;
use super::{
    InspectorOpenState, InspectorPanelSection, InspectorRenderCache, render_agent_turns,
    render_run_record_turns, render_unavailable, show_inspector_collapsing,
};

pub(crate) fn render_parent_create_for_inspector(
    ui: &mut egui::Ui,
    graph: &Graph,
    sections: &InspectorSections,
    render_cache: &mut InspectorRenderCache,
    open_state: InspectorOpenState,
) {
    if let Some(reason) = sections.unavailable() {
        render_unavailable(ui, reason);
        return;
    }
    match sections.parent_create() {
        Some(slot) => render_parent_create(
            ui,
            graph,
            slot.resolve(graph),
            sections.run_records(),
            render_cache,
            open_state,
        ),
        None => kv(ui, "attempt", "not_available"),
    }
}

fn render_parent_create(
    ui: &mut egui::Ui,
    graph: &Graph,
    lookup: ParentCreateLookup<'_, '_>,
    run_record_slots: &[RunRecordSlot],
    render_cache: &mut InspectorRenderCache,
    open_state: InspectorOpenState,
) {
    match lookup {
        ParentCreateLookup::Attempt(attempt) => {
            render_parent_create_attempt(
                ui,
                graph,
                attempt,
                run_record_slots,
                render_cache,
                open_state,
            );
        }
        ParentCreateLookup::Unavailable(reason) => {
            cached_kv_id(ui, render_cache, "attempt", "missing");
            render_parent_create_unavailable(ui, reason, render_cache);
        }
        ParentCreateLookup::Ambiguous { count, reason } => {
            cached_kv_id(ui, render_cache, "attempt", "ambiguous");
            cached_kv_usize(ui, render_cache, "matches", count);
            render_parent_create_unavailable(ui, reason, render_cache);
        }
    }
}

fn render_parent_create_attempt(
    ui: &mut egui::Ui,
    graph: &Graph,
    attempt: ParentCreateAttempt<'_>,
    run_record_slots: &[RunRecordSlot],
    render_cache: &mut InspectorRenderCache,
    open_state: InspectorOpenState,
) {
    let child = attempt.child();
    let surface = attempt.surface();
    let branch_summary = run_record_turn_summary(graph, run_record_slots);
    let summary = branch_summary.unwrap_or_else(|| agent_turn_summary(attempt));
    let (surface_producer, router_model) = match attempt.surface_producer() {
        Some(ploke_records::history::SurfaceProposalProducerRecord::NonRouter) => {
            (Some("non_router"), None)
        }
        Some(ploke_records::history::SurfaceProposalProducerRecord::Router { request_policy }) => {
            (Some("router"), Some(request_policy.model.value.as_str()))
        }
        None => (None, None),
    };
    let rows = render_cache.parent_create_rows(ParentCreateRowsKey {
        surface_touches: surface.map(|surface| surface.touches.len()),
        check_status: surface.map(|surface| surface_check_status_label(surface.check_status)),
        apply_status: surface.map(|surface| surface_apply_status_label(surface.apply_status)),
        tool_requested: summary.tool_requested,
        tool_completed: summary.tool_completed,
        tool_failed: summary.tool_failed,
        edit_proposals: summary.edit_proposals,
        create_proposals: summary.create_proposals,
        expected_file_changes: summary.expected_file_changes,
        candidate_evaluations: attempt.candidate_evaluation_count(),
    });

    cached_kv_id(ui, render_cache, "attempt", "available");
    cached_kv_path(
        ui,
        render_cache,
        "target",
        child
            .request
            .target_relpath
            .to_str()
            .unwrap_or("non_utf8_path"),
    );
    if let Some(producer) = surface_producer {
        cached_kv_id(ui, render_cache, "surface", producer);
    }
    if let Some(touched_files) = rows.surface_touches.as_ref() {
        cached_kv_text(ui, render_cache, "surface touches", touched_files);
    }
    if let Some(check_apply) = rows.check_apply.as_ref() {
        cached_kv_text(ui, render_cache, "check/apply", check_apply);
    }
    if let Some(model) = router_model {
        cached_kv_id(ui, render_cache, "model", model);
    } else if surface_producer == Some("non_router") {
        cached_kv_id(ui, render_cache, "model", "not_applicable");
    }
    cached_kv_text(ui, render_cache, "tools", rows.tools.as_ref());
    cached_kv_text(ui, render_cache, "llm proposal", rows.llm_proposal.as_ref());
    cached_kv_text(ui, render_cache, "child eval", rows.child_eval.as_ref());

    render_parent_create_llm_calls(
        ui,
        graph,
        &attempt,
        run_record_slots,
        render_cache,
        open_state,
    );

    // render_run_record_turns(ui, render_cache, graph, run_record_slots);
    // render_agent_turns(ui, render_cache, attempt.agent_turns());
    render_parent_create_source_status(ui, &attempt, render_cache);
}

pub(crate) fn render_parent_create_llm_calls(
    ui: &mut egui::Ui,
    graph: &Graph,
    attempt: &ParentCreateAttempt<'_>,
    run_record_slots: &[RunRecordSlot],
    render_cache: &mut InspectorRenderCache,
    open_state: InspectorOpenState,
) {
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("LLM calls")
            .default_open(false)
            .open(open_state.open(InspectorPanelSection::LlmCalls)),
        |ui| {
            render_parent_create_llm_calls_body(ui, graph, attempt, run_record_slots, render_cache)
        },
    );
}

pub(crate) fn render_parent_create_llm_calls_body(
    ui: &mut egui::Ui,
    graph: &Graph,
    attempt: &ParentCreateAttempt<'_>,
    run_record_slots: &[RunRecordSlot],
    render_cache: &mut InspectorRenderCache,
) {
    let _span = tracing::trace_span!(scope::INSPECTOR_PARENT_CREATE_LLM_CALLS).entered();
    if render_run_record_turns(ui, render_cache, graph, run_record_slots) {
        return;
    }
    cached_kv_id(ui, render_cache, "evidence", "agent_turn_sidecar_fallback");
    render_agent_turns(ui, render_cache, attempt.agent_turns());
}

fn render_parent_create_source_status(
    ui: &mut egui::Ui,
    attempt: &ParentCreateAttempt<'_>,
    render_cache: &mut InspectorRenderCache,
) {
    let child = attempt.child();
    show_inspector_collapsing(
        ui,
        egui::CollapsingHeader::new("Source status").default_open(false),
        |ui| {
            let _span =
                tracing::trace_span!(scope::INSPECTOR_PARENT_CREATE_SOURCE_STATUS).entered();
            ui.horizontal(|ui| {
                cached_label(ui, render_cache, "child");
                cached_expandable_id(
                    ui,
                    render_cache,
                    ("parent-create-child", child.node.node_id.as_str()),
                    child.node.node_id.as_str(),
                );
            });
            ui.horizontal(|ui| {
                cached_label(ui, render_cache, "branch");
                cached_expandable_id(
                    ui,
                    render_cache,
                    (
                        "parent-create-branch",
                        child.resolved.branch.branch_id.as_str(),
                    ),
                    child.resolved.branch.branch_id.as_str(),
                );
            });
            ui.horizontal(|ui| {
                cached_label(ui, render_cache, "candidate");
                cached_expandable_id(
                    ui,
                    render_cache,
                    (
                        "parent-create-candidate",
                        child.resolved.branch.candidate_id.as_str(),
                    ),
                    child.resolved.branch.candidate_id.as_str(),
                );
            });
            cached_kv_usize(ui, render_cache, "record refs", attempt.source_ref_count());
        },
    );
}

fn render_parent_create_unavailable(
    ui: &mut egui::Ui,
    reason: ploke_tree::graph::ParentCreateUnavailable<'_>,
    render_cache: &mut InspectorRenderCache,
) {
    let (record, key, value) = match reason {
        ploke_tree::graph::ParentCreateUnavailable::MissingJoin { record, key, value }
        | ploke_tree::graph::ParentCreateUnavailable::AmbiguousJoin { record, key, value } => {
            (record, key, value)
        }
    };

    ui.horizontal(|ui| {
        cached_label(ui, render_cache, record);
        cached_monospace_label(ui, render_cache, key);
        cached_expandable_id(
            ui,
            render_cache,
            ("parent-create-unavailable", record, key, value),
            value,
        );
    });
}

#[derive(Default, Clone, Copy)]
struct AgentTurnSummary {
    tool_requested: usize,
    tool_completed: usize,
    tool_failed: usize,
    edit_proposals: usize,
    create_proposals: usize,
    expected_file_changes: usize,
}

fn agent_turn_summary(attempt: ParentCreateAttempt<'_>) -> AgentTurnSummary {
    let mut summary = AgentTurnSummary::default();
    for turn in attempt.agent_turns() {
        summary.tool_requested += turn.tool_request_event_count;
        summary.tool_completed += turn.tool_completed_event_count;
        summary.tool_failed += turn.tool_failed_event_count;
        summary.edit_proposals += turn.edit_proposal_count;
        summary.create_proposals += turn.create_proposal_count;
        summary.expected_file_changes += turn.expected_file_change_count;
    }
    summary
}

/// archaeology:run-record-branch-output
/// proof:docs/active/archaeology/ploke-tree-graph/run-record-branch-output.md
fn run_record_turn_summary(
    graph: &Graph,
    run_record_slots: &[RunRecordSlot],
) -> Option<AgentTurnSummary> {
    let mut rendered = false;
    let mut summary = AgentTurnSummary::default();
    for record in run_record_slots
        .iter()
        .filter_map(|slot| slot.resolve(graph))
    {
        if record.stats.turn_count == 0 {
            continue;
        }
        rendered = true;
        summary.tool_requested += record.stats.tool_call_count;
        summary.tool_failed += record.stats.failed_tool_call_count;
        summary.tool_completed += record
            .stats
            .tool_call_count
            .saturating_sub(record.stats.failed_tool_call_count);
        for turn in record.turns() {
            if let Some(artifact) = turn.turn.agent_turn_artifact.as_ref() {
                summary.edit_proposals += artifact.patch_artifact.edit_proposals.len();
                summary.create_proposals += artifact.patch_artifact.create_proposals.len();
                summary.expected_file_changes +=
                    artifact.patch_artifact.expected_file_changes.len();
            }
        }
    }
    rendered.then_some(summary)
}
