//! Stable frame rendering for the operator graph UI.

use eframe::egui;

use crate::ui::diff;
use crate::ui::id_display;
use crate::ui::inspector::{
    AgentTurnSnapshot, InspectorEdge, InspectorMetric, InspectorRow, ParentCreateSnapshot,
    ParentCreateState, PatchSnapshot, SelectionInspectorSnapshot, SourceRef,
};
use crate::ui::text::decor::Badge;
use crate::ui::view::{GraphSelectionDetail, GraphViewDiagnostics, GraphViewMode};

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
    inspector: Option<&SelectionInspectorSnapshot<'_>>,
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
                render_fields(ui, &inspector.identity);
            } else {
                kv(ui, "record refs", "not_applicable");
            }

            ui.separator();
            ui.label("Roles");
            if let Some(inspector) = inspector {
                render_badges(ui, &inspector.roles);
                render_metrics(ui, &inspector.metrics);
            } else {
                kv(ui, "roles", "not_applicable");
            }

            ui.separator();
            ui.label("Patch Generation");
            if let Some(inspector) = inspector {
                render_parent_create(ui, inspector.parent_create.as_ref());
            } else {
                kv(ui, "attempt", "not_applicable");
            }

            ui.separator();
            egui::CollapsingHeader::new("Graph edges")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_edges(ui, "in", &inspector.incoming);
                        render_edges(ui, "out", &inspector.outgoing);
                    } else {
                        kv(ui, "edges", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Artifact edges")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_edges(ui, "in", &inspector.artifact_incoming);
                        render_edges(ui, "out", &inspector.artifact_outgoing);
                    } else {
                        kv(ui, "artifact edges", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Patch Debug")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_patches(ui, &inspector.patches);
                    } else {
                        kv(ui, "patch", "not_applicable");
                    }
                });

            ui.separator();
            egui::CollapsingHeader::new("Source refs")
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(inspector) = inspector {
                        render_source_refs(ui, &inspector.source_refs);
                        render_fields(ui, &inspector.unavailable);
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

fn render_fields(ui: &mut egui::Ui, fields: &[InspectorRow]) {
    if fields.is_empty() {
        kv(ui, "none", "not_applicable");
        return;
    }
    for field in fields {
        kv(ui, field.label, field.value);
    }
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

fn render_metrics(ui: &mut egui::Ui, metrics: &[InspectorMetric]) {
    for metric in metrics {
        ui.horizontal(|ui| {
            ui.label(metric.label);
            ui.monospace(metric.value.to_string());
        });
    }
}

fn render_parent_create(ui: &mut egui::Ui, parent_create: Option<&ParentCreateSnapshot<'_>>) {
    let Some(parent_create) = parent_create else {
        kv(ui, "attempt", "none");
        return;
    };

    match parent_create.state {
        ParentCreateState::Available => render_parent_create_available(ui, parent_create),
        ParentCreateState::Missing => {
            kv(ui, "attempt", "missing");
            render_parent_create_unavailable(ui, parent_create);
        }
        ParentCreateState::Ambiguous => {
            kv(ui, "attempt", "ambiguous");
            if let Some(count) = parent_create.ambiguous_count {
                ui.horizontal(|ui| {
                    ui.label("matches");
                    ui.monospace(count.to_string());
                });
            }
            render_parent_create_unavailable(ui, parent_create);
        }
    }
}

fn render_parent_create_available(ui: &mut egui::Ui, parent_create: &ParentCreateSnapshot<'_>) {
    kv(ui, "attempt", "available");
    if let Some(target) = parent_create.target_relpath {
        kv(ui, "target", target);
    }
    if let Some(producer) = parent_create.surface_producer {
        kv(ui, "surface", producer);
    }
    if let Some(touched_files) = parent_create.touched_files {
        ui.horizontal(|ui| {
            ui.label("surface touches");
            ui.monospace(touched_files.to_string());
        });
    }
    if let (Some(check), Some(apply)) = (parent_create.surface_check, parent_create.surface_apply) {
        kv(ui, "check/apply", &format!("{check}/{apply}"));
    }
    if let Some(model) = parent_create.router_model {
        kv(ui, "model", model);
    } else if parent_create.surface_producer == Some("non_router") {
        kv(ui, "model", "not_applicable");
    }
    ui.horizontal(|ui| {
        ui.label("tools");
        ui.monospace(format!(
            "{} requested, {} completed, {} failed",
            parent_create.tool_requested, parent_create.tool_completed, parent_create.tool_failed
        ));
    });
    ui.horizontal(|ui| {
        ui.label("llm proposal");
        ui.monospace(format!(
            "{} edits, {} creates, {} files",
            parent_create.edit_proposals,
            parent_create.create_proposals,
            parent_create.expected_file_changes
        ));
    });
    ui.horizontal(|ui| {
        ui.label("child eval");
        ui.monospace(format!(
            "{} evidence refs",
            parent_create.candidate_evaluation_count
        ));
    });

    egui::CollapsingHeader::new("LLM calls")
        .default_open(false)
        .show(ui, |ui| render_agent_turns(ui, &parent_create.agent_turns));
    egui::CollapsingHeader::new("Source status")
        .default_open(false)
        .show(ui, |ui| {
            if let Some(child_node_id) = parent_create.child_node_id {
                ui.horizontal(|ui| {
                    ui.label("child");
                    id_display::expandable_id(
                        ui,
                        ("parent-create-child", child_node_id),
                        child_node_id,
                    );
                });
            }
            if let Some(branch_id) = parent_create.branch_id {
                ui.horizontal(|ui| {
                    ui.label("branch");
                    id_display::expandable_id(ui, ("parent-create-branch", branch_id), branch_id);
                });
            }
            if let Some(candidate_id) = parent_create.candidate_id {
                ui.horizontal(|ui| {
                    ui.label("candidate");
                    id_display::expandable_id(
                        ui,
                        ("parent-create-candidate", candidate_id),
                        candidate_id,
                    );
                });
            }
            ui.horizontal(|ui| {
                ui.label("record refs");
                ui.monospace(parent_create.source_ref_count.to_string());
            });
        });
}

fn render_parent_create_unavailable(ui: &mut egui::Ui, parent_create: &ParentCreateSnapshot<'_>) {
    if let Some(reason) = parent_create.unavailable {
        ui.horizontal(|ui| {
            ui.label(reason.record);
            ui.monospace(reason.key);
            id_display::expandable_id(
                ui,
                (
                    "parent-create-unavailable",
                    reason.record,
                    reason.key,
                    reason.value,
                ),
                reason.value,
            );
        });
    }
}

fn render_agent_turns(ui: &mut egui::Ui, turns: &[AgentTurnSnapshot<'_>]) {
    if turns.is_empty() {
        kv(ui, "llm", "not_recorded");
        return;
    }
    for (index, turn) in turns.iter().enumerate() {
        egui::CollapsingHeader::new(format!("turn {}", index + 1))
            .default_open(index == 0)
            .show(ui, |ui| {
                kv(ui, "model", turn.selected_model);
                if let Some(outcome) = turn.terminal_outcome {
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
                        turn.tool_requested, turn.tool_completed, turn.tool_failed
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
                id_display::expandable_id(ui, ("agent-turn", turn.task_id), turn.task_id);
            });
    }
}

fn render_edges(ui: &mut egui::Ui, direction: &str, edges: &[InspectorEdge<'_>]) {
    if edges.is_empty() {
        kv(ui, direction, "none");
        return;
    }
    for edge in edges {
        ui.horizontal(|ui| {
            ui.label(direction);
            ui.monospace(edge.relation);
            id_display::expandable_id(
                ui,
                ("edge-from", direction, edge.relation, edge.from),
                edge.from,
            );
            ui.label("->");
            id_display::expandable_id(ui, ("edge-to", direction, edge.relation, edge.to), edge.to);
            ui.monospace(format!("({})", edge.source_count));
        });
    }
}

fn render_source_refs(ui: &mut egui::Ui, source_refs: &[SourceRef<'_>]) {
    if source_refs.is_empty() {
        kv(ui, "record refs", "none");
        return;
    }
    for source_ref in source_refs.iter().take(8) {
        render_source_ref(ui, source_ref);
    }
    if source_refs.len() > 8 {
        kv(ui, "more", &format!("{}", source_refs.len() - 8));
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
        SourceRef::ArtifactEvidenceCount { count } => {
            ui.horizontal(|ui| {
                ui.monospace("artifact_evidence_count");
                ui.monospace(count.to_string());
            });
        }
    }
}

fn render_patches(ui: &mut egui::Ui, patches: &[PatchSnapshot]) {
    if patches.is_empty() {
        kv(ui, "patch", "not_available");
        return;
    }
    for patch in patches {
        ui.horizontal(|ui| {
            ui.label("patch");
            id_display::expandable_id(ui, ("patch", patch.patch_id), patch.patch_id);
        });
        render_fields(ui, &patch.summary);
        if patch.touches.is_empty() {
            kv(ui, "touches", "none");
        } else {
            for touch in &patch.touches {
                ui.label(format!(
                    "touch {} {}:{}-{}",
                    touch.index, touch.relpath, touch.start, touch.end
                ));
                ui.add(egui::Label::new(egui::RichText::new(touch.replacement).monospace()).wrap());
            }
        }
        let diff = patch.unified_diff();
        if diff.is_empty() {
            kv(ui, "diff", "not_available");
        } else {
            ui.label("diff");
            render_diff(ui, diff.as_str());
        }
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
