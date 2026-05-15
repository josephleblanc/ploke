//! Stable frame rendering for the operator graph UI.

use eframe::egui;

use crate::ui::diff;
use crate::ui::id_display;
use crate::ui::inspector::{
    InspectorEdge, InspectorMetric, InspectorRow, PatchSnapshot, SelectionInspectorSnapshot,
    SourceRef,
};
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
                render_fields(ui, &inspector.roles);
                render_metrics(ui, &inspector.metrics);
            } else {
                kv(ui, "roles", "not_applicable");
            }

            ui.separator();
            ui.label("Graph edges");
            if let Some(inspector) = inspector {
                render_edges(ui, "in", &inspector.incoming);
                render_edges(ui, "out", &inspector.outgoing);
            } else {
                kv(ui, "edges", "not_applicable");
            }

            ui.separator();
            ui.label("Artifact edges");
            if let Some(inspector) = inspector {
                render_edges(ui, "in", &inspector.artifact_incoming);
                render_edges(ui, "out", &inspector.artifact_outgoing);
            } else {
                kv(ui, "artifact edges", "not_applicable");
            }

            ui.separator();
            ui.label("Patch");
            if let Some(inspector) = inspector {
                render_patches(ui, &inspector.patches);
            } else {
                kv(ui, "patch", "not_applicable");
            }

            ui.separator();
            ui.label("Source refs");
            if let Some(inspector) = inspector {
                render_source_refs(ui, &inspector.source_refs);
                render_fields(ui, &inspector.unavailable);
            } else {
                kv(ui, "record refs", "not_applicable");
            }

            ui.separator();
            ui.label("Drill next");
            kv(ui, "lineage", selection_status(selection));
            kv(ui, "candidate set", "blocked");
            kv(ui, "patch diff", patch_status(inspector));
            kv(ui, "evidence refs", "blocked");
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

fn render_metrics(ui: &mut egui::Ui, metrics: &[InspectorMetric]) {
    for metric in metrics {
        ui.horizontal(|ui| {
            ui.label(metric.label);
            ui.monospace(metric.value.to_string());
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

fn selection_status(selection: Option<&GraphSelectionDetail>) -> &'static str {
    if selection.is_some() {
        "available"
    } else {
        "not_applicable"
    }
}

fn patch_status(inspector: Option<&SelectionInspectorSnapshot<'_>>) -> &'static str {
    match inspector {
        Some(inspector) if !inspector.patches.is_empty() => "available",
        Some(_) => "not_available",
        None => "not_applicable",
    }
}
