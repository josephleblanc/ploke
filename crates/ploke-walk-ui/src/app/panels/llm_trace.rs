use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};
use ploke_eval::walk_client::{
    LlmArtifact, LlmHeadlessTerminal, LlmLane, LlmOuterEvidence, LlmResume, LlmSessionSummary,
    LlmStepDetail, LlmStepEntry, LlmStepOutcome, LlmToolResult, LlmTraceCoordinate, LlmTraceIndex,
    LlmTraceIssue, LlmTraceSnapshot, LlmWorkspaceState, TraceSource,
};

pub(in crate::app) struct LlmTracePanel<'a> {
    pub(in crate::app) index: Option<&'a LlmTraceIndex>,
    pub(in crate::app) selected: &'a mut Option<String>,
    pub(in crate::app) snapshot: Option<&'a LlmTraceSnapshot>,
    pub(in crate::app) index_pending: bool,
    pub(in crate::app) trace_pending: bool,
}

#[derive(Debug, Default)]
pub(in crate::app) struct LlmTraceAction {
    pub(in crate::app) refresh: bool,
    pub(in crate::app) load: Option<LlmTraceCoordinate>,
}

impl LlmTracePanel<'_> {
    pub(in crate::app) fn show(self, ui: &mut Ui) -> LlmTraceAction {
        let mut action = LlmTraceAction::default();
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("LIVE LLM OBSERVATION")
                    .strong()
                    .color(Color32::LIGHT_BLUE),
            )
            .on_hover_text(
                "Read-only observation of mutable debugger checkpoints. Session, resume, step, headless, and agent-turn files may be written at different times.",
            );
            ui.separator();
            ui.label("read-only · manual refresh").on_hover_text(
                "This view does not poll. Refresh again to observe files published after this snapshot.",
            );
            if ui
                .add_enabled(!self.index_pending, egui::Button::new("Refresh Sessions"))
                .on_hover_text(
                    "Read the campaign's current tool-loop session inventory. Existing detail is not treated as a live subscription.",
                )
                .clicked()
            {
                action.refresh = true;
            }
            if self.index_pending {
                ui.colored_label(Color32::YELLOW, "loading session index...");
            }
        });
        ui.label(
            "Each evidence layer keeps its own persisted state and source identity; no combined run status is inferred.",
        );
        ui.add_space(8.0);

        let Some(index) = self.index else {
            ui.separator();
            ui.colored_label(Color32::GRAY, "No live LLM session index loaded.");
            return action;
        };

        render_index(ui, index);
        ui.add_space(8.0);
        render_issues(ui, &index.issues);
        for lane in &index.lanes {
            render_lane(
                ui,
                lane,
                self.selected,
                self.snapshot,
                self.trace_pending,
                &mut action,
            );
        }

        ui.add_space(8.0);
        ui.separator();
        match (self.selected.as_deref(), self.snapshot) {
            (None, _) => {
                ui.label("Choose an exact debugger session to load its current observation.");
            }
            (Some(_), None) => {
                ui.label(if self.trace_pending {
                    "Loading the selected debugger session..."
                } else {
                    "Load the selected session to inspect its current files."
                });
            }
            (Some(id), Some(snapshot)) if snapshot.coordinate.session_id != id => {
                ui.colored_label(
                    Color32::YELLOW,
                    "The visible observation belongs to a previous session and has been hidden.",
                );
            }
            (Some(_), Some(snapshot)) => {
                ScrollArea::vertical()
                    .id_salt("live_llm_observation")
                    .auto_shrink([false, false])
                    .show(ui, |ui| render_snapshot(ui, snapshot, &mut action));
            }
        }
        action
    }
}

fn render_index(ui: &mut Ui, index: &LlmTraceIndex) {
    let sessions = index
        .lanes
        .iter()
        .map(|lane| lane.sessions.len())
        .sum::<usize>();
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("campaign: {}", index.campaign));
        ui.separator();
        ui.label(format!("lanes: {}", index.lanes.len()));
        ui.separator();
        ui.label(format!("sessions: {sessions}"));
        ui.separator();
        ui.label(format!("issues: {}", index.issues.len()));
        ui.separator();
        ui.label(format!("authority: {:?}", index.authority));
        ui.separator();
        ui.label(format!(
            "session revision: {}",
            index.version.journal_revision()
        ));
        ui.separator();
        ui.label(format!("protocol: {}", index.epoch.protocol_version));
    });
    ui.label(
        RichText::new(index.root.display().to_string())
            .monospace()
            .color(Color32::GRAY),
    );
}

fn render_issues(ui: &mut Ui, issues: &[LlmTraceIssue]) {
    if issues.is_empty() {
        return;
    }
    egui::CollapsingHeader::new(format!("Session manifest issues ({})", issues.len()))
        .default_open(true)
        .show(ui, |ui| {
            ui.colored_label(
                Color32::YELLOW,
                "Malformed or missing sessions remain visible here and do not hide healthy siblings.",
            );
            for issue in issues {
                match issue {
                    LlmTraceIssue::Missing { session_id, path } => {
                        ui.group(|ui| {
                            ui.colored_label(
                                Color32::YELLOW,
                                format!("session {session_id}: manifest missing"),
                            );
                            render_path(ui, path);
                        });
                    }
                    LlmTraceIssue::Invalid {
                        session_id,
                        source,
                        detail,
                    } => {
                        ui.group(|ui| {
                            ui.colored_label(
                                Color32::LIGHT_RED,
                                format!("session {session_id}: manifest invalid"),
                            );
                            ui.label(detail);
                            render_source(ui, source);
                        });
                    }
                    LlmTraceIssue::Unreadable {
                        session_id,
                        path,
                        detail,
                    } => {
                        ui.group(|ui| {
                            ui.colored_label(
                                Color32::LIGHT_RED,
                                format!("session {session_id}: manifest unreadable"),
                            );
                            ui.label(detail);
                            render_path(ui, path);
                        });
                    }
                }
            }
        });
}

fn render_lane(
    ui: &mut Ui,
    lane: &LlmLane,
    selected: &mut Option<String>,
    snapshot: Option<&LlmTraceSnapshot>,
    pending: bool,
    action: &mut LlmTraceAction,
) {
    egui::CollapsingHeader::new(format!(
        "lane {} · {} session(s)",
        lane.lane_id,
        lane.sessions.len()
    ))
    .id_salt(("llm_lane", &lane.lane_id))
    .default_open(true)
    .show(ui, |ui| {
        for session in &lane.sessions {
            render_session(ui, session, selected, snapshot, pending, action);
        }
    });
}

fn render_session(
    ui: &mut Ui,
    summary: &LlmSessionSummary,
    selected: &mut Option<String>,
    snapshot: Option<&LlmTraceSnapshot>,
    pending: bool,
    action: &mut LlmTraceAction,
) {
    let session = &summary.session.value;
    let is_selected = selected.as_deref() == Some(session.session_id.as_str());
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!("debug status: {:?}", session.status))
                    .strong()
                    .color(debug_color(session.status)),
            );
            ui.separator();
            ui.label(RichText::new(&session.session_id).monospace());
            let same = snapshot.is_some_and(|current| {
                current.coordinate.session_id == session.session_id
                    && current.coordinate.step.is_none()
            });
            let label = if same { "Reload Session" } else { "Load Session" };
            if ui
                .add_enabled(!pending, egui::Button::new(label).selected(is_selected))
                .on_hover_text(
                    "Load this exact session without selecting a representative or latest lane entry.",
                )
                .clicked()
            {
                *selected = Some(session.session_id.clone());
                action.load = Some(LlmTraceCoordinate {
                    session_id: session.session_id.clone(),
                    step: None,
                });
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("lane: {}", session.lane_id));
            ui.separator();
            ui.label(format!("model: {}", session.model.as_deref().unwrap_or("not recorded")));
            ui.separator();
            ui.label(format!("workspace: {}", session.workspace.display()));
        });
        render_source(ui, &summary.session.source);
        render_resume(ui, &summary.resume, false);
    });
}

fn render_snapshot(ui: &mut Ui, snapshot: &LlmTraceSnapshot, action: &mut LlmTraceAction) {
    ui.heading(format!(
        "Debugger session {}",
        snapshot.session.value.session_id
    ));
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!("debug status: {:?}", snapshot.session.value.status))
                .strong()
                .color(debug_color(snapshot.session.value.status)),
        );
        ui.separator();
        ui.label(format!("authority: {:?}", snapshot.authority));
        ui.separator();
        ui.label(format!(
            "session revision: {}",
            snapshot.version.journal_revision()
        ));
        ui.separator();
        ui.label(format!("protocol: {}", snapshot.epoch.protocol_version));
    });
    ui.label(
        "Debugger status is the exact session manifest value; it does not summarize outer execution.",
    );
    render_source(ui, &snapshot.session.source);

    egui::CollapsingHeader::new("Resume frontier")
        .default_open(true)
        .show(ui, |ui| render_resume(ui, &snapshot.resume, true));

    render_timeline(ui, snapshot, action);
    render_selected(ui, snapshot.selected.as_ref());
    render_outer(ui, &snapshot.outer);
}

fn render_resume(ui: &mut Ui, artifact: &LlmArtifact<LlmResume>, full: bool) {
    match artifact {
        LlmArtifact::Present { evidence } => {
            let resume = &evidence.value;
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("next step: {}", resume.next_step));
                ui.separator();
                ui.label(format!("attempts: {}", resume.attempts));
                ui.separator();
                ui.label(format!("terminal flag: {}", resume.terminal));
            });
            if full {
                ui.label(format!(
                    "assistant message: {}",
                    resume.assistant_message_id
                ));
                ui.label(format!("request: {}", resume.request_id));
                ui.label(format!("parent: {}", resume.parent_id));
            }
            render_source(ui, &evidence.source);
        }
        LlmArtifact::Missing { path } => {
            render_missing(ui, "resume frontier", path);
        }
        LlmArtifact::Unreadable { path, detail } => {
            render_unreadable(ui, "resume frontier", path, detail);
        }
        LlmArtifact::Invalid { source, detail } => {
            render_invalid(ui, "resume frontier", source, detail);
        }
    }
}

fn render_timeline(ui: &mut Ui, snapshot: &LlmTraceSnapshot, action: &mut LlmTraceAction) {
    egui::CollapsingHeader::new(format!(
        "Published response checkpoints ({})",
        snapshot.timeline.len()
    ))
    .default_open(true)
    .show(ui, |ui| {
        if snapshot.timeline.is_empty() {
            ui.colored_label(
                Color32::GRAY,
                "No response checkpoint is present in this observation.",
            );
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for entry in &snapshot.timeline {
                match entry {
                    LlmStepEntry::Present { summary } => {
                        let step = summary.value.step;
                        let selected = snapshot.coordinate.step == Some(step);
                        let response = ui
                            .add(egui::Button::new(step_label(&summary.value.outcome, step)).selected(selected))
                            .on_hover_ui(|ui| {
                                ui.label(format!("tool requests: {}", summary.value.tool_requests));
                                ui.label(format!("tool results: {}", summary.value.tool_results));
                                ui.label(format!("terminal flag: {}", summary.value.terminal));
                                render_source(ui, &summary.source);
                            });
                        if response.clicked() {
                            action.load = Some(LlmTraceCoordinate {
                                session_id: snapshot.session.value.session_id.clone(),
                                step: Some(step),
                            });
                        }
                    }
                    LlmStepEntry::Invalid {
                        step,
                        source,
                        detail,
                    } => {
                        ui.add_enabled(false, egui::Button::new(format!("#{step} invalid")))
                            .on_hover_ui(|ui| {
                                ui.label(detail);
                                render_source(ui, source);
                            });
                    }
                    LlmStepEntry::Missing { step, path } => {
                        ui.add_enabled(false, egui::Button::new(format!("#{step} missing")))
                            .on_hover_ui(|ui| render_path(ui, path));
                    }
                    LlmStepEntry::Unreadable { step, path, detail } => {
                        ui.add_enabled(false, egui::Button::new(format!("#{step} unreadable")))
                            .on_hover_ui(|ui| {
                                ui.label(detail);
                                render_path(ui, path);
                            });
                    }
                }
            }
        });
        ui.label(
            "Choose a checkpoint to load its full prompt, provider response, and settled tool batch.",
        );
    });
}

fn render_selected(ui: &mut Ui, artifact: Option<&LlmArtifact<LlmStepDetail>>) {
    egui::CollapsingHeader::new("Selected checkpoint detail")
        .default_open(true)
        .show(ui, |ui| match artifact {
            Some(LlmArtifact::Present { evidence }) => {
                let step = &evidence.value;
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("step: {}", step.step));
                    ui.separator();
                    ui.label(format!("outcome: {}", outcome_label(&step.outcome)));
                    ui.separator();
                    ui.label(format!("terminal flag: {}", step.terminal));
                });
                render_source(ui, &evidence.source);
                render_prompt(ui, step);
                render_response(ui, step);
                render_tools(ui, step);
                render_workspace(ui, "workspace before", &step.workspace_before);
                render_workspace(ui, "workspace after", &step.workspace_after);
                if !step.events.is_empty() {
                    egui::CollapsingHeader::new(format!("Observed events ({})", step.events.len()))
                        .show(ui, |ui| {
                            for (index, event) in step.events.iter().enumerate() {
                                if let Ok(raw) = serde_json::to_string_pretty(event) {
                                    text_block(ui, &format!("event {index}"), &raw);
                                }
                            }
                        });
                }
            }
            Some(LlmArtifact::Missing { path }) => {
                render_missing(ui, "selected checkpoint", path);
            }
            Some(LlmArtifact::Unreadable { path, detail }) => {
                render_unreadable(ui, "selected checkpoint", path, detail);
            }
            Some(LlmArtifact::Invalid { source, detail }) => {
                render_invalid(ui, "selected checkpoint", source, detail);
            }
            None => {
                ui.colored_label(
                    Color32::GRAY,
                    "No checkpoint is selected because no published checkpoint is available.",
                );
            }
        });
}

fn render_prompt(ui: &mut Ui, step: &LlmStepDetail) {
    egui::CollapsingHeader::new(format!("Request prompt ({})", step.request_messages.len()))
        .default_open(true)
        .show(ui, |ui| {
            for (index, message) in step.request_messages.iter().enumerate() {
                egui::CollapsingHeader::new(format!("message {index} · {:?}", message.role))
                    .default_open(index + 1 == step.request_messages.len())
                    .show(ui, |ui| {
                        text_block(ui, "content", &message.content);
                        if let Some(call_id) = message.tool_call_id.as_deref() {
                            ui.label(format!("tool call id: {call_id}"));
                        }
                        if let Some(calls) = message.tool_calls.as_ref() {
                            for call in calls {
                                ui.label(format!(
                                    "tool: {} ({})",
                                    call.function.name.as_str(),
                                    call.call_id
                                ));
                                text_block(
                                    ui,
                                    "arguments",
                                    &pretty_json(call.function.arguments.as_str()),
                                );
                            }
                        }
                    });
            }
        });
}

fn render_response(ui: &mut Ui, step: &LlmStepDetail) {
    let record = &step.response;
    let response = record.response();
    egui::CollapsingHeader::new(format!(
        "Provider response {} · {}",
        record.response_index().get(),
        response.model
    ))
    .default_open(true)
    .show(ui, |ui| {
        ui.label(format!(
            "assistant message: {}",
            record.assistant_message_id
        ));
        ui.label(format!("response id: {}", response.id));
        ui.label(format!(
            "provider: {}",
            response
                .provider
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "not recorded".to_string())
        ));
        for (index, choice) in response.choices.iter().enumerate() {
            egui::CollapsingHeader::new(format!(
                "choice {index} · finish {:?}",
                choice.finish_reason
            ))
            .default_open(true)
            .show(ui, |ui| {
                if let Some(message) = choice.message.as_ref() {
                    if let Some(content) = message.content.as_deref() {
                        text_block(ui, "assistant content", content);
                    }
                    if let Some(reasoning) = message.reasoning.as_deref() {
                        text_block(ui, "assistant reasoning", reasoning);
                    }
                    if let Some(calls) = message.tool_calls.as_ref() {
                        for call in calls {
                            ui.label(format!(
                                "tool: {} ({})",
                                call.function.name.as_str(),
                                call.call_id
                            ));
                            text_block(ui, "arguments", &pretty_json(&call.function.arguments));
                        }
                    }
                }
            });
        }
        egui::CollapsingHeader::new("Normalized provider envelope").show(ui, |ui| {
            if let Ok(raw) = serde_json::to_string_pretty(response) {
                text_block(ui, "response JSON", &raw);
            }
        });
    });
}

fn render_tools(ui: &mut Ui, step: &LlmStepDetail) {
    egui::CollapsingHeader::new(format!("Tool requests ({})", step.tool_requests.len()))
        .default_open(true)
        .show(ui, |ui| {
            for (index, request) in step.tool_requests.iter().enumerate() {
                egui::CollapsingHeader::new(format!(
                    "[{index}] {} · {}",
                    request.tool, request.call_id
                ))
                .show(ui, |ui| {
                    ui.label(format!("request: {}", request.request_id));
                    ui.label(format!("parent: {}", request.parent_id));
                    text_block(ui, "arguments", &pretty_json(request.arguments.as_str()));
                });
            }
        });
    egui::CollapsingHeader::new(format!("Tool results ({})", step.tool_results.len()))
        .default_open(true)
        .show(ui, |ui| {
            for (index, result) in step.tool_results.iter().enumerate() {
                match result {
                    LlmToolResult::Completed(record) => {
                        egui::CollapsingHeader::new(format!(
                            "[{index}] completed · {} · {}",
                            record.tool, record.call_id
                        ))
                        .show(ui, |ui| {
                            ui.label(format!("latency: {} ms", record.latency_ms));
                            text_block(ui, "result", &record.content);
                        });
                    }
                    LlmToolResult::Failed(record) => {
                        egui::CollapsingHeader::new(format!(
                            "[{index}] failed · {} · {}",
                            record.tool.as_deref().unwrap_or("unknown tool"),
                            record.call_id
                        ))
                        .show(ui, |ui| {
                            ui.colored_label(Color32::LIGHT_RED, "tool failed");
                            ui.label(format!("latency: {} ms", record.latency_ms));
                            text_block(ui, "error", &record.error);
                        });
                    }
                }
            }
        });
}

fn render_workspace(ui: &mut Ui, label: &str, workspace: &LlmWorkspaceState) {
    egui::CollapsingHeader::new(format!(
        "{label} · {} dirty path(s)",
        workspace.dirty_paths.len()
    ))
    .show(ui, |ui| {
        for path in &workspace.dirty_paths {
            render_path(ui, path);
        }
        if let Some(error) = workspace.error.as_deref() {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
    });
}

fn render_outer(ui: &mut Ui, artifact: &LlmArtifact<LlmOuterEvidence>) {
    ui.separator();
    ui.label(
        RichText::new("OUTER EXECUTOR EVIDENCE")
            .strong()
            .color(Color32::LIGHT_BLUE),
    )
    .on_hover_text(
        "Headless completion and agent-turn completion are persisted separately from debugger session status.",
    );
    match artifact {
        LlmArtifact::Present { evidence } => {
            ui.label("Published harness request source");
            render_source(ui, &evidence.source);
            render_headless(ui, &evidence.value.headless);
            render_turn(ui, &evidence.value);
        }
        LlmArtifact::Missing { path } => {
            render_missing(ui, "outer harness evidence", path);
        }
        LlmArtifact::Unreadable { path, detail } => {
            render_unreadable(ui, "outer harness evidence", path, detail);
        }
        LlmArtifact::Invalid { source, detail } => {
            render_invalid(ui, "outer harness evidence", source, detail);
        }
    }
}

fn render_headless(ui: &mut Ui, artifact: &LlmArtifact<Option<LlmHeadlessTerminal>>) {
    egui::CollapsingHeader::new("Outer headless terminal")
        .default_open(true)
        .show(ui, |ui| match artifact {
            LlmArtifact::Present { evidence } => {
                match evidence.value.as_ref() {
                    Some(terminal) => {
                        ui.label(
                            RichText::new(format!("terminal: {}", terminal_label(terminal)))
                                .strong(),
                        );
                        if let Ok(raw) = serde_json::to_string_pretty(terminal) {
                            text_block(ui, "terminal record", &raw);
                        }
                    }
                    None => {
                        ui.colored_label(
                            Color32::GRAY,
                            "terminal: not recorded in this headless summary",
                        )
                        .on_hover_text(
                            "Absence is shown as absence; this view has no authority to label it in flight.",
                        );
                    }
                }
                render_source(ui, &evidence.source);
            }
            LlmArtifact::Missing { path } => {
                render_missing(ui, "headless summary", path);
            }
            LlmArtifact::Unreadable { path, detail } => {
                render_unreadable(ui, "headless summary", path, detail);
            }
            LlmArtifact::Invalid { source, detail } => {
                render_invalid(ui, "headless summary", source, detail);
            }
        });
}

fn render_turn(ui: &mut Ui, outer: &LlmOuterEvidence) {
    egui::CollapsingHeader::new("Agent-turn summary terminal")
        .default_open(true)
        .show(ui, |ui| match &outer.turn {
            LlmArtifact::Present { evidence } => {
                let turn = &evidence.value.0;
                ui.label(format!("task: {}", turn.task_id));
                ui.label(format!("model: {}", turn.selected_model));
                match turn.terminal_record.as_ref() {
                    Some(terminal) => {
                        ui.label(
                            RichText::new(format!("terminal outcome: {}", terminal.outcome))
                                .strong(),
                        );
                        ui.label(format!("attempts: {}", terminal.attempts));
                        text_block(ui, "terminal summary", &terminal.summary);
                    }
                    None => {
                        ui.colored_label(
                            Color32::GRAY,
                            "terminal outcome: not recorded in this agent-turn summary",
                        )
                        .on_hover_text(
                            "This missing field is not used to infer whether the outer executor is still running.",
                        );
                    }
                }
                render_source(ui, &evidence.source);
            }
            LlmArtifact::Missing { path } => {
                render_missing(ui, "agent-turn summary", path);
            }
            LlmArtifact::Unreadable { path, detail } => {
                render_unreadable(ui, "agent-turn summary", path, detail);
            }
            LlmArtifact::Invalid { source, detail } => {
                render_invalid(ui, "agent-turn summary", source, detail);
            }
        });
}

fn render_missing(ui: &mut Ui, label: &str, path: &std::path::Path) {
    ui.colored_label(Color32::GRAY, format!("{label}: file missing"))
        .on_hover_text(
            "Files in this observation may be published at different times. Missing evidence is not labeled in flight.",
        );
    render_path(ui, path);
}

fn render_invalid(ui: &mut Ui, label: &str, source: &TraceSource, detail: &str) {
    ui.colored_label(Color32::LIGHT_RED, format!("{label}: file invalid"));
    ui.label(detail);
    render_source(ui, source);
}

fn render_unreadable(ui: &mut Ui, label: &str, path: &std::path::Path, detail: &str) {
    ui.colored_label(Color32::LIGHT_RED, format!("{label}: file unreadable"));
    ui.label(detail);
    render_path(ui, path);
}

fn render_source(ui: &mut Ui, source: &TraceSource) {
    ui.group(|ui| {
        ui.label(format!("source: {:?}", source.kind));
        render_path(ui, &source.path);
        ui.label(
            RichText::new(format!("sha256: {}", source.content_sha256))
                .monospace()
                .color(Color32::GRAY),
        );
    });
}

fn render_path(ui: &mut Ui, path: &std::path::Path) {
    ui.label(RichText::new(path.display().to_string()).monospace());
}

fn text_block(ui: &mut Ui, label: &str, text: &str) {
    ui.label(RichText::new(format!("{label}:")).strong());
    ui.add(
        egui::Label::new(RichText::new(text).monospace())
            .wrap()
            .selectable(true),
    );
}

fn pretty_json(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| raw.to_string())
}

fn step_label(outcome: &LlmStepOutcome, step: usize) -> String {
    match outcome {
        LlmStepOutcome::ToolCalls { count, .. } => format!("#{step} · {count} tool call(s)"),
        LlmStepOutcome::Content { .. } => format!("#{step} · content"),
    }
}

fn outcome_label(outcome: &LlmStepOutcome) -> String {
    match outcome {
        LlmStepOutcome::ToolCalls {
            count,
            finish_reason,
            ..
        } => format!("tool calls: {count} · finish: {finish_reason}"),
        LlmStepOutcome::Content { .. } => "content".to_string(),
    }
}

fn terminal_label(terminal: &LlmHeadlessTerminal) -> &'static str {
    match terminal {
        LlmHeadlessTerminal::Applied { .. } => "applied",
        LlmHeadlessTerminal::Exhausted { .. } => "exhausted",
        LlmHeadlessTerminal::CompletedWithoutEdit { .. } => "completed without edit",
        LlmHeadlessTerminal::ToolFailed { .. } => "tool failed",
        LlmHeadlessTerminal::NoEdit => "no edit",
        LlmHeadlessTerminal::ContextUnavailable { .. } => "context unavailable",
        LlmHeadlessTerminal::ProviderUnavailable { .. } => "provider unavailable",
        LlmHeadlessTerminal::SetupUnavailable { .. } => "setup unavailable",
        LlmHeadlessTerminal::AppliedValidationFailed { .. } => "applied; validation failed",
        LlmHeadlessTerminal::AppliedValidationMissing { .. } => "applied; validation missing",
        LlmHeadlessTerminal::AppliedTurnAborted { .. } => "applied; turn aborted",
        LlmHeadlessTerminal::AppliedTimedOut { .. } => "applied; timed out",
        LlmHeadlessTerminal::TimedOut { .. } => "timed out",
    }
}

fn debug_color(status: ploke_eval::walk_client::LlmSessionStatus) -> Color32 {
    match status {
        ploke_eval::walk_client::LlmSessionStatus::Active => Color32::LIGHT_GREEN,
        ploke_eval::walk_client::LlmSessionStatus::Paused => Color32::YELLOW,
        ploke_eval::walk_client::LlmSessionStatus::Terminal => Color32::LIGHT_BLUE,
        ploke_eval::walk_client::LlmSessionStatus::Abandoned => Color32::GRAY,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use egui_kittest::{Harness, kittest::Queryable};

    use super::*;

    #[test]
    fn live_observation_keeps_layers_and_exact_coordinates() {
        let loads = Arc::new(Mutex::new(Vec::new()));
        let first = Arc::clone(&loads);
        let index = typed_index();
        let mut selected = None;
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(1_200.0, 1_200.0))
            .build_ui(move |ui| {
                let action = LlmTracePanel {
                    index: Some(&index),
                    selected: &mut selected,
                    snapshot: None,
                    index_pending: false,
                    trace_pending: false,
                }
                .show(ui);
                if let Some(coordinate) = action.load {
                    first.lock().expect("load capture").push(coordinate);
                }
            });

        assert_eq!(harness.get_all_by_label("healthy-session").count(), 1);
        assert_eq!(
            harness
                .get_all_by_label("session corrupt-session: manifest invalid")
                .count(),
            1
        );
        assert_eq!(harness.get_all_by_label("debug status: Paused").count(), 1);
        assert_eq!(harness.get_all_by_label("next step: 3").count(), 1);

        harness.get_by_label("Load Session").click();
        harness.run();
        assert_eq!(
            loads.lock().expect("load capture").as_slice(),
            &[LlmTraceCoordinate {
                session_id: "healthy-session".to_string(),
                step: None,
            }]
        );

        let second = Arc::clone(&loads);
        let index = typed_index();
        let snapshot = typed_snapshot();
        let mut selected = Some("healthy-session".to_string());
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(1_200.0, 1_600.0))
            .build_ui(move |ui| {
                let action = LlmTracePanel {
                    index: Some(&index),
                    selected: &mut selected,
                    snapshot: Some(&snapshot),
                    index_pending: false,
                    trace_pending: false,
                }
                .show(ui);
                if let Some(coordinate) = action.load {
                    second.lock().expect("load capture").push(coordinate);
                }
            });

        assert!(harness.get_all_by_label("debug status: Paused").count() >= 2);
        assert!(harness.get_all_by_label("next step: 3").count() >= 2);
        assert_eq!(
            harness.get_all_by_label("OUTER EXECUTOR EVIDENCE").count(),
            1
        );
        assert_eq!(
            harness.get_all_by_label("Outer headless terminal").count(),
            1
        );
        assert_eq!(
            harness
                .get_all_by_label("Agent-turn summary terminal")
                .count(),
            1
        );

        harness.get_by_label("#2 · content").click();
        harness.run();
        assert_eq!(
            loads.lock().expect("load capture").as_slice(),
            &[
                LlmTraceCoordinate {
                    session_id: "healthy-session".to_string(),
                    step: None,
                },
                LlmTraceCoordinate {
                    session_id: "healthy-session".to_string(),
                    step: Some(2),
                },
            ]
        );
    }

    fn typed_index() -> LlmTraceIndex {
        serde_json::from_value(serde_json::json!({
            "version": version(),
            "epoch": epoch(),
            "campaign": "panel-campaign",
            "root": "/tmp/panel-campaign/tool-loop",
            "authority": "tool_loop_checkpoint",
            "lanes": [{
                "lane_id": "lane-a",
                "sessions": [{
                    "session": session(),
                    "resume": resume()
                }]
            }],
            "issues": [{
                "state": "invalid",
                "session_id": "corrupt-session",
                "source": source("tool_loop_session", "/tmp/corrupt-session/session.json", "bad0"),
                "detail": "unexpected end of JSON"
            }]
        }))
        .expect("typed LLM index")
    }

    fn typed_snapshot() -> LlmTraceSnapshot {
        serde_json::from_value(serde_json::json!({
            "coordinate": {
                "session_id": "healthy-session"
            },
            "version": version(),
            "epoch": epoch(),
            "authority": "tool_loop_checkpoint",
            "session": session(),
            "resume": resume(),
            "timeline": [
                {
                    "state": "present",
                    "summary": {
                        "value": {
                            "step": 2,
                            "outcome": {
                                "kind": "content",
                                "content_preview": "checkpoint response"
                            },
                            "tool_requests": 0,
                            "tool_results": 0,
                            "terminal": false
                        },
                        "source": source("tool_loop_step", "/tmp/healthy-session/steps/2.json", "2222")
                    }
                },
                {
                    "state": "unreadable",
                    "step": 3,
                    "path": "/tmp/healthy-session/steps/3.json",
                    "detail": "permission denied"
                }
            ],
            "selected": {
                "state": "unreadable",
                "path": "/tmp/healthy-session/steps/2.json",
                "detail": "changed during observation"
            },
            "outer": {
                "state": "present",
                "evidence": {
                    "value": {
                        "headless": {
                            "state": "unreadable",
                            "path": "/tmp/lane-a.headless-tui.json",
                            "detail": "changed during observation"
                        },
                        "turn": {
                            "state": "missing",
                            "path": "/tmp/lane-a.agent-turn-summary.json"
                        }
                    },
                    "source": source("harness_request", "/tmp/lane-a.json", "aaaa")
                }
            }
        }))
        .expect("typed LLM snapshot")
    }

    fn session() -> serde_json::Value {
        serde_json::json!({
            "value": {
                "session_id": "healthy-session",
                "lane_id": "lane-a",
                "workspace": "/tmp/healthy-workspace",
                "model": "google/gemini-test",
                "status": "paused"
            },
            "source": source("tool_loop_session", "/tmp/healthy-session/session.json", "1111")
        })
    }

    fn resume() -> serde_json::Value {
        serde_json::json!({
            "state": "present",
            "evidence": {
                "value": {
                    "next_step": 3,
                    "assistant_message_id": "assistant-1",
                    "parent_id": "parent-1",
                    "request_id": "request-1",
                    "attempts": 1,
                    "terminal": false
                },
                "source": source("tool_loop_resume", "/tmp/healthy-session/resume.json", "3333")
            }
        })
    }

    fn source(kind: &str, path: &str, hash: &str) -> serde_json::Value {
        serde_json::json!({
            "kind": kind,
            "path": path,
            "content_sha256": hash
        })
    }

    fn version() -> serde_json::Value {
        serde_json::json!({
            "session_id": null,
            "cursor": null,
            "journal_revision": 17
        })
    }

    fn epoch() -> serde_json::Value {
        serde_json::json!({
            "protocol_version": 10,
            "transition_graph_version": "walk-r0-r14a-v2",
            "repo_root": "/tmp/ploke",
            "exe_path": "/tmp/ploke-eval",
            "exe_modified_unix_ms": 17,
            "git_head": "abc123",
            "active_branch": "feature/ploke-loop",
            "source_status_hash": "def456"
        })
    }
}
