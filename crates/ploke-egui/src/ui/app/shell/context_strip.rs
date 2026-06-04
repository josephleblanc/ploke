//! Run-level evidence context: snapshot source, host, and decode capability.

use eframe::egui;

use crate::ui::provenance::{
    EvidenceLane, decode_capability_label, detail_for_snapshot_load, host_label,
    render_evidence_lane_chip_with_inspect,
};

pub(crate) fn render_run_evidence_context_strip(
    ui: &mut egui::Ui,
    source_label: Option<&str>,
    catalog_error: Option<&str>,
) {
    ui.vertical(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Source:").strong());
            ui.monospace(source_label.unwrap_or("export snapshot"));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Host:").strong());
            ui.monospace(host_label());
            ui.label(egui::RichText::new("Decode:").strong());
            ui.monospace(decode_capability_label());
            if let Some(error) = catalog_error {
                let detail = detail_for_snapshot_load(error, source_label);
                render_evidence_lane_chip_with_inspect(
                    ui,
                    EvidenceLane::UiSnapshotLoad,
                    detail,
                    ("context-strip-snapshot-load", error),
                );
            }
        });
    });
    if let Some(error) = catalog_error {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            let detail = detail_for_snapshot_load(error, source_label);
            render_evidence_lane_chip_with_inspect(
                ui,
                EvidenceLane::UiSnapshotLoad,
                detail,
                ("context-strip-snapshot-banner", error),
            );
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
                let detail = detail_for_snapshot_load(catalog_error, None);
                render_evidence_lane_chip_with_inspect(
                    ui,
                    EvidenceLane::UiSnapshotLoad,
                    detail,
                    ("snapshot-load-banner", catalog_error),
                );
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    format!("UI snapshot load failed: {catalog_error}"),
                );
            });
        });
}
