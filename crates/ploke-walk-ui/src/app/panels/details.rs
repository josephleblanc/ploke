use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};
use ploke_eval::walk_client::{
    AdvertisedStep, WalkAction, WalkActionKind, WalkAuthority, WalkPhase, WalkQuerySnapshot,
    WalkResponse, WalkRunEntry, WalkSessionSnapshot,
};

use crate::model::{AutoMode, HandoffContinuation, WalkRequestKind};

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
    pub(in crate::app) allow_live: &'a mut bool,
    pub(in crate::app) allow_git: &'a mut bool,
    pub(in crate::app) operation_pending: bool,
    pub(in crate::app) config_step: bool,
    pub(in crate::app) follows_endpoint: bool,
    pub(in crate::app) auto: &'a AutoMode,
    pub(in crate::app) updates: &'a [WalkResponse],
}

#[derive(Debug, Default)]
pub(in crate::app) struct DetailsAction {
    pub(in crate::app) refresh_health: bool,
    pub(in crate::app) show_state: bool,
    pub(in crate::app) start: Option<WalkPhase>,
    pub(in crate::app) step: Option<AdvertisedStep>,
    pub(in crate::app) stop: bool,
    pub(in crate::app) start_auto: bool,
    pub(in crate::app) stop_auto: bool,
    pub(in crate::app) resume_auto: bool,
    pub(in crate::app) end_auto: bool,
}

impl DetailsPanel<'_> {
    pub(in crate::app) fn show(mut self, ui: &mut Ui) -> DetailsAction {
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
                    .add_enabled(
                        self.client_available && walk_idle,
                        egui::Button::new("Refresh Status"),
                    )
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
                ui.label(format!(
                    "phase source: {}",
                    snapshot.position.source_label()
                ));
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
                self.render_controls(ui, snapshot, &mut action);
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
        if !matches!(self.response, Some(WalkResponse::Status { .. })) {
            self.render_auto_controls(ui, None, &mut action);
        }
        self.render_updates(ui);
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

    fn render_controls(
        &mut self,
        ui: &mut Ui,
        snapshot: &WalkSessionSnapshot,
        output: &mut DetailsAction,
    ) {
        ui.separator();
        ui.heading("Operator controls");
        ui.checkbox(self.allow_live, "Allow live provider operations");
        ui.checkbox(self.allow_git, "Allow checkout mutations");
        ui.label(
            "Grants are UI-local confirmations and are rechecked against each server-advertised action.",
        );

        let idle = !self.operation_pending;
        ui.horizontal_wrapped(|ui| {
            for action in snapshot
                .actions
                .iter()
                .filter(|action| action.kind == WalkActionKind::Start)
            {
                let enabled = idle
                    && action.target.is_some()
                    && action_allowed(action, *self.allow_live, *self.allow_git);
                if ui
                    .add_enabled(enabled, egui::Button::new("Start/attach"))
                    .on_hover_text(action_hint(action))
                    .clicked()
                {
                    output.start = action.target;
                }
            }
            let advertised =
                AdvertisedStep::from_snapshot(snapshot, *self.allow_live, *self.allow_git);
            let (label, hint, endpoint_ready) = match advertised.as_ref() {
                Ok(step) => {
                    let count = step.outcomes().len();
                    let noun = if count == 1 { "outcome" } else { "outcomes" };
                    let edges = step
                        .outcomes()
                        .iter()
                        .map(|edge| edge.id())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let endpoint_ready =
                        !step.requires_endpoint_following() || self.follows_endpoint;
                    let hint = if endpoint_ready {
                        format!("Admitted outcomes: {edges}")
                    } else {
                        "This Step can transfer to a successor; use an unpinned client that follows endpoint authority"
                            .to_string()
                    };
                    (
                        format!("Step · {count} possible {noun}"),
                        hint,
                        endpoint_ready,
                    )
                }
                Err(error) => ("Step".to_string(), error.to_string(), false),
            };
            if ui
                .add_enabled(
                    idle && advertised.is_ok() && endpoint_ready,
                    egui::Button::new(label),
                )
                .on_hover_text(hint)
                .clicked()
            {
                output.step = advertised.ok();
            }
            for action in snapshot
                .actions
                .iter()
                .filter(|action| action.kind == WalkActionKind::Stop)
            {
                let enabled = idle && action_allowed(action, *self.allow_live, *self.allow_git);
                if ui
                    .add_enabled(enabled, egui::Button::new("Stop idle server"))
                    .on_hover_text(
                        "Shuts down an idle walk server. This is not loop cancellation or pause.",
                    )
                    .clicked()
                {
                    output.stop = true;
                }
            }
        });

        self.render_auto_controls(ui, Some(snapshot), output);
    }

    fn render_auto_controls(
        &mut self,
        ui: &mut Ui,
        snapshot: Option<&WalkSessionSnapshot>,
        output: &mut DetailsAction,
    ) {
        ui.add_space(6.0);
        ui.label(RichText::new("UI-local automation · Step-mode server").strong());
        let auto_ready = snapshot.is_some_and(|snapshot| {
            auto_allowed(
                snapshot,
                self.config_step,
                self.follows_endpoint,
                *self.allow_live,
                *self.allow_git,
            ) && !self.operation_pending
        });
        match self.auto {
            AutoMode::Idle | AutoMode::Halted(_) | AutoMode::Complete => {
                if ui
                    .add_enabled(auto_ready, egui::Button::new("Auto-advance transitions"))
                    .clicked()
                {
                    output.start_auto = true;
                }
            }
            AutoMode::Refreshing { .. } => {
                if ui.button("Stop before next transition").clicked() {
                    output.stop_auto = true;
                }
            }
            AutoMode::Running => {
                if ui.button("Stop after current transition").clicked() {
                    output.stop_auto = true;
                }
            }
            AutoMode::LoadingConfig { continuation, .. }
            | AutoMode::CheckingSuccessor { continuation, .. } => {
                let label = match continuation {
                    HandoffContinuation::Manual => {
                        "Validating successor configuration for the manual handoff…"
                    }
                    HandoffContinuation::Auto => "Validating successor configuration…",
                    HandoffContinuation::Paused => {
                        "Validating successor configuration before pausing…"
                    }
                };
                ui.label(RichText::new(label).color(Color32::YELLOW));
                if *continuation == HandoffContinuation::Auto
                    && ui.button("Stop before next transition").clicked()
                {
                    output.stop_auto = true;
                }
            }
            AutoMode::Waiting(wait) => match wait.continuation {
                HandoffContinuation::Manual => {
                    ui.label(
                        RichText::new("Following successor controller for the manual handoff…")
                            .color(Color32::YELLOW),
                    );
                }
                HandoffContinuation::Auto => {
                    ui.label(
                        RichText::new("Waiting for successor controller").color(Color32::YELLOW),
                    );
                    if ui.button("Stop before successor transition").clicked() {
                        output.stop_auto = true;
                    }
                }
                HandoffContinuation::Paused => {
                    ui.label(
                        RichText::new("Waiting for successor controller before pausing…")
                            .color(Color32::YELLOW),
                    );
                }
            },
            AutoMode::Stopping => {
                ui.label(
                    RichText::new("Stopping after current transition…").color(Color32::YELLOW),
                );
            }
            AutoMode::Paused => {
                ui.label(RichText::new("Auto-advance paused").color(Color32::YELLOW));
                if ui
                    .add_enabled(auto_ready, egui::Button::new("Resume auto-advance"))
                    .clicked()
                {
                    output.resume_auto = true;
                }
                if ui.button("End local automation").clicked() {
                    output.end_auto = true;
                }
            }
        }
        if let AutoMode::Halted(reason) = self.auto {
            ui.label(RichText::new(reason).color(Color32::LIGHT_RED));
        }
        if matches!(self.auto, AutoMode::Complete) {
            ui.label(RichText::new("Run completed at R14a").color(Color32::LIGHT_GREEN));
        }
        if !auto_ready && matches!(self.auto, AutoMode::Idle | AutoMode::Paused) {
            let hint = snapshot.map_or(
                "Refresh Walk status before starting or resuming auto-advance.",
                |snapshot| {
                    auto_hint(
                        snapshot,
                        self.config_step,
                        self.follows_endpoint,
                        *self.allow_live,
                        *self.allow_git,
                    )
                },
            );
            ui.label(RichText::new(hint));
        }
    }

    fn render_updates(&self, ui: &mut Ui) {
        if self.updates.is_empty() {
            return;
        }
        ui.separator();
        egui::CollapsingHeader::new(format!("Operation updates ({})", self.updates.len()))
            .id_salt("operation_updates")
            .default_open(true)
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("operation_update_scroll")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for (index, response) in self.updates.iter().enumerate() {
                            ui.label(RichText::new(format!("Update {}", index + 1)).strong());
                            match response {
                                WalkResponse::Job { job, message, .. } => {
                                    ui.monospace(format!(
                                        "{} {} · {:?} · {} → {}",
                                        job.command,
                                        job.operation_id,
                                        job.status,
                                        job.phase_before,
                                        job.phase_after.map_or_else(
                                            || "-".to_string(),
                                            |phase| phase.to_string()
                                        )
                                    ));
                                    ui.label(message);
                                    if let Some(receipt) = &job.receipt {
                                        ui.monospace(format!(
                                            "receipt {} → {} · revision {} · projection {:?}",
                                            receipt.phase_before,
                                            receipt.phase_after,
                                            receipt.version.journal_revision(),
                                            receipt.event_projection
                                        ));
                                    }
                                }
                                WalkResponse::Error { code, detail, .. } => {
                                    ui.label(
                                        RichText::new(format!("{code}: {detail}"))
                                            .color(Color32::LIGHT_RED),
                                    );
                                }
                                response => {
                                    ui.label(response_message(response));
                                }
                            }
                            ui.add_space(4.0);
                        }
                    });
            });
    }
}

fn action_allowed(action: &WalkAction, allow_live: bool, allow_git: bool) -> bool {
    action.enabled
        && (!action.requires_live_api || allow_live)
        && (!action.requires_git_changes || allow_git)
}

fn auto_allowed(
    snapshot: &WalkSessionSnapshot,
    config_step: bool,
    follows_endpoint: bool,
    allow_live: bool,
    allow_git: bool,
) -> bool {
    config_step && follows_endpoint && strict_step(snapshot, allow_live, allow_git).is_ok()
}

fn auto_hint(
    snapshot: &WalkSessionSnapshot,
    config_step: bool,
    follows_endpoint: bool,
    allow_live: bool,
    allow_git: bool,
) -> &'static str {
    if !config_step {
        "Auto-advance requires an admitted Step-mode run."
    } else if !follows_endpoint {
        "Auto-advance requires an unpinned client that follows successor endpoints."
    } else if !matches!(
        snapshot.position,
        ploke_eval::walk_client::WalkPosition::Session { .. }
    ) {
        "Auto-advance requires an attached durable Session."
    } else if !snapshot.controller_attached || snapshot.authority != WalkAuthority::Active {
        "Auto-advance requires attached Active controller authority."
    } else if snapshot.actions.iter().any(|action| {
        action.kind == WalkActionKind::Step && action.enabled && action.requires_live_api
    }) && !allow_live
    {
        "Enable the live provider grant for the next transition."
    } else if snapshot.actions.iter().any(|action| {
        action.kind == WalkActionKind::Step && action.enabled && action.requires_git_changes
    }) && !allow_git
    {
        "Enable the checkout mutation grant for the next transition."
    } else {
        "Auto-advance requires a coherent server-advertised Step command."
    }
}

pub(in crate::app) fn strict_step(
    snapshot: &WalkSessionSnapshot,
    allow_live: bool,
    allow_git: bool,
) -> Result<AdvertisedStep, ploke_eval::spec::PrepareError> {
    if !allow_git
        && snapshot.actions.iter().any(|action| {
            action.kind == WalkActionKind::Step && action.enabled && action.requires_git_changes
        })
    {
        return Err(ploke_eval::spec::PrepareError::InvalidBatchSelection {
            detail: "auto-advance requires checkout authority covering every advertised outcome"
                .to_string(),
        });
    }
    AdvertisedStep::from_snapshot(snapshot, allow_live, allow_git)
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
            "campaign {}\nsetup plan {}\nsetup receipt {}\nprovider {} (admitted manifest)\nprofile {}\ncandidate patch gate {}\nrun mode {:?}\nparallel cap {} ({:?})\npatch cap {} ({:?})",
            config.identity.record.campaign_id,
            short_hash(config.campaign.admission.plan_hash.as_str()),
            config.campaign.admission.path.display(),
            config.campaign.provider,
            config.profile.record.name,
            config.profile.record.selection.patch.gate.as_str(),
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
        WalkResponse::EvaluationTraceIndex { index } => format!(
            "{} completed evaluation run(s) from {:?}",
            index.runs.len(),
            index.authority
        ),
        WalkResponse::EvaluationTrace { snapshot } => match &snapshot.trace {
            ploke_eval::walk_client::EvaluationTraceState::NotCompleted {
                registration, ..
            } => format!(
                "run {} is {:?}; mutable trace evidence was not opened",
                snapshot.coordinate.run_id, registration.value.lifecycle.execution_status
            ),
            ploke_eval::walk_client::EvaluationTraceState::Completed { trace } => format!(
                "run {}: {} turn(s), {} model exchange(s), {} protocol artifact(s)",
                snapshot.coordinate.run_id,
                trace.run.value.turn_count(),
                trace
                    .exchanges
                    .as_ref()
                    .map_or(0, |exchanges| exchanges.value.len()),
                trace.protocol.len()
            ),
        },
        WalkResponse::LlmTraceIndex { index } => {
            let sessions = index
                .lanes
                .iter()
                .map(|lane| lane.sessions.len())
                .sum::<usize>();
            format!(
                "{sessions} live LLM session(s), {} manifest issue(s)",
                index.issues.len()
            )
        }
        WalkResponse::LlmTrace { snapshot } => format!(
            "LLM session {}: {} published checkpoint(s)",
            snapshot.coordinate.session_id,
            snapshot.timeline.len()
        ),
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

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };

    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use ploke_eval::walk_client::{ControlEdge, ServerEpoch};

    use super::*;
    use crate::model::HandoffWait;

    #[test]
    fn kittest_step_button_obeys_explicit_capability_grant() {
        let response = status_response();
        let auto = AutoMode::Idle;
        let mut allow_live = false;
        let mut allow_git = false;
        let mut details = false;
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(680.0, 900.0))
            .build_ui(move |ui| {
                DetailsPanel {
                    runs: &[],
                    selected_run: None,
                    run_error: None,
                    client_available: true,
                    walk_pending: None,
                    response: Some(&response),
                    notice: None,
                    query_result: None,
                    selected_row: None,
                    run_details: &mut details,
                    allow_live: &mut allow_live,
                    allow_git: &mut allow_git,
                    operation_pending: false,
                    config_step: true,
                    follows_endpoint: true,
                    auto: &auto,
                    updates: &[],
                }
                .show(ui);
            });

        assert!(harness.get_by_label("Step").accesskit_node().is_disabled());
        assert!(
            !harness
                .get_by_label("Stop idle server")
                .accesskit_node()
                .is_disabled()
        );
        assert!(
            harness
                .get_by_label("Auto-advance transitions")
                .accesskit_node()
                .is_disabled()
        );

        harness
            .get_by_label("Allow live provider operations")
            .click();
        harness.run();

        assert!(
            !harness
                .get_by_label("Step · 1 possible outcome")
                .accesskit_node()
                .is_disabled()
        );
        assert!(
            !harness
                .get_by_label("Auto-advance transitions")
                .accesskit_node()
                .is_disabled()
        );
    }

    #[test]
    fn kittest_renders_one_step_button_for_each_branch_offer() {
        let cases = [
            (
                WalkPhase::R1,
                vec![ControlEdge::R1ToR2a, ControlEdge::R1ToR3],
                false,
                false,
                "Step · 2 possible outcomes",
            ),
            (
                WalkPhase::R4a,
                vec![ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
                false,
                false,
                "Step · 2 possible outcomes",
            ),
            (
                WalkPhase::R10,
                vec![ControlEdge::R10ToR11a, ControlEdge::R10ToR11],
                true,
                false,
                "Step · 2 possible outcomes",
            ),
            (
                WalkPhase::R12,
                vec![
                    ControlEdge::R12ToR13a,
                    ControlEdge::R12ToR13b,
                    ControlEdge::R12ToR13c,
                ],
                false,
                true,
                "Step · 3 possible outcomes",
            ),
        ];
        for (phase, edges, live, git, label) in cases {
            let response = status_response_for(phase, &edges);
            let auto = AutoMode::Idle;
            let mut allow_live = live;
            let mut allow_git = git;
            let mut details = false;
            let harness = Harness::builder()
                .with_size(egui::Vec2::new(680.0, 900.0))
                .build_ui(move |ui| {
                    DetailsPanel {
                        runs: &[],
                        selected_run: None,
                        run_error: None,
                        client_available: true,
                        walk_pending: None,
                        response: Some(&response),
                        notice: None,
                        query_result: None,
                        selected_row: None,
                        run_details: &mut details,
                        allow_live: &mut allow_live,
                        allow_git: &mut allow_git,
                        operation_pending: false,
                        config_step: true,
                        follows_endpoint: true,
                        auto: &auto,
                        updates: &[],
                    }
                    .show(ui);
                });

            assert_eq!(
                harness.query_all_by_label(label).count(),
                1,
                "phase {phase}"
            );
        }
    }

    #[test]
    fn kittest_pinned_manual_step_blocks_handoff_but_allows_stop_only() {
        let response = status_response_for(
            WalkPhase::R12,
            &[
                ControlEdge::R12ToR13a,
                ControlEdge::R12ToR13b,
                ControlEdge::R12ToR13c,
            ],
        );
        let auto = AutoMode::Idle;
        let mut allow_live = false;
        let mut allow_git = true;
        let mut details = false;
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(680.0, 900.0))
            .build_ui(move |ui| {
                DetailsPanel {
                    runs: &[],
                    selected_run: None,
                    run_error: None,
                    client_available: true,
                    walk_pending: None,
                    response: Some(&response),
                    notice: None,
                    query_result: None,
                    selected_row: None,
                    run_details: &mut details,
                    allow_live: &mut allow_live,
                    allow_git: &mut allow_git,
                    operation_pending: false,
                    config_step: true,
                    follows_endpoint: false,
                    auto: &auto,
                    updates: &[],
                }
                .show(ui);
            });

        assert!(
            harness
                .get_by_label("Step · 3 possible outcomes")
                .accesskit_node()
                .is_disabled(),
            "pinned client must not commit a possible successor transfer"
        );

        harness.get_by_label("Allow checkout mutations").click();
        harness.run();

        assert!(
            !harness
                .get_by_label("Step · 1 possible outcome")
                .accesskit_node()
                .is_disabled(),
            "without checkout authority the retained R12 offer is stop-only"
        );
    }

    #[test]
    fn kittest_auto_labels_preserve_handoff_pause_intent() {
        let response = status_response();
        let now = Instant::now();
        let auto = AutoMode::Waiting(HandoffWait {
            prior: serde_json::from_value(serde_json::json!(
                "00000000-0000-0000-0000-000000000001"
            ))
            .expect("typed test session"),
            started: now,
            next: now,
            delay: Duration::from_millis(250),
            after: 0,
            continuation: HandoffContinuation::Paused,
        });
        let mut allow_live = true;
        let mut allow_git = true;
        let mut details = false;
        let harness = Harness::builder()
            .with_size(egui::Vec2::new(680.0, 900.0))
            .build_ui(move |ui| {
                DetailsPanel {
                    runs: &[],
                    selected_run: None,
                    run_error: None,
                    client_available: true,
                    walk_pending: None,
                    response: Some(&response),
                    notice: None,
                    query_result: None,
                    selected_row: None,
                    run_details: &mut details,
                    allow_live: &mut allow_live,
                    allow_git: &mut allow_git,
                    operation_pending: false,
                    config_step: true,
                    follows_endpoint: true,
                    auto: &auto,
                    updates: &[],
                }
                .show(ui);
            });

        harness.get_by_label("UI-local automation · Step-mode server");
        harness.get_by_label("Waiting for successor controller before pausing…");
        assert_eq!(
            harness
                .query_all_by_label("Stop after current transition")
                .count(),
            0
        );
    }

    #[test]
    fn kittest_paused_auto_exposes_explicit_end() {
        let response = status_response();
        let auto = AutoMode::Paused;
        let mut allow_live = true;
        let mut allow_git = true;
        let mut details = false;
        let harness = Harness::builder()
            .with_size(egui::Vec2::new(680.0, 900.0))
            .build_ui(move |ui| {
                DetailsPanel {
                    runs: &[],
                    selected_run: None,
                    run_error: None,
                    client_available: true,
                    walk_pending: None,
                    response: Some(&response),
                    notice: None,
                    query_result: None,
                    selected_row: None,
                    run_details: &mut details,
                    allow_live: &mut allow_live,
                    allow_git: &mut allow_git,
                    operation_pending: false,
                    config_step: true,
                    follows_endpoint: true,
                    auto: &auto,
                    updates: &[],
                }
                .show(ui);
            });

        harness.get_by_label("Resume auto-advance");
        harness.get_by_label("End local automation");
    }

    #[test]
    fn kittest_refreshing_can_stop_before_the_next_transition() {
        let response = status_response();
        let auto = AutoMode::Refreshing { after: 7 };
        let mut allow_live = true;
        let mut allow_git = true;
        let mut details = false;
        let harness = Harness::builder()
            .with_size(egui::Vec2::new(680.0, 900.0))
            .build_ui(move |ui| {
                DetailsPanel {
                    runs: &[],
                    selected_run: None,
                    run_error: None,
                    client_available: true,
                    walk_pending: None,
                    response: Some(&response),
                    notice: None,
                    query_result: None,
                    selected_row: None,
                    run_details: &mut details,
                    allow_live: &mut allow_live,
                    allow_git: &mut allow_git,
                    operation_pending: false,
                    config_step: true,
                    follows_endpoint: true,
                    auto: &auto,
                    updates: &[],
                }
                .show(ui);
            });

        harness.get_by_label("Stop before next transition");
        assert_eq!(
            harness
                .query_all_by_label("Stop after current transition")
                .count(),
            0
        );
    }

    #[test]
    fn kittest_running_job_keeps_auto_stop_actionable() {
        let response = job_response();
        let auto = AutoMode::Running;
        let mut allow_live = true;
        let mut allow_git = true;
        let mut details = false;
        let harness = Harness::builder()
            .with_size(egui::Vec2::new(680.0, 900.0))
            .build_ui(move |ui| {
                DetailsPanel {
                    runs: &[],
                    selected_run: None,
                    run_error: None,
                    client_available: true,
                    walk_pending: None,
                    response: Some(&response),
                    notice: None,
                    query_result: None,
                    selected_row: None,
                    run_details: &mut details,
                    allow_live: &mut allow_live,
                    allow_git: &mut allow_git,
                    operation_pending: true,
                    config_step: true,
                    follows_endpoint: true,
                    auto: &auto,
                    updates: &[],
                }
                .show(ui);
            });

        harness.get_by_label("job: step (Running)");
        assert!(
            !harness
                .get_by_label("Stop after current transition")
                .accesskit_node()
                .is_disabled(),
            "auto stop must remain actionable while a Job response is displayed"
        );
    }

    fn job_response() -> WalkResponse {
        serde_json::from_value(serde_json::json!({
            "type": "job",
            "phase": "r5",
            "job": {
                "job_id": 4,
                "operation_id": "00000000-0000-0000-0000-000000000004",
                "expected": {
                    "session_id": "00000000-0000-0000-0000-000000000001",
                    "cursor": {
                        "phase": "r5",
                        "evidence": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    },
                    "journal_revision": 3
                },
                "command": "step",
                "status": "running",
                "phase_before": "r5",
                "phase_after": null,
                "target_phase": null,
                "watch": false,
                "allow_live_api": true,
                "allow_git_changes": false,
                "started_at": "2026-07-31T00:00:00Z",
                "updated_at": "2026-07-31T00:00:00Z",
                "finished_at": null,
                "message": "running",
                "receipt": null,
                "resolution": null
            },
            "message": "accepted",
            "epoch": {
                "protocol_version": 9,
                "transition_graph_version": "walk-r0-r14a-v2",
                "repo_root": "/tmp/ploke-parent",
                "exe_path": "/tmp/ploke-eval",
                "exe_modified_unix_ms": 17,
                "git_head": "abc123",
                "active_branch": "parent/runtime-1",
                "source_status_hash": "def456"
            }
        }))
        .expect("typed running job")
    }

    fn status_response() -> WalkResponse {
        status_response_for(WalkPhase::R5, &[ControlEdge::R5ToR6])
    }

    fn status_response_for(phase: WalkPhase, edges: &[ControlEdge]) -> WalkResponse {
        let mut actions = edges
            .iter()
            .map(|edge| {
                serde_json::json!({
                    "kind": "step",
                    "edge": edge,
                    "target": edge.to(),
                    "enabled": true,
                    "requires_live_api": edge.requires_live(),
                    "requires_git_changes": edge.requires_checkout(),
                    "blocker": null
                })
            })
            .collect::<Vec<_>>();
        actions.push(serde_json::json!({
            "kind": "stop",
            "edge": null,
            "target": null,
            "enabled": true,
            "requires_live_api": false,
            "requires_git_changes": false,
            "blocker": null
        }));
        let snapshot: WalkSessionSnapshot = serde_json::from_value(serde_json::json!({
            "phase": phase,
            "version": {
                "session_id": "00000000-0000-0000-0000-000000000001",
                "cursor": {
                    "phase": phase,
                    "evidence": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                },
                "journal_revision": 3
            },
            "position": {
                "source": "session",
                "version": {
                    "session_id": "00000000-0000-0000-0000-000000000001",
                    "cursor": {
                        "phase": phase,
                        "evidence": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    },
                    "journal_revision": 3
                }
            },
            "controller_attached": true,
            "authority": "active",
            "job": null,
            "blocker": null,
            "actions": actions
        }))
        .expect("typed active status");
        WalkResponse::Status {
            message: "active".to_string(),
            snapshot,
            epoch: ServerEpoch {
                protocol_version: 9,
                transition_graph_version: "walk-r0-r14a-v2".to_string(),
                repo_root: PathBuf::from("/tmp/ploke-parent"),
                exe_path: PathBuf::from("/tmp/ploke-eval"),
                exe_modified_unix_ms: Some(17),
                git_head: Some("abc123".to_string()),
                active_branch: Some("parent/runtime-1".to_string()),
                source_status_hash: Some("def456".to_string()),
                build_fingerprint: String::new(),
            },
        }
    }
}
