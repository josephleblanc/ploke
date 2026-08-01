use eframe::egui::{self, Grid, ScrollArea};

use crate::model::{ServiceStatus, WalkRequestKind};

pub(in crate::app) struct DebugWindow<'a> {
    pub(in crate::app) debug_hover: &'a mut bool,
    pub(in crate::app) status: &'a ServiceStatus,
    pub(in crate::app) selected_campaign: Option<&'a str>,
    pub(in crate::app) client_socket: Option<&'a str>,
    pub(in crate::app) walk_pending: Option<WalkRequestKind>,
    pub(in crate::app) query_pending: bool,
    pub(in crate::app) runs_len: usize,
    pub(in crate::app) row_count: Option<usize>,
}

impl DebugWindow<'_> {
    pub(in crate::app) fn show(self, ctx: &egui::Context) {
        egui::Window::new("Debug")
            .id(egui::Id::new("ploke-walk-ui.debug-window"))
            .default_width(460.0)
            .show(ctx, |ui| {
                if ui
                    .checkbox(self.debug_hover, "widget info on hover")
                    .changed()
                {
                    ctx.set_debug_on_hover(*self.debug_hover);
                }
                ui.separator();
                Grid::new("debug_state_grid").striped(true).show(ui, |ui| {
                    ui.label("status");
                    ui.label(status_plain(self.status));
                    ui.end_row();
                    ui.label("selected_run");
                    ui.label(self.selected_campaign.unwrap_or("none").to_string());
                    ui.end_row();
                    ui.label("client_socket");
                    ui.label(self.client_socket.unwrap_or("none").to_string());
                    ui.end_row();
                    ui.label("walk_pending");
                    ui.label(
                        self.walk_pending
                            .map(WalkRequestKind::label)
                            .unwrap_or("none"),
                    );
                    ui.end_row();
                    ui.label("query_pending");
                    ui.label(self.query_pending.to_string());
                    ui.end_row();
                    ui.label("runs");
                    ui.label(self.runs_len.to_string());
                    ui.end_row();
                    ui.label("rows");
                    ui.label(
                        self.row_count
                            .map(|count| count.to_string())
                            .unwrap_or_else(|| "none".to_string()),
                    );
                    ui.end_row();
                });
                ui.separator();
                ScrollArea::vertical()
                    .id_salt("egui_debug_scroll")
                    .max_height(420.0)
                    .show(ui, |ui| {
                        ui.heading("Inspection");
                        ctx.inspection_ui(ui);
                        ui.separator();
                        ui.heading("Settings");
                        ctx.settings_ui(ui);
                        ui.separator();
                        ui.heading("Memory");
                        ctx.memory_ui(ui);
                    });
            });
    }
}

fn status_plain(status: &ServiceStatus) -> String {
    match status {
        ServiceStatus::Unresolved => "unresolved".to_string(),
        ServiceStatus::Offline => "offline".to_string(),
        ServiceStatus::Online => "online".to_string(),
        ServiceStatus::Busy(detail) => format!("busy: {detail}"),
        ServiceStatus::Error(error) => format!("error: {error}"),
    }
}
