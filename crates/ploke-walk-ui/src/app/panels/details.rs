use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};
use ploke_eval::walk_client::{WalkQuerySnapshot, WalkResponse, WalkRunEntry};

use crate::model::WalkRequestKind;

pub(in crate::app) struct DetailsPanel<'a> {
    pub(in crate::app) runs: &'a [WalkRunEntry],
    pub(in crate::app) selected_run: Option<usize>,
    pub(in crate::app) run_error: Option<&'a str>,
    pub(in crate::app) client_available: bool,
    pub(in crate::app) walk_pending: Option<WalkRequestKind>,
    pub(in crate::app) response: Option<&'a WalkResponse>,
    pub(in crate::app) notice: Option<&'a str>,
    pub(in crate::app) query_result: Option<&'a WalkQuerySnapshot>,
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
        if let Some(response) = self.response {
            let phase = response.phase();
            let epoch = response.epoch();
            ui.label(format!(
                "phase: {}",
                phase.map_or("unknown", |phase| phase.as_str())
            ));
            if let Some(phase) = phase {
                ui.label(phase.detail());
            }
            ui.label(format!("protocol: {}", epoch.protocol_version));
            ui.label(format!("graph: {}", epoch.transition_graph_version));
            if let Some(head) = epoch.git_head.as_deref() {
                ui.label(format!("git: {}", short_hash(head)));
            }
            if let WalkResponse::Status { snapshot, .. } = response {
                let authority_color =
                    if snapshot.authority == ploke_eval::walk_client::WalkAuthority::Active {
                        Color32::LIGHT_GREEN
                    } else {
                        Color32::YELLOW
                    };
                ui.label(
                    RichText::new(format!("authority: {:?}", snapshot.authority))
                        .color(authority_color),
                );
                ui.label(format!(
                    "controller: {}",
                    if snapshot.controller_attached {
                        "attached"
                    } else {
                        "detached"
                    }
                ));
                if let Some(blocker) = &snapshot.blocker {
                    ui.label(
                        RichText::new(format!("blocked: {:?}", blocker.code))
                            .color(Color32::YELLOW),
                    )
                    .on_hover_text(&blocker.detail);
                }
                ui.label("actions");
                ui.horizontal_wrapped(|ui| {
                    for action in &snapshot.actions {
                        let label = action
                            .edge
                            .map_or_else(|| format!("{:?}", action.kind), |edge| edge.to_string());
                        let color = if action.enabled {
                            Color32::LIGHT_GREEN
                        } else {
                            Color32::GRAY
                        };
                        ui.label(RichText::new(label).color(color))
                            .on_hover_text(action_hint(action));
                    }
                });
            }
            if let WalkResponse::Job { job, .. } = response {
                ui.label(format!("job: {} ({:?})", job.command, job.status));
                ui.label(format!("operation: {}", job.operation_id));
                if let Some(source) = job.llm_source {
                    ui.label(format!("LLM source: {source:?}"));
                }
                if let Some(receipt) = &job.receipt {
                    let edges = receipt
                        .edges
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" → ");
                    ui.label(
                        RichText::new(format!(
                            "committed: {} → {}",
                            receipt.phase_before, receipt.phase_after
                        ))
                        .color(Color32::LIGHT_GREEN),
                    );
                    ui.label(format!("edges: {edges}"));
                    ui.label(format!(
                        "journal revision: {}",
                        receipt.version.journal_revision()
                    ));
                    ui.label(format!("event projection: {:?}", receipt.event_projection));
                }
                if let Some(resolution) = &job.resolution {
                    ui.label(
                        RichText::new(format!("resolved: {:?}", resolution.kind))
                            .color(Color32::YELLOW),
                    );
                    ui.label(format!(
                        "observed journal revision: {}",
                        resolution.observed.journal_revision()
                    ));
                }
            }
            ui.add_space(8.0);
            ScrollArea::vertical()
                .id_salt("walk_message")
                .max_height(180.0)
                .show(ui, |ui| {
                    ui.monospace(response_message(response));
                });
        } else {
            ui.label("No walk snapshot");
        }
        ui.separator();
        if let Some(notice) = self.notice {
            ui.label(notice);
        }
        ui.separator();
        if let (Some(query), Some(row)) = (&self.query_result, self.selected_row)
            && let Some(row) = query.result.rows.get(row)
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

fn response_message(response: &WalkResponse) -> String {
    match response {
        WalkResponse::Ok { result, .. } => result.text().to_string(),
        WalkResponse::Status { message, .. } => message.clone(),
        WalkResponse::Audit { report, .. } => report.render_table(),
        WalkResponse::Query { query } => format!(
            "query revision {} returned {} row(s)",
            query.result.revision.as_str(),
            query.result.row_count
        ),
        WalkResponse::Config { config, .. } => format!(
            "campaign {}\nsetup plan {}\nsetup receipt {}\nprovider {} (admitted manifest)\nprofile {}\nrun mode {:?}\nparallel cap {} ({:?})\npatch cap {} ({:?})",
            config.identity.record.campaign_id,
            short_hash(config.campaign.admission.plan_hash.as_str()),
            config.campaign.admission.path.display(),
            config.campaign.provider,
            config.profile.record.name,
            config.control.mode,
            config.control.parallel_cap.value,
            config.control.parallel_cap.source,
            config.control.patch_cap.value,
            config.control.patch_cap.source,
        ),
        WalkResponse::Job { job, message, .. } => job
            .message
            .as_ref()
            .map_or_else(|| message.clone(), |detail| format!("{message}\n{detail}")),
        WalkResponse::History { history } => format!(
            "{} durable session event(s) at journal revision {}",
            history.events.len(),
            history.version.journal_revision()
        ),
        WalkResponse::Delta { report, .. } => report.clone(),
        WalkResponse::Error { detail, .. } => detail.clone(),
    }
}

fn short_hash(text: &str) -> &str {
    text.get(..12).unwrap_or(text)
}

fn action_hint(action: &ploke_eval::walk_client::WalkAction) -> String {
    let mut hints = Vec::new();
    if action.requires_live_api {
        hints.push("requires live API admission".to_string());
    }
    if action.requires_git_changes {
        hints.push("requires checkout mutation admission".to_string());
    }
    if let Some(blocker) = action.blocker {
        hints.push(format!("blocked by {blocker:?}"));
    }
    if hints.is_empty() {
        "currently available".to_string()
    } else {
        hints.join("; ")
    }
}
