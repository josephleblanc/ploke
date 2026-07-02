use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};
use ploke_eval::walk_client::{DbQueryResult, WalkRunEntry, WalkSnapshot};

use crate::model::WalkRequestKind;

pub(in crate::app) struct DetailsPanel<'a> {
    pub(in crate::app) runs: &'a [WalkRunEntry],
    pub(in crate::app) selected_run: Option<usize>,
    pub(in crate::app) run_error: Option<&'a str>,
    pub(in crate::app) client_available: bool,
    pub(in crate::app) walk_pending: Option<WalkRequestKind>,
    pub(in crate::app) snapshot: Option<&'a WalkSnapshot>,
    pub(in crate::app) notice: Option<&'a str>,
    pub(in crate::app) query_result: Option<&'a DbQueryResult>,
    pub(in crate::app) selected_row: Option<usize>,
    pub(in crate::app) run_details: &'a mut bool,
}

#[derive(Debug, Default)]
pub(in crate::app) struct DetailsAction {
    pub(in crate::app) refresh_health: bool,
    pub(in crate::app) show_state: bool,
}

impl DetailsPanel<'_> {
    pub(in crate::app) fn show(self, ui: &mut Ui) -> DetailsAction {
        let mut action = DetailsAction::default();
        ui.horizontal_top(|ui| {
            ui.heading("Run");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("Details").clicked() {
                    *self.run_details = !*self.run_details;
                }
            });
        });

        if *self.run_details {
            ui.add_space(6.0);
            if let Some(run) = self.selected_run.and_then(|index| self.runs.get(index)) {
                ui.label(&run.campaign_id);
                ui.label(format!("campaign: {}", run.campaign_dir.display()));
                ui.label(format!("prototype1: {}", run.prototype1_root.display()));
                if let Some(worktree) = run.worktree_root.as_deref() {
                    ui.label(format!("worktree: {}", worktree.display()));
                } else {
                    ui.label(RichText::new("worktree: missing").color(Color32::YELLOW));
                }
                ui.label(format!(
                    "db: {}",
                    if run.has_owner_db {
                        "present"
                    } else {
                        "missing"
                    }
                ));
                ui.label(format!(
                    "parent identity: {}",
                    if run.has_parent_identity {
                        "present"
                    } else {
                        "missing"
                    }
                ));
            } else {
                ui.label("No run selected");
            }
            if self.runs.is_empty() {
                ui.label("No Prototype 1 runs found under the ploke-eval home");
            }
            if let Some(error) = self.run_error {
                ui.label(RichText::new(error).color(Color32::LIGHT_RED));
            }
        }
        ui.separator();

        ui.horizontal_wrapped(|ui| {
            ui.horizontal_top(|ui| {
                ui.heading("Walk");
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                let walk_idle = self.walk_pending.is_none();
                if ui
                    .add_enabled(walk_idle, egui::Button::new("Health"))
                    .clicked()
                {
                    action.refresh_health = true;
                }
                let show_response = ui.add_enabled(
                    self.client_available && walk_idle,
                    egui::Button::new("Show"),
                );
                if show_response.clicked() {
                    action.show_state = true;
                }
                if let Some(kind) = self.walk_pending {
                    ui.label(RichText::new(format!("{}...", kind.label())).color(Color32::YELLOW));
                }
            });
        });

        ui.add_space(6.0);
        if let Some(snapshot) = self.snapshot {
            ui.label(format!(
                "phase: {}",
                snapshot.phase_id.as_deref().unwrap_or("unknown")
            ));
            if let Some(label) = snapshot.phase_label.as_deref() {
                ui.label(label);
            }
            ui.label(format!("protocol: {}", snapshot.epoch.protocol_version));
            ui.label(format!(
                "graph: {}",
                snapshot.epoch.transition_graph_version
            ));
            if let Some(head) = snapshot.epoch.git_head.as_deref() {
                ui.label(format!("git: {}", short_hash(head)));
            }
            ui.add_space(8.0);
            ScrollArea::vertical()
                .id_salt("walk_message")
                .max_height(180.0)
                .show(ui, |ui| {
                    ui.monospace(&snapshot.message);
                });
        } else {
            ui.label("No walk snapshot");
        }
        ui.separator();
        if let Some(notice) = self.notice {
            ui.label(notice);
        }
        ui.separator();
        if let (Some(result), Some(row)) = (&self.query_result, self.selected_row)
            && let Some(row) = result.rows.get(row)
        {
            ui.label("row");
            let text = serde_json::to_string_pretty(&row.object)
                .unwrap_or_else(|_| row.object.to_string());
            ScrollArea::vertical()
                .id_salt("row_detail")
                .show(ui, |ui| ui.monospace(text));
        }
        action
    }
}

fn short_hash(text: &str) -> &str {
    text.get(..12).unwrap_or(text)
}
