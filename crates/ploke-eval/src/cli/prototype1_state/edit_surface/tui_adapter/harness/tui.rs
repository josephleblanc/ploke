//! Headless `ploke-tui` [`Harness`] implementation.

use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc::Receiver},
    time::Instant,
};

use ploke_llm::manager::RecordedResponse;
use uuid::Uuid;

use super::super::super::harness_request::contract;
use super::super::super::surface_policy::SurfacePolicy;
use super::super::harness_io::{
    Event, Feedback, HeadlessAttempt, HeadlessAttemptResult, HeadlessRun, HeadlessTerminal,
    Outcome, PromptDiagnostic, Tool, observe_cargo_validation, push_changed_paths, truncate_chars,
};
use super::super::tui_bridge::{
    AppliedItem, AttemptEnd, Candidate, LiveObserver, StagedItem, ToolBatch,
    applied_edit_from_terminal_items, approve_selected, candidate_for_item,
    classify_applied_terminal, classify_paths, drain_debug_observed, drain_response_records,
    next_event_with_deadline, observe_staged_item, provider_failure_from_chat,
    provider_failure_from_message, provider_unavailable_reason, record_batch_terminal,
    record_post_approval_indeterminate, reject_item, repair_prompt_feedback,
    run_contract_validations, select_disjoint, turn_aborted_after_apply_terminal,
    validate_applied_batch, wait_for_refresh, wait_for_selected,
};
use super::{
    Batch, Decision, DenyItem, Error, Harness, Progress, PromptInfo, SessionSpec, Settled, Staged,
    StagedKind, ToolTrace, TurnStop,
};

pub(crate) struct TuiHarness {
    runtime: crate::runner::WorkspaceTuiRuntime,
    run: HeadlessRun,
    spec: SessionSpec,
    active_parent_id: Uuid,
    turn: u32,
    validation_commands: Vec<contract::Command>,
    response_rx: Option<Arc<Mutex<Receiver<RecordedResponse>>>>,
    observer: LiveObserver,
    pending_retry: Option<String>,
    provider_failure: Option<String>,
    applied: Vec<AppliedItem>,
    changed_paths: Vec<PathBuf>,
    policy_feedbacks: Vec<String>,
    batches: HashMap<Uuid, ToolBatch>,
    tool_requests: HashMap<String, (String, String)>,
    pending_events: VecDeque<ploke_tui::AppEvent>,
    awaiting_decision: bool,
}

impl TuiHarness {
    pub(crate) fn attach(
        runtime: crate::runner::WorkspaceTuiRuntime,
        run: HeadlessRun,
        spec: SessionSpec,
        active_parent_id: Uuid,
        turn: u32,
        validation_commands: Vec<contract::Command>,
        response_rx: Option<Arc<Mutex<Receiver<RecordedResponse>>>>,
        observer: LiveObserver,
    ) -> Self {
        Self {
            runtime,
            run,
            spec,
            active_parent_id,
            turn,
            validation_commands,
            response_rx,
            observer,
            pending_retry: None,
            provider_failure: None,
            applied: Vec::new(),
            changed_paths: Vec::new(),
            policy_feedbacks: Vec::new(),
            batches: HashMap::new(),
            tool_requests: HashMap::new(),
            pending_events: VecDeque::new(),
            awaiting_decision: false,
        }
    }

    pub(crate) fn applied_items(&self) -> &[AppliedItem] {
        &self.applied
    }

    pub(crate) fn changed_paths(&self) -> &[PathBuf] {
        &self.changed_paths
    }

    pub(crate) fn take_pending_retry(&mut self) -> Option<String> {
        self.pending_retry.take()
    }

    pub(crate) fn policy_feedbacks(&self) -> &[String] {
        &self.policy_feedbacks
    }

    pub(crate) async fn drive_to_attempt_end(
        &mut self,
        surface: &SurfacePolicy,
    ) -> Result<AttemptEnd, Error> {
        let started = Instant::now();
        loop {
            let deadline = self.spec.timeouts.attempt_deadline(started);
            match self.next(deadline).await? {
                Progress::PendingEdit(batch) => {
                    let decision = surface_decision(&batch, &self.spec.workspace_path, surface);
                    let newly_applied = self.decide(decision).await?;
                    if newly_applied {
                        let settle_deadline = self.spec.timeouts.phase_deadline(
                            deadline,
                            self.spec.timeouts.post_apply_index_duration(),
                        );
                        self.settle(settle_deadline).await?;
                        if !self.validation_commands.is_empty() {
                            if let Some(terminal) = validate_applied_batch(
                                &self.runtime,
                                self.active_parent_id,
                                batch.request_id,
                                self.turn,
                                &mut self.run,
                                &self.observer,
                                &self.validation_commands,
                                &self.applied,
                                &self.changed_paths,
                            )
                            .await
                            {
                                if matches!(terminal, HeadlessTerminal::Applied { .. }) {
                                    self.observer.emit(format!(
                                        "attempt {} finalize_applied {}",
                                        self.turn,
                                        terminal.live_summary()
                                    ));
                                    return Ok(AttemptEnd::Terminal(terminal));
                                }
                                self.observer.emit(format!(
                                    "attempt {} applied_batch_validation_unsatisfied {}",
                                    self.turn,
                                    terminal.live_summary()
                                ));
                            }
                        }
                    }
                }
                Progress::ContextUnavailable(reason) => {
                    return Ok(AttemptEnd::Terminal(HeadlessTerminal::ContextUnavailable {
                        reason,
                    }));
                }
                Progress::ProviderUnavailable(reason) => {
                    return Ok(AttemptEnd::Terminal(
                        HeadlessTerminal::ProviderUnavailable { reason },
                    ));
                }
                Progress::TurnEnded(stop) => {
                    return classify_turn_stop(self, stop).await;
                }
                Progress::Prompt(_) | Progress::Tool(_) => {}
            }
        }
    }

    pub(crate) async fn finalize(&mut self) {
        self.runtime.app.pump_pending_events().await;
        drain_debug_observed(
            &mut self.runtime.debug_rx,
            &mut self.run,
            &self.observer,
            self.turn,
        );
    }

    pub(crate) fn into_parts(self) -> (HeadlessRun, crate::runner::WorkspaceTuiRuntime) {
        (self.run, self.runtime)
    }
}

impl Harness for TuiHarness {
    async fn next(&mut self, deadline: Instant) -> Result<Progress, Error> {
        if self.awaiting_decision {
            return Err(Error::HeadlessEvent(
                "harness is awaiting decide() before next event".to_string(),
            ));
        }

        use ploke_tui::{AppEvent, app_state::events::SystemEvent};

        loop {
            self.runtime.app.pump_pending_events().await;
            drain_debug_observed(
                &mut self.runtime.debug_rx,
                &mut self.run,
                &self.observer,
                self.turn,
            );

            let event = if let Some(event) = self.pending_events.pop_front() {
                event
            } else {
                next_event_with_deadline(
                    &mut self.runtime,
                    deadline,
                    &mut self.run,
                    &self.observer,
                    self.turn,
                )
                .await?
            };

            match event {
                AppEvent::Llm(ploke_tui::llm::LlmEvent::ChatCompletion(
                    ploke_tui::llm::ChatEvt::PromptConstructed {
                        parent_id,
                        formatted_prompt,
                        context_plan,
                    },
                )) if parent_id == self.active_parent_id => {
                    let diagnostic = PromptDiagnostic::capture(
                        &self.runtime.state,
                        parent_id,
                        &formatted_prompt,
                        &context_plan,
                    )
                    .await;
                    if let Some(reason) = diagnostic.context_unavailable_reason() {
                        return Ok(Progress::ContextUnavailable(reason));
                    }
                    self.run.prompt_diagnostics.push(diagnostic.clone());
                    return Ok(Progress::Prompt(PromptInfo { diagnostic }));
                }
                AppEvent::System(SystemEvent::ToolCallRequested {
                    request_id,
                    parent_id,
                    tool_call,
                }) if parent_id == self.active_parent_id => {
                    self.run.events.push(Event::ToolRequest {
                        request_id: request_id.to_string(),
                        parent_id: parent_id.to_string(),
                        call_id: tool_call.call_id.to_string(),
                        tool: tool_call.function.name.as_str().to_string(),
                        arguments: tool_call.function.arguments.clone(),
                    });
                    self.tool_requests.insert(
                        tool_call.call_id.to_string(),
                        (
                            tool_call.function.name.as_str().to_string(),
                            tool_call.function.arguments.clone(),
                        ),
                    );
                    self.observer.emit(format!(
                        "attempt {} tool_request call_id={} tool={} args={}",
                        self.turn,
                        tool_call.call_id,
                        tool_call.function.name.as_str(),
                        truncate_chars(&tool_call.function.arguments, 240)
                    ));
                    self.batches
                        .entry(request_id)
                        .or_default()
                        .request(tool_call.call_id.clone());
                }
                AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id,
                    parent_id,
                    call_id,
                    content,
                    ui_payload,
                    ..
                }) if parent_id == self.active_parent_id => {
                    self.run.events.push(Event::Tool {
                        call_id: call_id.to_string(),
                        result: Tool::Completed {
                            content: content.clone(),
                        },
                    });
                    let call_id_text = call_id.to_string();
                    if let Some((tool, arguments)) = self.tool_requests.get(&call_id_text)
                        && tool == "cargo"
                        && let Some(validation) = observe_cargo_validation(
                            &mut self.run,
                            &call_id_text,
                            arguments,
                            &content,
                        )
                    {
                        self.observer.emit(format!(
                            "attempt {} cargo_validation call_id={} ok={} status={} command={}",
                            self.turn,
                            call_id,
                            validation.ok,
                            validation.status_reason,
                            validation.display_command
                        ));
                    }
                    self.observer.emit(format!(
                        "attempt {} tool_completed call_id={} content={}",
                        self.turn,
                        call_id,
                        truncate_chars(&content, 240)
                    ));
                    let preview = truncate_chars(&content, 240);
                    let tool_name = self
                        .tool_requests
                        .get(call_id.as_ref())
                        .map(|(tool, _)| tool.clone())
                        .unwrap_or_default();
                    let staged = observe_staged_item(
                        &self.runtime,
                        ui_payload.as_ref(),
                        request_id,
                        &self.applied,
                        self.turn,
                        &mut self.run,
                        &self.observer,
                    )
                    .await;
                    let batch_ready = record_batch_terminal(
                        &mut self.batches,
                        request_id,
                        call_id.clone(),
                        staged,
                    );
                    if let Some(items) = batch_ready {
                        let batch = self.build_batch(request_id, items).await?;
                        self.awaiting_decision = true;
                        return Ok(Progress::PendingEdit(batch));
                    }
                    return Ok(Progress::Tool(ToolTrace {
                        call_id: call_id.to_string(),
                        tool: tool_name,
                        completed: true,
                        preview,
                    }));
                }
                AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id,
                    call_id,
                    error,
                    ..
                }) if parent_id == self.active_parent_id => {
                    self.run.events.push(Event::Tool {
                        call_id: call_id.to_string(),
                        result: Tool::Failed {
                            error: error.clone(),
                        },
                    });
                    self.observer.emit(format!(
                        "attempt {} tool_failed call_id={} error={}",
                        self.turn,
                        call_id,
                        truncate_chars(&error, 240)
                    ));
                    self.run.attempts.push(HeadlessAttempt {
                        turn: self.turn,
                        proposal_id: None,
                        result: HeadlessAttemptResult::ToolFailed {
                            error: error.clone(),
                        },
                    });
                    self.pending_retry = Some(error.clone());
                    let batch_ready =
                        record_batch_terminal(&mut self.batches, request_id, call_id.clone(), None);
                    if let Some(items) = batch_ready {
                        let batch = self.build_batch(request_id, items).await?;
                        self.awaiting_decision = true;
                        return Ok(Progress::PendingEdit(batch));
                    }
                    let tool_name = self
                        .tool_requests
                        .get(call_id.as_ref())
                        .map(|(tool, _)| tool.clone())
                        .unwrap_or_default();
                    return Ok(Progress::Tool(ToolTrace {
                        call_id: call_id.to_string(),
                        tool: tool_name,
                        completed: false,
                        preview: truncate_chars(&error, 240),
                    }));
                }
                AppEvent::MessageUpdated(message) => {
                    let error_message = {
                        let chat = self.runtime.state.chat.0.read().await;
                        chat.messages.get(&message.0).and_then(|message| {
                            if !matches!(
                                message.status,
                                ploke_tui::chat_history::MessageStatus::Error { .. }
                            ) {
                                return None;
                            }
                            let provider_reason = provider_failure_from_message(
                                message.kind,
                                &message.status,
                                &message.content,
                            );
                            Some((
                                message.id,
                                message.kind,
                                message.content.clone(),
                                provider_reason,
                            ))
                        })
                    };
                    let Some((message_id, kind, content, message_provider_failure)) = error_message
                    else {
                        continue;
                    };
                    if matches!(kind, ploke_tui::chat_history::MessageKind::Assistant) {
                        self.run.events.push(Event::AssistantMessage {
                            id: message_id.to_string(),
                            status: "error".to_string(),
                            content: content.clone(),
                        });
                        self.observer.emit(format!(
                            "attempt {} assistant_error id={} content={}",
                            self.turn,
                            message_id,
                            truncate_chars(&content, 240)
                        ));
                    } else {
                        self.observer.emit(format!(
                            "attempt {} message_error kind={} id={} content={}",
                            self.turn,
                            kind,
                            message_id,
                            truncate_chars(&content, 240)
                        ));
                    }
                    if self.provider_failure.is_none() {
                        self.provider_failure = message_provider_failure;
                    }
                }
                AppEvent::System(SystemEvent::ChatTurnFinished {
                    session_id,
                    request_id,
                    parent_id,
                    assistant_message_id,
                    outcome,
                    error_id,
                    attempts,
                    summary,
                }) if parent_id == self.active_parent_id => {
                    self.run.events.push(Event::Turn {
                        session_id: session_id.to_string(),
                        request_id: request_id.to_string(),
                        parent_id: parent_id.to_string(),
                        assistant_message_id: assistant_message_id.to_string(),
                        outcome: outcome.clone(),
                        error_id: error_id.map(|id| id.to_string()),
                        attempts,
                        summary: summary.clone(),
                    });
                    if let Some(response_rx) = self.response_rx.as_ref() {
                        drain_response_records(
                            &mut self.run,
                            assistant_message_id,
                            Some(response_rx),
                        );
                    }
                    self.observer.emit(format!(
                        "attempt {} turn_finished outcome={} attempts={} summary={}",
                        self.turn,
                        outcome,
                        attempts,
                        truncate_chars(&summary, 240)
                    ));
                    if self.provider_failure.is_none() {
                        self.provider_failure = provider_failure_from_chat(&self.runtime).await;
                    }
                    if self.provider_failure.is_none() {
                        self.provider_failure = provider_unavailable_reason(&summary);
                    }
                    if let Some(reason) = self.provider_failure.take() {
                        return Ok(Progress::ProviderUnavailable(reason));
                    }
                    return Ok(Progress::TurnEnded(TurnStop {
                        outcome,
                        summary,
                        attempts,
                        request_id,
                    }));
                }
                _ => {}
            }
        }
    }

    async fn decide(&mut self, decision: Decision) -> Result<bool, Error> {
        if !self.awaiting_decision {
            return Err(Error::HeadlessEvent(
                "decide() called without a pending edit batch".to_string(),
            ));
        }
        self.awaiting_decision = false;

        let workspace_path = self.spec.workspace_path.as_path();
        let mut candidates = Vec::new();

        for deny in &decision.deny {
            let item = staged_to_item(&deny.item);
            reject_item_with_reason(
                &self.runtime,
                item,
                self.turn,
                &mut self.run,
                &self.observer,
                deny.reason.clone(),
            )
            .await?;
            self.policy_feedbacks
                .push(repair_prompt_feedback(&deny.reason));
        }

        for staged in &decision.approve {
            let item = staged_to_item(staged);
            if self.applied.contains(&item.applied()) {
                continue;
            }
            let Some(candidate) = candidate_for_item(&self.runtime, item).await else {
                let reason = format!("staged proposal {} disappeared before admission", item.id());
                reject_item_with_reason(
                    &self.runtime,
                    item,
                    self.turn,
                    &mut self.run,
                    &self.observer,
                    reason.clone(),
                )
                .await?;
                self.pending_retry = Some(reason);
                continue;
            };
            if candidate.paths.is_empty() {
                let reason = match item {
                    StagedItem::Edit(_) => {
                        "No material edit was staged; make a concrete bounded edit."
                    }
                    StagedItem::Create(_) => {
                        "No material file creation was staged; make a concrete bounded edit."
                    }
                }
                .to_string();
                reject_item_with_reason(
                    &self.runtime,
                    item,
                    self.turn,
                    &mut self.run,
                    &self.observer,
                    reason.clone(),
                )
                .await?;
                self.policy_feedbacks.push(repair_prompt_feedback(&reason));
                continue;
            }
            candidates.push(candidate);
        }

        let (selected, rejected) = select_disjoint(candidates, workspace_path);
        for candidate in rejected {
            let reason =
                "Staged edit overlaps a newer valid proposal from the same tool batch".to_string();
            reject_item_with_reason(
                &self.runtime,
                candidate.item,
                self.turn,
                &mut self.run,
                &self.observer,
                reason.clone(),
            )
            .await?;
            self.policy_feedbacks.push(repair_prompt_feedback(&reason));
        }

        if selected.is_empty() {
            return Ok(false);
        }

        approve_selected(&self.runtime, self.turn, &self.observer, &selected).await?;
        let applied_outcome = match wait_for_selected(
            &mut self.runtime,
            self.turn,
            &mut self.run,
            &self.observer,
            &selected,
            &self.spec.timeouts,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                record_post_approval_indeterminate(
                    &mut self.run,
                    self.turn,
                    &self.observer,
                    &selected,
                    &error.to_string(),
                );
                return Err(error);
            }
        };
        let newly_applied = !applied_outcome.applied.is_empty();
        self.applied.extend(applied_outcome.applied);
        push_changed_paths(&mut self.changed_paths, applied_outcome.changed_paths);
        if let Some(feedback) = applied_outcome.retry {
            self.pending_retry = Some(feedback);
        }
        Ok(newly_applied)
    }

    async fn settle(&mut self, deadline: Instant) -> Result<Settled, Error> {
        if self.applied.is_empty() && self.changed_paths.is_empty() {
            return Ok(Settled {
                applied: Vec::new(),
                changed_paths: Vec::new(),
            });
        }
        wait_for_refresh(
            &mut self.runtime,
            &mut self.pending_events,
            self.turn,
            &self.observer,
            deadline,
            &self.spec.timeouts,
            &self.changed_paths,
        )
        .await?;
        Ok(Settled {
            applied: self.applied.iter().map(|item| item.id()).collect(),
            changed_paths: self.changed_paths.clone(),
        })
    }

    fn run(&self) -> &HeadlessRun {
        &self.run
    }

    fn run_mut(&mut self) -> &mut HeadlessRun {
        &mut self.run
    }
}

impl TuiHarness {
    async fn build_batch(&self, request_id: Uuid, items: Vec<StagedItem>) -> Result<Batch, Error> {
        let mut staged = Vec::new();
        for item in items {
            let Some(candidate) = candidate_for_item(&self.runtime, item).await else {
                continue;
            };
            staged.push(Staged {
                id: item.id(),
                kind: match item {
                    StagedItem::Edit(_) => StagedKind::Edit,
                    StagedItem::Create(_) => StagedKind::Create,
                },
                paths: candidate.paths,
                proposed_at_ms: candidate.proposed_at_ms,
            });
        }
        Ok(Batch { request_id, staged })
    }
}

pub(crate) fn surface_decision(
    batch: &Batch,
    workspace_path: &Path,
    surface: &SurfacePolicy,
) -> Decision {
    let mut approve = Vec::new();
    let mut deny = Vec::new();
    for staged in &batch.staged {
        if staged.paths.is_empty() {
            let reason = match staged.kind {
                StagedKind::Edit => {
                    "No material edit was staged; make a concrete bounded edit.".to_string()
                }
                StagedKind::Create => {
                    "No material file creation was staged; make a concrete bounded edit."
                        .to_string()
                }
            };
            deny.push(DenyItem {
                item: staged.clone(),
                reason,
            });
            continue;
        }
        if let Some(rejection) = classify_paths(workspace_path, &surface, &staged.paths) {
            let feedback = Feedback::from_outcome(&Outcome::Rejected(rejection));
            deny.push(DenyItem {
                item: staged.clone(),
                reason: feedback.message().to_string(),
            });
            continue;
        }
        approve.push(staged.clone());
    }
    Decision {
        request_id: batch.request_id,
        approve,
        deny,
    }
}

fn staged_to_item(staged: &Staged) -> StagedItem {
    match staged.kind {
        StagedKind::Edit => StagedItem::Edit(staged.id),
        StagedKind::Create => StagedItem::Create(staged.id),
    }
}

async fn reject_item_with_reason(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    item: StagedItem,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    reason: String,
) -> Result<(), Error> {
    reject_item(runtime, item, turn, run, observer, reason).await
}

async fn classify_turn_stop(harness: &mut TuiHarness, stop: TurnStop) -> Result<AttemptEnd, Error> {
    let TurnStop {
        outcome,
        summary,
        attempts: _,
        request_id,
    } = stop;
    let repaired_failure = harness.take_pending_retry();
    if outcome != "completed" {
        if let Some(applied_edit) =
            applied_edit_from_terminal_items(harness.applied_items(), harness.changed_paths())
        {
            return Ok(AttemptEnd::Terminal(turn_aborted_after_apply_terminal(
                applied_edit,
                outcome,
                summary,
            )));
        }
        let feedback = if let Some(feedback) = repaired_failure {
            feedback
        } else if summary.trim().is_empty() {
            format!(
                "The model turn ended with outcome `{outcome}` before completing the candidate."
            )
        } else {
            summary.clone()
        };
        return Ok(AttemptEnd::RetryFailure(feedback));
    }

    if harness.applied_items().is_empty() {
        let feedback = if let Some(feedback) = repaired_failure {
            feedback
        } else if !harness.policy_feedbacks().is_empty() {
            harness.policy_feedbacks().join("\n")
        } else if summary.trim().is_empty() {
            "The model returned without staging an edit; make a concrete bounded edit.".to_string()
        } else {
            summary.clone()
        };
        let turn = harness.turn;
        harness.run_mut().attempts.push(HeadlessAttempt {
            turn,
            proposal_id: None,
            result: HeadlessAttemptResult::NoEdit {
                summary: feedback.clone(),
            },
        });
        return Ok(AttemptEnd::RetryNoEdit {
            feedback,
            outcome,
            summary,
        });
    }

    if !harness.validation_commands.is_empty() {
        let active_parent_id = harness.active_parent_id;
        let turn = harness.turn;
        let validation_commands = harness.validation_commands.clone();
        let observer = harness.observer;
        run_contract_validations(
            &harness.runtime,
            active_parent_id,
            request_id,
            turn,
            &mut harness.run,
            &observer,
            &validation_commands,
        )
        .await;
    }

    let applied_edit =
        applied_edit_from_terminal_items(harness.applied_items(), harness.changed_paths())
            .expect("applied is not empty");
    Ok(AttemptEnd::Terminal(classify_applied_terminal(
        harness.run(),
        &harness.validation_commands,
        request_id,
        applied_edit,
    )))
}
