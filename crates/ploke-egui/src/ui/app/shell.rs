//! Stable frame rendering for the operator graph UI.

use eframe::egui;

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

pub(crate) fn render_right_inspector(ui: &mut egui::Ui, selection: Option<&GraphSelectionDetail>) {
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
            if let Some(selection) = selection {
                ui.monospace(selection.detail.as_str());
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

fn selection_status(selection: Option<&GraphSelectionDetail>) -> &'static str {
    if selection.is_some() {
        "available"
    } else {
        "not_applicable"
    }
}
