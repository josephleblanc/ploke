//! Stable frame rendering for the operator graph UI.

use eframe::egui;

use crate::ui::diff;
use crate::ui::id_display;
use crate::ui::inspector::{
    ArtifactInspection, PatchInspection, RunForestNodeInspection, SelectionEdge,
    SelectionInspector, SourceRef, UnavailableReason, artifact_edges, artifact_metrics,
    artifact_source_refs, phase_label, result_class_label, run_forest_artifact_edges,
    run_forest_incoming_edges, run_forest_node_identity, run_forest_outgoing_edges,
    run_forest_source_refs, surface_apply_status_label, surface_check_status_label,
};
use crate::ui::text::decor::Badge;
use crate::ui::view::{GraphSelectionDetail, GraphViewDiagnostics, GraphViewMode};
use ploke_tree::graph::{AgentTurnArtifactMetadata, ParentCreateAttempt, ParentCreateLookup};

pub(crate) fn render_top_strip(
    ui: &mut egui::Ui,
    mode: GraphViewMode,
    run_name: Option<&str>,
    graph_has_content: bool,
) {
    ui.horizontal(|ui| {
        ui.label("ploke-egui");
        ui.separator();
        ui.label(format!("Mode: {}", mode.as_str()));
        if let Some(run_name) = run_name {
            ui.separator();
            ui.label(format!("Run: {run_name}"));
        }
        if !graph_has_content {
            ui.separator();
            ui.label("No run loaded");
        }
    });
}

pub(crate) fn render_right_inspector(
    ui: &mut egui::Ui,
    selection: Option<&GraphSelectionDetail>,
    inspector: Option<&SelectionInspector<'_>>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Inspector");
            ui.separator();

            ui.label("Summary");
            if let Some(selection) = selection {
                kv(ui, "kind", selection.kind.as_str());
                kv(ui, "label", selection.label.as_str());
            } else {
                kv(ui, "selection", "not_applicable");
            }

            ui.separator();
            ui.label("Identity");
            if let Some(inspector) = inspector {
                render_identity(ui, inspector);
            } else {
                kv(ui, "record refs", "not_applicable");
            }

            ui.separator();
            ui.label("Roles");
            if let Some(inspector) = inspector {
                render_roles_and_metrics(ui, inspector);
            } else {
                kv(ui, "roles", "not_applicable");
            }

            ui.separator();
            ui.label("Patch Generation");
            if let Some(inspector) = inspector {
                render_parent_create_for_inspector(ui, inspector);
            } else {
                kv(ui, "attempt", "not_applicable");
            }

            ui.separator();
            egui::CollapsingHeader::new("Graph edges")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_graph_edges_for_inspector(ui, inspector);
                    } else {
                        kv(ui, "edges", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Artifact edges")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_artifact_edges_for_inspector(ui, inspector);
                    } else {
                        kv(ui, "artifact edges", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Patch Debug")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_patches_for_inspector(ui, inspector);
                    } else {
                        kv(ui, "patch", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Source refs")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_source_refs_for_inspector(ui, inspector);
                    } else {
                        kv(ui, "record refs", "not_applicable");
                    }
                });
        });
}

pub(crate) fn render_bottom_timeline(
    ui: &mut egui::Ui,
    diagnostics: Option<&GraphViewDiagnostics>,
    selection: Option<&GraphSelectionDetail>,
) {
    ui.horizontal(|ui| {
        ui.label("Timeline");
        ui.separator();
        if let Some(diagnostics) = diagnostics {
            ui.label(format!(
                "nodes={}, edges={}",
                diagnostics.node_count, diagnostics.edge_count
            ));
        } else {
            ui.label("spans=0");
        }
        ui.separator();
        ui.label(format!(
            "selection={}",
            if selection.is_some() {
                "synced"
            } else {
                "not_applicable"
            }
        ));
        ui.separator();
        ui.label("order_strength=blocked");
    });
}

fn kv(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(key);
        id_display::expandable_id(ui, ("kv", key, value), value);
    });
}

fn render_badges(ui: &mut egui::Ui, badges: &[Badge<'_>]) {
    if badges.is_empty() {
        kv(ui, "none", "not_applicable");
        return;
    }
    for badge in badges {
        ui.horizontal(|ui| {
            let badge_text = badge.to_badge_text();
            let artifact_id = badge_text.artifact_id();
            badge_text.show(ui);
            id_display::expandable_id(
                ui,
                ("role-badge", artifact_id.0.as_str()),
                artifact_id.0.as_str(),
            );
        });
    }
}

fn render_identity(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => render_run_forest_identity(ui, run),
        SelectionInspector::Artifact(artifact) => render_artifact_identity(ui, artifact),
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_run_forest_identity(ui: &mut egui::Ui, run: &RunForestNodeInspection<'_>) {
    let identity = run_forest_node_identity(run.node);
    kv(ui, "run forest node", identity.node_key);
    kv(ui, "candidate", identity.candidate_id);
    kv(ui, "source artifact", identity.source_artifact);
    if let Some(parent) = identity.parent_node {
        kv(ui, "parent run forest node", parent);
    }
    if let Some(base) = identity.base_artifact {
        kv(ui, "base artifact", base);
    }
    if let Some(derived) = identity.derived_artifact {
        kv(ui, "derived artifact", derived);
    }
    if let Some(patch) = identity.patch {
        kv(ui, "patch", patch);
    }
    kv(ui, "branch", identity.branch_id);
    kv(ui, "target", identity.target_relpath);
    kv(ui, "phase", phase_label(identity.phase));
    kv(ui, "result", result_class_label(identity.result));
}

fn render_artifact_identity(ui: &mut egui::Ui, artifact: &ArtifactInspection<'_>) {
    let identity = artifact.identity();
    kv(ui, "artifact", identity.artifact());
}

fn render_roles_and_metrics(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => {
            render_badges(ui, &run.role_badges);
            render_run_forest_metrics(ui, run);
        }
        SelectionInspector::Artifact(artifact) => {
            render_badges(ui, &artifact.role_badges);
            render_artifact_metrics(ui, artifact);
        }
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_run_forest_metrics(ui: &mut egui::Ui, run: &RunForestNodeInspection<'_>) {
    ui.horizontal(|ui| {
        ui.label("generation");
        ui.monospace(run.node.generation.to_string());
    });
    ui.horizontal(|ui| {
        ui.label("child run forest nodes");
        ui.monospace(run.children.len().to_string());
    });
}

fn render_artifact_metrics(ui: &mut egui::Ui, artifact: &ArtifactInspection<'_>) {
    let metrics = artifact_metrics(&artifact.sources);
    ui.horizontal(|ui| {
        ui.label("source records");
        ui.monospace(metrics.source_records.to_string());
    });
    ui.horizontal(|ui| {
        ui.label("evidence refs");
        ui.monospace(metrics.evidence_refs.to_string());
    });
}

fn render_parent_create_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => render_parent_create(ui, run.parent_create),
        SelectionInspector::Artifact(artifact) => render_parent_create(ui, artifact.parent_create),
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_graph_edges_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => {
            render_edges(ui, "in", run_forest_incoming_edges(run));
            render_edges(ui, "out", run_forest_outgoing_edges(run));
        }
        SelectionInspector::Artifact(artifact) => {
            render_edges(ui, "in", artifact_edges(&artifact.incoming));
            render_edges(ui, "out", artifact_edges(&artifact.outgoing));
        }
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_artifact_edges_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => {
            render_edges(ui, "in", std::iter::empty());
            render_edges(ui, "out", run_forest_artifact_edges(run.node));
        }
        SelectionInspector::Artifact(artifact) => {
            render_edges(ui, "in", artifact_edges(&artifact.incoming));
            render_edges(ui, "out", artifact_edges(&artifact.outgoing));
        }
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_patches_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => render_patches(ui, run.patch.iter().copied()),
        SelectionInspector::Artifact(artifact) => {
            render_patches(ui, artifact.patches.iter().copied());
        }
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_source_refs_for_inspector(ui: &mut egui::Ui, inspector: &SelectionInspector<'_>) {
    match inspector {
        SelectionInspector::RunForestNode(run) => {
            render_source_refs(ui, run_forest_source_refs(run.node));
        }
        SelectionInspector::Artifact(artifact) => {
            render_source_refs(ui, artifact_source_refs(&artifact.sources));
        }
        SelectionInspector::Unresolved(reason) => render_unavailable(ui, *reason),
    }
}

fn render_unavailable(ui: &mut egui::Ui, reason: UnavailableReason) {
    kv(ui, reason.subject(), reason.state());
}

fn render_parent_create(ui: &mut egui::Ui, lookup: ParentCreateLookup<'_, '_>) {
    match lookup {
        ParentCreateLookup::Attempt(attempt) => render_parent_create_attempt(ui, attempt),
        ParentCreateLookup::Unavailable(reason) => {
            kv(ui, "attempt", "missing");
            render_parent_create_unavailable(ui, reason);
        }
        ParentCreateLookup::Ambiguous { count, reason } => {
            kv(ui, "attempt", "ambiguous");
            ui.horizontal(|ui| {
                ui.label("matches");
                ui.monospace(count.to_string());
            });
            render_parent_create_unavailable(ui, reason);
        }
    }
}

fn render_parent_create_attempt(ui: &mut egui::Ui, attempt: ParentCreateAttempt<'_>) {
    let child = attempt.child();
    let surface = attempt.surface();
    let summary = agent_turn_summary(attempt);
    let (surface_producer, router_model) = match attempt.surface_producer() {
        Some(ploke_records::history::SurfaceProposalProducerRecord::NonRouter) => {
            (Some("non_router"), None)
        }
        Some(ploke_records::history::SurfaceProposalProducerRecord::Router { request_policy }) => {
            (Some("router"), Some(request_policy.model.value.as_str()))
        }
        None => (None, None),
    };

    kv(ui, "attempt", "available");
    kv(
        ui,
        "target",
        child
            .request
            .target_relpath
            .to_str()
            .unwrap_or("non_utf8_path"),
    );
    if let Some(producer) = surface_producer {
        kv(ui, "surface", producer);
    }
    if let Some(touched_files) = surface.map(|surface| surface.touches.len()) {
        ui.horizontal(|ui| {
            ui.label("surface touches");
            ui.monospace(touched_files.to_string());
        });
    }
    if let Some(surface) = surface {
        kv(
            ui,
            "check/apply",
            &format!(
                "{}/{}",
                crate::ui::inspector::surface_check_status_label(surface.check_status),
                crate::ui::inspector::surface_apply_status_label(surface.apply_status)
            ),
        );
    }
    if let Some(model) = router_model {
        kv(ui, "model", model);
    } else if surface_producer == Some("non_router") {
        kv(ui, "model", "not_applicable");
    }
    ui.horizontal(|ui| {
        ui.label("tools");
        ui.monospace(format!(
            "{} requested, {} completed, {} failed",
            summary.tool_requested, summary.tool_completed, summary.tool_failed
        ));
    });
    ui.horizontal(|ui| {
        ui.label("llm proposal");
        ui.monospace(format!(
            "{} edits, {} creates, {} files",
            summary.edit_proposals, summary.create_proposals, summary.expected_file_changes
        ));
    });
    ui.horizontal(|ui| {
        ui.label("child eval");
        ui.monospace(format!(
            "{} evidence refs",
            attempt.candidate_evaluation_count()
        ));
    });

    egui::CollapsingHeader::new("LLM calls")
        .default_open(false)
        .show(ui, |ui| render_agent_turns(ui, attempt.agent_turns()));
    egui::CollapsingHeader::new("Source status")
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("child");
                id_display::expandable_id(
                    ui,
                    ("parent-create-child", child.node.node_id.as_str()),
                    child.node.node_id.as_str(),
                );
            });
            ui.horizontal(|ui| {
                ui.label("branch");
                id_display::expandable_id(
                    ui,
                    (
                        "parent-create-branch",
                        child.resolved.branch.branch_id.as_str(),
                    ),
                    child.resolved.branch.branch_id.as_str(),
                );
            });
            ui.horizontal(|ui| {
                ui.label("candidate");
                id_display::expandable_id(
                    ui,
                    (
                        "parent-create-candidate",
                        child.resolved.branch.candidate_id.as_str(),
                    ),
                    child.resolved.branch.candidate_id.as_str(),
                );
            });
            ui.horizontal(|ui| {
                ui.label("record refs");
                ui.monospace(attempt.source_ref_count().to_string());
            });
        });
}

fn render_parent_create_unavailable(
    ui: &mut egui::Ui,
    reason: ploke_tree::graph::ParentCreateUnavailable<'_>,
) {
    let (record, key, value) = match reason {
        ploke_tree::graph::ParentCreateUnavailable::MissingJoin { record, key, value }
        | ploke_tree::graph::ParentCreateUnavailable::AmbiguousJoin { record, key, value } => {
            (record, key, value)
        }
    };

    ui.horizontal(|ui| {
        ui.label(record);
        ui.monospace(key);
        id_display::expandable_id(ui, ("parent-create-unavailable", record, key, value), value);
    });
}

#[derive(Default)]
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

fn render_agent_turns<'a>(
    ui: &mut egui::Ui,
    turns: impl IntoIterator<Item = &'a AgentTurnArtifactMetadata>,
) {
    let mut rendered = false;
    for (index, turn) in turns.into_iter().enumerate() {
        rendered = true;
        egui::CollapsingHeader::new(format!("turn {}", index + 1))
            .default_open(index == 0)
            .show(ui, |ui| {
                kv(ui, "model", turn.selected_model.as_str());
                if let Some(outcome) = turn.terminal_outcome.as_deref() {
                    kv(ui, "outcome", outcome);
                }
                ui.horizontal(|ui| {
                    ui.label("events");
                    ui.monospace(turn.event_count.to_string());
                });
                ui.horizontal(|ui| {
                    ui.label("prompt messages");
                    ui.monospace(turn.llm_prompt_message_count.to_string());
                });
                ui.horizontal(|ui| {
                    ui.label("tools");
                    ui.monospace(format!(
                        "{} requested, {} completed, {} failed",
                        turn.tool_request_event_count,
                        turn.tool_completed_event_count,
                        turn.tool_failed_event_count
                    ));
                });
                ui.horizontal(|ui| {
                    ui.label("patch");
                    ui.monospace(if turn.patch_applied {
                        "applied"
                    } else {
                        "not_applied"
                    });
                });
                id_display::expandable_id(
                    ui,
                    ("agent-turn", turn.task_id.as_str()),
                    turn.task_id.as_str(),
                );
            });
    }
    if !rendered {
        kv(ui, "llm", "not_recorded");
    }
}

fn render_edges<'a>(
    ui: &mut egui::Ui,
    direction: &str,
    edges: impl IntoIterator<Item = SelectionEdge<'a>>,
) {
    let mut rendered = false;
    for edge in edges {
        rendered = true;
        ui.horizontal(|ui| {
            ui.label(direction);
            ui.monospace(edge.relation.label());
            id_display::expandable_id(
                ui,
                ("edge-from", direction, edge.relation.label(), edge.from),
                edge.from,
            );
            ui.label("->");
            id_display::expandable_id(
                ui,
                ("edge-to", direction, edge.relation.label(), edge.to),
                edge.to,
            );
            ui.monospace(format!("({})", edge.source_count));
        });
    }
    if !rendered {
        kv(ui, direction, "none");
    }
}

fn render_source_refs<'a>(ui: &mut egui::Ui, source_refs: impl IntoIterator<Item = SourceRef<'a>>) {
    let mut total = 0;
    for source_ref in source_refs {
        if total < 8 {
            render_source_ref(ui, &source_ref);
        }
        total += 1;
    }
    if total == 0 {
        kv(ui, "record refs", "none");
        return;
    }
    if total > 8 {
        kv(ui, "more", &format!("{}", total - 8));
    }
}

fn render_source_ref(ui: &mut egui::Ui, source_ref: &SourceRef<'_>) {
    match source_ref {
        SourceRef::Evidence {
            kind,
            authority,
            recorded_at,
        } => {
            ui.horizontal(|ui| {
                ui.monospace(*kind);
                ui.monospace(*authority);
                if let Some(recorded_at) = recorded_at {
                    id_display::expandable_id(
                        ui,
                        ("source-ref", kind, authority, recorded_at),
                        recorded_at,
                    );
                }
            });
        }
        SourceRef::Diagnostic { severity, code } => {
            ui.horizontal(|ui| {
                ui.monospace("diagnostic");
                ui.monospace(*severity);
                id_display::expandable_id(ui, ("source-ref", severity, code), code);
            });
        }
        SourceRef::ArtifactHistoryRef { artifact } => {
            ui.horizontal(|ui| {
                ui.monospace("artifact_history_ref");
                id_display::expandable_id(ui, ("source-ref", artifact), artifact);
            });
        }
        SourceRef::ArtifactId { artifact } => {
            ui.horizontal(|ui| {
                ui.monospace("artifact_id");
                id_display::expandable_id(ui, ("source-ref", artifact), artifact);
            });
        }
    }
}

fn render_patches<'a>(ui: &mut egui::Ui, patches: impl IntoIterator<Item = PatchInspection<'a>>) {
    let mut rendered = false;
    for patch in patches {
        rendered = true;
        render_patch(ui, patch);
    }
    if !rendered {
        kv(ui, "patch", "not_available");
    }
}

fn render_patch(ui: &mut egui::Ui, patch: PatchInspection<'_>) {
    ui.horizontal(|ui| {
        ui.label("patch");
        id_display::expandable_id(ui, ("patch", patch.patch_id()), patch.patch_id());
    });
    kv(ui, "target", patch.target_relpath());
    kv(ui, "branch", patch.branch_id());
    kv(ui, "candidate", patch.candidate_id());
    kv(ui, "source hash", patch.source_content_hash());
    kv(ui, "proposed hash", patch.proposed_content_hash());
    if let Some(base) = patch.base_artifact() {
        kv(ui, "base artifact", base);
    }
    if let Some(derived) = patch.derived_artifact() {
        kv(ui, "derived artifact", derived);
    }
    if let Some(check) = patch.check_status() {
        kv(ui, "check", surface_check_status_label(check));
    }
    if let Some(apply) = patch.apply_status() {
        kv(ui, "apply", surface_apply_status_label(apply));
    }

    let mut touched = false;
    for touch in patch.touches() {
        touched = true;
        ui.label(format!(
            "touch {} {}:{}-{}",
            touch.index, touch.relpath, touch.start, touch.end
        ));
        ui.add(egui::Label::new(egui::RichText::new(touch.replacement).monospace()).wrap());
    }
    if !touched {
        kv(ui, "touches", "none");
    }
    let diff = patch.unified_diff();
    if diff.is_empty() {
        kv(ui, "diff", "not_available");
    } else {
        ui.label("diff");
        render_diff(ui, diff.as_str());
    }
}

fn render_diff(ui: &mut egui::Ui, diff: &str) {
    let width = ui.available_width().max(240.0);
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .max_height(320.0)
                .show(ui, |ui| {
                    let job = diff::highlighted_diff_job(ui, diff, f32::INFINITY);
                    ui.set_min_width(width);
                    ui.add(egui::Label::new(job).selectable(true));
                });
        });
}
