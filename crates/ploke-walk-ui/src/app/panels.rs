use eframe::egui::{self, Color32, ComboBox, Grid, RichText, ScrollArea, TextEdit, TextStyle, Ui};
use ploke_eval::walk_client::{DbQueryResult, PhaseInfo, WalkPhase, WalkRunEntry};

use crate::model::{
    DEFAULT_QUERY, MAX_TABLE_ROWS, RUN_LABEL_MAX_CHARS, ServiceStatus, WalkRequestKind,
};

use super::WalkUiApp;

impl WalkUiApp {
    pub(super) fn top_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(status_text(&self.status));
            if let ServiceStatus::Error(error) = &self.status {
                ui.label(RichText::new(error).color(Color32::LIGHT_RED));
            }
            ui.separator();
            ui.label("run");
            let mut selected = self.selected_run;
            ComboBox::from_id_salt("ploke-walk-ui.run-picker")
                .width(440.0)
                .selected_text(selected_run_label(self.selected_run()))
                .show_ui(ui, |ui| {
                    for (index, run) in self.runs.iter().enumerate() {
                        ui.selectable_value(&mut selected, Some(index), run_menu_label(run));
                    }
                });
            if selected != self.selected_run
                && let Some(index) = selected
            {
                self.select_run(index);
            }
            if ui.button("Refresh Runs").clicked() {
                self.refresh_runs();
                self.refresh_health(Some(ui.ctx().clone()));
            }
            ui.separator();
            ui.label("socket");
            ui.add_sized([260.0, 22.0], TextEdit::singleline(&mut self.socket_input));
            ui.toggle_value(&mut self.debug_panel, "Debug");
        });
    }

    pub(super) fn phase_rail(&mut self, ui: &mut Ui) {
        ui.heading("Phases");
        ui.add_space(6.0);
        let current = self.current_phase();
        ScrollArea::vertical().show(ui, |ui| {
            for phase in &self.phases.phases {
                phase_row(ui, phase, current);
            }
        });
    }

    pub(super) fn query_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("campaign");
            ui.add_sized(
                [320.0, 22.0],
                TextEdit::singleline(&mut self.campaign_input),
            );
            if ui.button("Use Selected").clicked()
                && let Some(campaign) = self.selected_campaign_id()
            {
                self.campaign_input = campaign.to_string();
            }
            if ui
                .add_enabled(!self.query_pending, egui::Button::new("Run Query"))
                .clicked()
            {
                self.run_query(Some(ui.ctx().clone()));
            }
            if ui.button("Relations").clicked() {
                self.query_script = DEFAULT_QUERY.to_string();
            }
            if self.query_pending {
                ui.label(RichText::new("query...").color(Color32::YELLOW));
            }
        });
        ui.add_space(8.0);
        ui.add_sized(
            [ui.available_width(), 150.0],
            TextEdit::multiline(&mut self.query_script)
                .font(TextStyle::Monospace)
                .desired_rows(7),
        );
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
        match self.query_result.as_ref() {
            Some(result) => query_result_table(ui, result, &mut self.selected_row),
            None => {
                ui.label(RichText::new("No query result").color(Color32::GRAY));
            }
        }
    }

    pub(super) fn details_panel(&mut self, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            ui.heading("Run");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("Details").clicked() {
                    self.buttons.run_details = !self.buttons.run_details;
                }
            });
        });

        if self.buttons.run_details {
            ui.add_space(6.0);
            if let Some(run) = self.selected_run() {
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
            if let Some(error) = self.run_error.as_deref() {
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
                    self.refresh_health(Some(ui.ctx().clone()));
                }
                let show_response = ui.add_enabled(
                    self.client.is_some() && walk_idle,
                    egui::Button::new("Show"),
                );
                if show_response.clicked() {
                    self.show_state(Some(ui.ctx().clone()));
                }
                if let Some(kind) = self.walk_pending {
                    ui.label(RichText::new(format!("{}...", kind.label())).color(Color32::YELLOW));
                }
            });
        });

        ui.add_space(6.0);
        if let Some(snapshot) = &self.snapshot {
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
        if let Some(notice) = self.notice.as_deref() {
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
    }

    pub(super) fn debug_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("Debug")
            .id(egui::Id::new("ploke-walk-ui.debug-window"))
            .default_width(460.0)
            .show(ctx, |ui| {
                if ui
                    .checkbox(&mut self.debug_hover, "widget info on hover")
                    .changed()
                {
                    ctx.set_debug_on_hover(self.debug_hover);
                }
                ui.separator();
                Grid::new("debug_state_grid").striped(true).show(ui, |ui| {
                    ui.label("status");
                    ui.label(status_plain(&self.status));
                    ui.end_row();
                    ui.label("selected_run");
                    ui.label(self.selected_campaign_id().unwrap_or("none").to_string());
                    ui.end_row();
                    ui.label("client_socket");
                    ui.label(
                        self.client
                            .as_ref()
                            .map(|client| client.socket().display().to_string())
                            .unwrap_or_else(|| "none".to_string()),
                    );
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
                    ui.label(self.runs.len().to_string());
                    ui.end_row();
                    ui.label("rows");
                    ui.label(
                        self.query_result
                            .as_ref()
                            .map(|result| result.row_count.to_string())
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

    fn current_phase(&self) -> Option<WalkPhase> {
        self.snapshot.as_ref().and_then(|snapshot| snapshot.phase)
    }
}

fn phase_row(ui: &mut Ui, phase: &PhaseInfo, current: Option<WalkPhase>) {
    let is_current = current == Some(phase.phase);
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
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&phase.id).monospace().strong());
                ui.label(&phase.label);
            });
            for next in &phase.next {
                ui.label(
                    RichText::new(format!("-> {} ({})", next.phase_id, next.edge))
                        .small()
                        .color(Color32::LIGHT_GRAY),
                );
            }
        });
    ui.add_space(5.0);
}

fn query_result_table(ui: &mut Ui, result: &DbQueryResult, selected_row: &mut Option<usize>) {
    ui.horizontal(|ui| {
        ui.label(format!("rows: {}", result.row_count));
        ui.separator();
        ui.label(result.db_path.display().to_string());
    });
    ui.add_space(6.0);
    if result.headers.is_empty() {
        ui.label("No headers");
        return;
    }
    ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            Grid::new("query_result_grid")
                .striped(true)
                .min_col_width(120.0)
                .show(ui, |ui| {
                    ui.label(RichText::new("#").strong());
                    for header in &result.headers {
                        ui.label(RichText::new(header).strong());
                    }
                    ui.end_row();
                    for (index, row) in result.rows.iter().take(MAX_TABLE_ROWS).enumerate() {
                        let selected = *selected_row == Some(index);
                        if ui.selectable_label(selected, index.to_string()).clicked() {
                            *selected_row = Some(index);
                        }
                        for cell in &row.cells {
                            ui.label(format_cell(cell));
                        }
                        ui.end_row();
                    }
                });
        });
}

fn status_text(status: &ServiceStatus) -> RichText {
    match status {
        ServiceStatus::Unresolved => RichText::new("unresolved").color(Color32::GRAY),
        ServiceStatus::Offline => RichText::new("offline").color(Color32::YELLOW),
        ServiceStatus::Online => RichText::new("online").color(Color32::GREEN),
        ServiceStatus::Busy(_) => RichText::new("busy").color(Color32::YELLOW),
        ServiceStatus::Error(_) => RichText::new("error").color(Color32::RED),
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

fn selected_run_label(run: Option<&WalkRunEntry>) -> String {
    run.map(|run| elide_middle(&run.campaign_id, RUN_LABEL_MAX_CHARS))
        .unwrap_or_else(|| "Select run".to_string())
}

fn run_menu_label(run: &WalkRunEntry) -> String {
    let mut flags = Vec::new();
    flags.push(if run.has_owner_db { "db" } else { "no db" });
    flags.push(if run.worktree_root.is_some() {
        "worktree"
    } else {
        "no worktree"
    });
    flags.push(if run.has_parent_identity {
        "identity"
    } else {
        "no identity"
    });
    format!(
        "{}  |  {}",
        elide_middle(&run.campaign_id, RUN_LABEL_MAX_CHARS),
        flags.join(", ")
    )
}

fn format_cell(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn short_hash(text: &str) -> &str {
    text.get(..12).unwrap_or(text)
}

fn elide_middle(value: &str, max_chars: usize) -> String {
    let len = value.chars().count();
    if len <= max_chars || max_chars < 5 {
        return value.to_owned();
    }

    let prefix_len = (max_chars - 1) / 2;
    let suffix_len = max_chars - prefix_len - 1;
    let prefix: String = value.chars().take(prefix_len).collect();
    let suffix: String = value
        .chars()
        .rev()
        .take(suffix_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}.{suffix}")
}
