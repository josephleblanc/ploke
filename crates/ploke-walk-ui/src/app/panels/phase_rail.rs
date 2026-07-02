use eframe::egui::{self, Color32, Response, RichText, ScrollArea, Ui, Widget};
use ploke_eval::walk_client::{PhaseInfo, PhaseInventory, WalkPhase};

pub(in crate::app) struct PhaseRail<'a> {
    pub(in crate::app) phases: &'a PhaseInventory,
    pub(in crate::app) current: Option<WalkPhase>,
}

impl PhaseRail<'_> {
    pub(in crate::app) fn show(self, ui: &mut Ui) {
        ui.heading("Phases");
        ui.add_space(6.0);
        ScrollArea::vertical().show(ui, |ui| {
            for phase in &self.phases.phases {
                ui.add(PhaseRow {
                    phase,
                    current: self.current,
                });
            }
        });
    }
}

struct PhaseRow<'a> {
    phase: &'a PhaseInfo,
    current: Option<WalkPhase>,
}

impl Widget for PhaseRow<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let is_current = self.current == Some(self.phase.phase);
        let fill = if is_current {
            Color32::from_rgb(35, 74, 92)
        } else {
            Color32::from_rgb(31, 31, 34)
        };
        let stroke = if is_current {
            Color32::from_rgb(113, 180, 166)
        } else {
            Color32::from_rgb(62, 62, 66)
        };
        let response = egui::Frame::new()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, stroke))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.phase.id).monospace().strong());
                    ui.label(&self.phase.label);
                });
                for next in &self.phase.next {
                    ui.label(
                        RichText::new(format!("-> {} ({})", next.phase_id, next.edge))
                            .small()
                            .color(Color32::LIGHT_GRAY),
                    );
                }
            })
            .response;
        ui.add_space(5.0);
        response
    }
}
