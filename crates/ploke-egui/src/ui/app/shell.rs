//! Stable frame rendering for the operator graph UI.

use eframe::egui;

use crate::ui::inspector::{InspectorEdge, InspectorRow, SelectionInspectorSnapshot};
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
    inspector: Option<&SelectionInspectorSnapshot>,
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
            kv(ui, "patch diff", "blocked");
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
        ui.monospace(value);
    });
}

fn render_fields(ui: &mut egui::Ui, fields: &[InspectorRow]) {
    if fields.is_empty() {
        kv(ui, "none", "not_applicable");
        return;
    }
    for field in fields {
        kv(ui, field.label.as_str(), field.value.as_str());
    }
}

fn render_edges(ui: &mut egui::Ui, direction: &str, edges: &[InspectorEdge]) {
    if edges.is_empty() {
        kv(ui, direction, "none");
        return;
    }
    for edge in edges {
        kv(
            ui,
            direction,
            format!(
                "{} {} -> {} ({})",
                edge.relation, edge.from, edge.to, edge.source_count
            )
            .as_str(),
        );
    }
}

fn render_source_refs(ui: &mut egui::Ui, source_refs: &[String]) {
    if source_refs.is_empty() {
        kv(ui, "record refs", "none");
        return;
    }
    for source_ref in source_refs.iter().take(8) {
        ui.monospace(source_ref.as_str());
    }
    if source_refs.len() > 8 {
        kv(ui, "more", &format!("{}", source_refs.len() - 8));
    }
}

fn selection_status(selection: Option<&GraphSelectionDetail>) -> &'static str {
    if selection.is_some() {
        "available"
    } else {
        "not_applicable"
    }
}
