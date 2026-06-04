//! Run-level evidence context: snapshot source, host, and decode capability.

use eframe::egui;

use crate::ui::provenance::{
    EvidenceLane, decode_capability_label, host_label, render_evidence_lane_chip,
};

pub(crate) fn render_run_evidence_context_strip(
    ui: &mut egui::Ui,
    source_label: Option<&str>,
    catalog_error: Option<&str>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Source:").strong());
        ui.monospace(source_label.unwrap_or("export snapshot"));
        ui.separator();
        ui.label(egui::RichText::new("Host:").strong());
        ui.monospace(host_label());
        ui.separator();
        ui.label(egui::RichText::new("Decode:").strong());
        ui.monospace(decode_capability_label());
        if catalog_error.is_some() {
            ui.separator();
            render_evidence_lane_chip(ui, EvidenceLane::UiSnapshotLoad);
        }
    });
    if let Some(error) = catalog_error {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            render_evidence_lane_chip(ui, EvidenceLane::UiSnapshotLoad);
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!("UI snapshot load failed: {error}"),
            );
        });
    }
    ui.separator();
}

pub(crate) fn render_snapshot_load_banner(ui: &mut egui::Ui, catalog_error: &str) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgba_unmultiplied(80, 20, 20, 40))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                render_evidence_lane_chip(ui, EvidenceLane::UiSnapshotLoad);
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    format!("UI snapshot load failed: {catalog_error}"),
                );
            });
        });
}
