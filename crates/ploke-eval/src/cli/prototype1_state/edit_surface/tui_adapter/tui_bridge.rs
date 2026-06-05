//! Headless `ploke-tui` executor bridge for broad edit attempts.

use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

use ploke_llm::{manager::RecordedResponse, router_only::RouterVariants};
use ploke_records::llm_response::RawFullResponseRecord;
use ploke_tui::app::commands::harness::TestAppAccessor;
use serde::Deserialize;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::super::harness_request::{
    BroadEditPolicy, EvidenceRoot, EvidenceRootKind, EvidenceRootLocation, contract,
};
use super::{
    Budget, Error, LIVE_TRACE_ENV, ModelSelection, POST_APPLY_INDEX_START_GRACE_MS,
    POST_APPLY_INDEX_TIMEOUT_SECS, POST_APPLY_STATUS_TIMEOUT_SECS,
    harness_io::{
        AppliedEdit, CargoValidationObservation, Event, Feedback, HeadlessAttempt,
        HeadlessAttemptResult, HeadlessRun, HeadlessTerminal, Outcome, PromptDiagnostic, Reject,
        Tool, join_paths, latest_failed_cargo_validation_feedback, observe_cargo_validation,
        observed_headless_error, push_changed_paths, truncate_chars,
    },
};

pub(crate) async fn run_headless(
    workspace_path: &Path,
    prompt: &str,
    budget: Budget,
    edit_policy: BroadEditPolicy,
    evidence_roots: &[EvidenceRoot],
) -> Result<HeadlessRun, Error> {
    run_headless_with_model(
        workspace_path,
        prompt,
        budget,
        edit_policy,
        evidence_roots,
        None,
    )
    .await
}

pub(crate) async fn run_headless_with_model(
    workspace_path: &Path,
    prompt: &str,
    budget: Budget,
    edit_policy: BroadEditPolicy,
    evidence_roots: &[EvidenceRoot],
    model: Option<ModelSelection>,
) -> Result<HeadlessRun, Error> {
    run_headless_with_model_inner(
        workspace_path,
        prompt,
        budget,
        edit_policy,
        evidence_roots,
        &[],
        model,
        None,
    )
    .await
}

pub(crate) async fn run_headless_with_model_capture_responses(
    workspace_path: &Path,
    prompt: &str,
    budget: Budget,
    edit_policy: BroadEditPolicy,
    evidence_roots: &[EvidenceRoot],
    validation_commands: &[contract::Command],
    model: Option<ModelSelection>,
) -> Result<HeadlessRun, Error> {
    let (response_tx, response_rx) = std::sync::mpsc::channel();
    let response_rx = Mutex::new(response_rx);
    let _response_tap_guard = ploke_tui::llm::install_response_tap(response_tx);
    run_headless_with_model_inner(
        workspace_path,
        prompt,
        budget,
        edit_policy,
        evidence_roots,
        validation_commands,
        model,
        Some(&response_rx),
    )
    .await
}

async fn run_headless_with_model_inner(
    workspace_path: &Path,
    prompt: &str,
    budget: Budget,
    edit_policy: BroadEditPolicy,
    evidence_roots: &[EvidenceRoot],
    validation_commands: &[contract::Command],
    model: Option<ModelSelection>,
    response_rx: Option<&Mutex<Receiver<RecordedResponse>>>,
) -> Result<HeadlessRun, Error> {
    let mut run = HeadlessRun::new();
    run.model_route = model.as_ref().map(ModelSelection::model_route_record);
    let mut turn = 1_u32;
    let extra_read_roots = evidence_read_roots(evidence_roots);
    let mut next_prompt = attempt_prompt(workspace_path, edit_policy, evidence_roots, prompt, None);
    let observer = LiveObserver::from_env();
    observer.emit(format!(
        "start workspace={} max_attempts={} timeout_secs={} evidence_read_roots={}",
        workspace_path.display(),
        budget.max_attempts(),
        budget.timeout_secs(),
        extra_read_roots.len()
    ));
    observer.emit_workspace_size("workspace_start", workspace_path);

    let outcome = tokio::time::timeout(Duration::from_secs(budget.timeout_secs()), async {
        loop {
            observer.emit(format!("attempt {turn} start"));
            let (mut runtime, parent_id) = start_attempt_runtime(
                workspace_path,
                &extra_read_roots,
                next_prompt.clone(),
                edit_policy,
                model.as_ref(),
            )
            .await?;
            // TODO: This doesn't need to take &mut runtime, since it drops it
            // right after anyways.
            let end = run_attempt(
                &mut runtime,
                parent_id,
                workspace_path,
                edit_policy,
                turn,
                &mut run,
                &observer,
                validation_commands,
                response_rx,
            )
            .await?;
            // TODO: Should happen inside `run_attempt`
            runtime.app.pump_pending_events().await;
            drain_debug(&mut runtime.debug_rx, &mut run);
            drop(runtime);

            match end {
                AttemptEnd::Terminal(terminal) => {
                    observer.emit(format!("terminal {}", terminal.live_summary()));
                    return Ok::<HeadlessTerminal, Error>(terminal);
                }
                AttemptEnd::RetryFailure(feedback) => {
                    let feedback = retry_feedback(&feedback);
                    if !advance_turn(&budget, &mut turn) {
                        observer.emit(format!(
                            "terminal exhausted attempts={turn} last={}",
                            truncate_chars(&feedback, 240)
                        ));
                        return Ok::<HeadlessTerminal, Error>(HeadlessTerminal::Exhausted {
                            attempts: turn,
                            last: feedback,
                        });
                    }
                    observer.emit(format!("retry attempt={turn} feedback={}", feedback,));
                    next_prompt = attempt_prompt(
                        workspace_path,
                        edit_policy,
                        evidence_roots,
                        prompt,
                        Some(&feedback),
                    );
                }
                AttemptEnd::RetryNoEdit {
                    feedback,
                    outcome,
                    summary,
                } => {
                    let feedback = retry_feedback(&feedback);
                    if !advance_turn(&budget, &mut turn) {
                        observer.emit(format!(
                            "terminal completed_without_edit outcome={} summary={}",
                            outcome,
                            truncate_chars(&summary, 240)
                        ));
                        return Ok::<HeadlessTerminal, Error>(
                            HeadlessTerminal::CompletedWithoutEdit { outcome, summary },
                        );
                    }
                    observer.emit(format!(
                        "retry attempt={turn} no_edit_feedback={}",
                        truncate_chars(&feedback, 240)
                    ));
                    next_prompt = attempt_prompt(
                        workspace_path,
                        edit_policy,
                        evidence_roots,
                        prompt,
                        Some(&feedback),
                    );
                }
            }
        }
    })
    .await;

    let terminal = match outcome {
        Ok(Ok(terminal)) => terminal,
        Ok(Err(source)) => {
            if !run.has_observed_activity() {
                return Err(source);
            }
            HeadlessTerminal::ToolFailed {
                error: observed_headless_error(source),
            }
        }
        Err(_) => timeout_terminal_for_run(&run, budget.timeout_secs()),
    };
    observer.emit(format!("done {}", terminal.live_summary()));
    observer.emit_workspace_size("workspace_done", workspace_path);
    run.terminal = Some(terminal);
    Ok(run)
}

pub(super) async fn start_attempt_runtime(
    workspace_path: &Path,
    extra_read_roots: &[PathBuf],
    prompt: String,
    edit_policy: BroadEditPolicy,
    model: Option<&ModelSelection>,
) -> Result<(crate::runner::WorkspaceTuiRuntime, Uuid), Error> {
    let runtime = crate::runner::setup_workspace_tui_runtime_with_read_roots(
        workspace_path,
        extra_read_roots,
    )
    .await
    .map_err(|source| Error::HeadlessStart(source.to_string()))?;

    let write_scope = write_scope_for_policy(edit_policy);
    runtime
        .state
        .with_system_txn(|txn| txn.set_write_scope(Some(write_scope)))
        .await;

    let cmd_tx = runtime.app.state_cmd_tx();
    send_state(
        &cmd_tx,
        ploke_tui::app_state::StateCommand::SetEditingAutoConfirm { enabled: false },
    )
    .await?;
    {
        let mut cfg = runtime.state.config.write().await;
        cfg.context_management.mode = ploke_tui::user_config::CtxMode::Off;
        if let Some(model) = model {
            cfg.active_model = model.model_id.clone();
            cfg.active_router = model.router();
            if !matches!(model.router(), RouterVariants::Google(_)) || model.provider.is_some() {
                cfg.model_registry
                    .select_model_provider(&model.model_id, model.provider.as_ref());
            }
        }
    }
    let parent_id = submit_prompt(&runtime.app, prompt).await?;
    Ok((runtime, parent_id))
}

fn write_scope_for_policy(
    edit_policy: BroadEditPolicy,
) -> ploke_tui::utils::path_scoping::WriteScope {
    use crate::cli::prototype1_state::backend::{
        WORKSPACE_EXCEPT_AUTHORITY_FILENAMES, WORKSPACE_EXCEPT_AUTHORITY_PREFIXES,
    };

    match edit_policy {
        BroadEditPolicy::WorkspaceExceptPlokeEval => {
            ploke_tui::utils::path_scoping::WriteScope::new()
                .deny_prefixes(
                    WORKSPACE_EXCEPT_AUTHORITY_PREFIXES
                        .iter()
                        .map(PathBuf::from),
                )
                .deny_filenames(
                    WORKSPACE_EXCEPT_AUTHORITY_FILENAMES
                        .iter()
                        .map(|name| (*name).to_string()),
                )
        }
    }
}

#[derive(Debug)]
pub(super) enum AttemptEnd {
    Terminal(HeadlessTerminal),
    RetryFailure(String),
    RetryNoEdit {
        feedback: String,
        outcome: String,
        summary: String,
    },
}

const MAX_POLICY_REPAIR_TURNS: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AppliedItem {
    Edit(Uuid),
    Create(Uuid),
}

impl AppliedItem {
    pub(super) fn id(self) -> Uuid {
        match self {
            Self::Edit(id) | Self::Create(id) => id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum StagedItem {
    Edit(Uuid),
    Create(Uuid),
}

impl StagedItem {
    pub(super) fn id(self) -> Uuid {
        match self {
            Self::Edit(id) | Self::Create(id) => id,
        }
    }

    fn applied(self) -> AppliedItem {
        match self {
            Self::Edit(id) => AppliedItem::Edit(id),
            Self::Create(id) => AppliedItem::Create(id),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct ToolBatch {
    expected: Vec<ploke_core::ArcStr>,
    completed: Vec<ploke_core::ArcStr>,
    staged: Vec<StagedItem>,
}

impl ToolBatch {
    pub(super) fn request(&mut self, call_id: ploke_core::ArcStr) {
        if !self.expected.contains(&call_id) {
            self.expected.push(call_id);
        }
    }

    fn complete(&mut self, call_id: ploke_core::ArcStr, staged: Option<StagedItem>) {
        if !self.completed.contains(&call_id) {
            self.completed.push(call_id);
        }
        if let Some(staged) = staged {
            if !self.staged.contains(&staged) {
                self.staged.push(staged);
            }
        }
    }

    fn is_complete(&self) -> bool {
        !self.expected.is_empty()
            && self
                .expected
                .iter()
                .all(|call_id| self.completed.contains(call_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    pub(super) item: StagedItem,
    pub(super) proposed_at_ms: i64,
    pub(super) paths: Vec<PathBuf>,
}

#[derive(Debug, Default)]
struct BatchOutcome {
    applied: Vec<AppliedItem>,
    changed_paths: Vec<PathBuf>,
    mutated: bool,
    feedbacks: Vec<String>,
    retry: Option<String>,
}

// TODO: I think this actually wants to be a method of `HeadlessRun`
pub(super) async fn run_attempt(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    active_parent_id: Uuid,
    workspace_path: &Path,
    edit_policy: BroadEditPolicy,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    validation_commands: &[contract::Command],
    response_rx: Option<&Mutex<Receiver<RecordedResponse>>>,
) -> Result<AttemptEnd, Error> {
    use ploke_tui::{AppEvent, app_state::events::SystemEvent};

    let mut pending_retry = None::<String>;
    let mut provider_failure = None::<String>;
    let mut applied = Vec::<AppliedItem>::new();
    let mut changed_paths = Vec::<PathBuf>::new();
    let mut policy_feedbacks = Vec::<String>::new();
    let _policy_repair_turns = 0_u32;
    let mut batches = HashMap::<Uuid, ToolBatch>::new();
    let mut tool_requests = HashMap::<String, (String, String)>::new();
    let mut pending_events = VecDeque::<ploke_tui::AppEvent>::new();

    loop {
        runtime.app.pump_pending_events().await;
        drain_debug_observed(&mut runtime.debug_rx, run, observer, turn);

        let event = if let Some(event) = pending_events.pop_front() {
            event
        } else {
            next_event(runtime).await?
        };

        match event {
            AppEvent::Llm(ploke_tui::llm::LlmEvent::ChatCompletion(
                ploke_tui::llm::ChatEvt::PromptConstructed {
                    parent_id,
                    formatted_prompt,
                    context_plan,
                },
            )) if parent_id == active_parent_id => {
                let diagnostic = PromptDiagnostic::capture(
                    &runtime.state,
                    parent_id,
                    &formatted_prompt,
                    &context_plan,
                )
                .await;
                let context_unavailable = diagnostic.context_unavailable_reason();
                observer.emit(format!(
                    "attempt {turn} prompt parent={} messages={} estimated_tokens={} rag_parts={} bm25={}",
                    diagnostic.parent_id,
                    diagnostic.message_count,
                    diagnostic.estimated_total_tokens,
                    diagnostic.included_rag_parts,
                    diagnostic
                        .bm25
                        .as_ref()
                        .map(|bm25| bm25.status.as_str())
                        .unwrap_or("none")
                ));
                run.prompt_diagnostics.push(diagnostic);
                if let Some(reason) = context_unavailable {
                    observer.emit(format!(
                        "attempt {turn} context_unavailable {}",
                        truncate_chars(&reason, 240)
                    ));
                    return Ok(AttemptEnd::Terminal(HeadlessTerminal::ContextUnavailable {
                        reason,
                    }));
                }
            }
            AppEvent::System(SystemEvent::ToolCallRequested {
                request_id,
                parent_id,
                tool_call,
            }) if parent_id == active_parent_id => {
                run.events.push(Event::ToolRequest {
                    request_id: request_id.to_string(),
                    parent_id: parent_id.to_string(),
                    call_id: tool_call.call_id.to_string(),
                    tool: tool_call.function.name.as_str().to_string(),
                    arguments: tool_call.function.arguments.clone(),
                });
                tool_requests.insert(
                    tool_call.call_id.to_string(),
                    (
                        tool_call.function.name.as_str().to_string(),
                        tool_call.function.arguments.clone(),
                    ),
                );
                observer.emit(format!(
                    "attempt {turn} tool_request call_id={} tool={} args={}",
                    tool_call.call_id,
                    tool_call.function.name.as_str(),
                    truncate_chars(&tool_call.function.arguments, 240)
                ));
                batches
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
            }) if parent_id == active_parent_id => {
                run.events.push(Event::Tool {
                    call_id: call_id.to_string(),
                    result: Tool::Completed {
                        content: content.clone(),
                    },
                });
                let call_id_text = call_id.to_string();
                if let Some((tool, arguments)) = tool_requests.get(&call_id_text)
                    && tool == "cargo"
                    && let Some(validation) =
                        observe_cargo_validation(run, &call_id_text, arguments, &content)
                {
                    observer.emit(format!(
                        "attempt {turn} cargo_validation call_id={} ok={} status={} command={}",
                        call_id,
                        validation.ok,
                        validation.status_reason,
                        validation.display_command
                    ));
                }
                observer.emit(format!(
                    "attempt {turn} tool_completed call_id={} content={}",
                    call_id,
                    truncate_chars(&content, 240)
                ));
                let staged = observe_staged_item(
                    runtime,
                    ui_payload.as_ref(),
                    request_id,
                    &applied,
                    turn,
                    run,
                    observer,
                )
                .await;
                let batch_ready =
                    record_batch_terminal(&mut batches, request_id, call_id.clone(), staged);
                if let Some(items) = batch_ready {
                    let outcome = settle_staged_batch(
                        runtime,
                        &mut pending_events,
                        workspace_path,
                        edit_policy,
                        turn,
                        run,
                        observer,
                        items,
                        &applied,
                    )
                    .await?;
                    applied.extend(outcome.applied);
                    push_changed_paths(&mut changed_paths, outcome.changed_paths);
                    policy_feedbacks.extend(outcome.feedbacks);
                    if let Some(feedback) = outcome.retry {
                        pending_retry = Some(feedback);
                    }
                }
            }
            AppEvent::System(SystemEvent::ToolCallFailed {
                request_id,
                parent_id,
                call_id,
                error,
                ..
            }) if parent_id == active_parent_id => {
                run.events.push(Event::Tool {
                    call_id: call_id.to_string(),
                    result: Tool::Failed {
                        error: error.clone(),
                    },
                });
                observer.emit(format!(
                    "attempt {turn} tool_failed call_id={} error={}",
                    call_id,
                    truncate_chars(&error, 240)
                ));
                run.attempts.push(HeadlessAttempt {
                    turn,
                    proposal_id: None,
                    result: HeadlessAttemptResult::ToolFailed {
                        error: error.clone(),
                    },
                });
                pending_retry = Some(error);
                let batch_ready =
                    record_batch_terminal(&mut batches, request_id, call_id.clone(), None);
                if let Some(items) = batch_ready {
                    let outcome = settle_staged_batch(
                        runtime,
                        &mut pending_events,
                        workspace_path,
                        edit_policy,
                        turn,
                        run,
                        observer,
                        items,
                        &applied,
                    )
                    .await?;
                    applied.extend(outcome.applied);
                    push_changed_paths(&mut changed_paths, outcome.changed_paths);
                    policy_feedbacks.extend(outcome.feedbacks);
                    if let Some(feedback) = outcome.retry {
                        pending_retry = Some(feedback);
                    }
                }
            }
            AppEvent::MessageUpdated(message) => {
                let error_message = {
                    let chat = runtime.state.chat.0.read().await;
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
                    run.events.push(Event::AssistantMessage {
                        id: message_id.to_string(),
                        status: "error".to_string(),
                        content: content.clone(),
                    });
                    observer.emit(format!(
                        "attempt {turn} assistant_error id={} content={}",
                        message_id,
                        truncate_chars(&content, 240)
                    ));
                } else {
                    observer.emit(format!(
                        "attempt {turn} message_error kind={} id={} content={}",
                        kind,
                        message_id,
                        truncate_chars(&content, 240)
                    ));
                }
                if provider_failure.is_none() {
                    provider_failure = message_provider_failure;
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
            }) if parent_id == active_parent_id => {
                run.events.push(Event::Turn {
                    session_id: session_id.to_string(),
                    request_id: request_id.to_string(),
                    parent_id: parent_id.to_string(),
                    assistant_message_id: assistant_message_id.to_string(),
                    outcome: outcome.clone(),
                    error_id: error_id.map(|id| id.to_string()),
                    attempts,
                    summary: summary.clone(),
                });
                drain_response_records(run, assistant_message_id, response_rx);
                observer.emit(format!(
                    "attempt {turn} turn_finished outcome={} attempts={} summary={}",
                    outcome,
                    attempts,
                    truncate_chars(&summary, 240)
                ));
                if provider_failure.is_none() {
                    provider_failure = provider_failure_from_chat(runtime).await;
                }
                if provider_failure.is_none() {
                    provider_failure = provider_unavailable_reason(&summary);
                }
                if let Some(reason) = provider_failure.take() {
                    return Ok(AttemptEnd::Terminal(
                        HeadlessTerminal::ProviderUnavailable { reason },
                    ));
                }
                let repaired_failure = pending_retry.take();

                if outcome != "completed" {
                    if let Some(applied_edit) =
                        applied_edit_from_terminal_items(&applied, &changed_paths)
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

                if applied.is_empty() {
                    let feedback = if let Some(feedback) = repaired_failure {
                        feedback
                    } else if !policy_feedbacks.is_empty() {
                        policy_feedbacks.join("\n")
                    } else if summary.trim().is_empty() {
                        "The model returned without staging an edit; make a concrete bounded edit."
                            .to_string()
                    } else {
                        summary.clone()
                    };
                    run.attempts.push(HeadlessAttempt {
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

                if let Some(feedback) = repaired_failure {
                    observer.emit(format!(
                        "attempt {turn} recovered_tool_failure_after_apply {}",
                        truncate_chars(&feedback, 240)
                    ));
                }

                let applied_edit = applied_edit_from_terminal_items(&applied, &changed_paths)
                    .expect("applied is not empty");
                if !validation_commands.is_empty() {
                    run_contract_validations(
                        runtime,
                        active_parent_id,
                        request_id,
                        turn,
                        run,
                        observer,
                        validation_commands,
                    )
                    .await;
                }
                return Ok(AttemptEnd::Terminal(classify_applied_terminal(
                    run,
                    validation_commands,
                    request_id,
                    applied_edit,
                )));
            }
            _ => {}
        }
    }
}

pub(super) fn terminal_ids(applied: &[AppliedItem]) -> Option<(Uuid, Vec<Uuid>)> {
    let proposal_ids = applied.iter().map(|item| item.id()).collect::<Vec<_>>();
    proposal_ids
        .last()
        .copied()
        .map(|primary| (primary, proposal_ids))
}

fn applied_edit_from_terminal_items(
    applied: &[AppliedItem],
    changed_paths: &[PathBuf],
) -> Option<AppliedEdit> {
    let (proposal_id, proposal_ids) = terminal_ids(applied)?;
    Some(AppliedEdit {
        proposal_id,
        proposal_ids,
        changed_paths: changed_paths.to_vec(),
    })
}

pub(super) fn timeout_terminal_for_run(run: &HeadlessRun, secs: u64) -> HeadlessTerminal {
    if let Some(applied) = run.applied_edit() {
        return HeadlessTerminal::AppliedTimedOut { secs, applied };
    }
    HeadlessTerminal::TimedOut { secs }
}

pub(super) fn turn_aborted_after_apply_terminal(
    applied: AppliedEdit,
    outcome: String,
    summary: String,
) -> HeadlessTerminal {
    HeadlessTerminal::AppliedTurnAborted {
        applied,
        outcome,
        summary,
    }
}

pub(super) fn classify_applied_terminal(
    run: &HeadlessRun,
    validation_commands: &[contract::Command],
    request_id: Uuid,
    applied: AppliedEdit,
) -> HeadlessTerminal {
    if validation_commands.is_empty() {
        if let Some(feedback) = latest_failed_cargo_validation_feedback(run) {
            return HeadlessTerminal::AppliedValidationFailed { applied, feedback };
        }
        return applied_terminal(request_id, applied);
    }

    if let Some(feedback) = failed_requested_validation_feedback(run, validation_commands) {
        return HeadlessTerminal::AppliedValidationFailed { applied, feedback };
    }

    let missing = missing_requested_validation_commands(run, validation_commands);
    if !missing.is_empty() {
        return HeadlessTerminal::AppliedValidationMissing { applied, missing };
    }

    applied_terminal(request_id, applied)
}

fn applied_terminal(request_id: Uuid, applied: AppliedEdit) -> HeadlessTerminal {
    HeadlessTerminal::Applied {
        proposal_id: applied.proposal_id(),
        applied_proposal_ids: applied.proposal_ids().to_vec(),
        request_id,
        changed_paths: applied.changed_paths().to_vec(),
    }
}

fn failed_requested_validation_feedback(
    run: &HeadlessRun,
    validation_commands: &[contract::Command],
) -> Option<String> {
    for command in validation_commands {
        let required = validation_command_display(command);
        let Some(observation) = latest_observation_for_command(run, &required) else {
            continue;
        };
        if let Some(feedback) = observation.failure_feedback() {
            return Some(format!(
                "Requested validation `{required}` failed: {feedback}"
            ));
        }
    }
    None
}

fn missing_requested_validation_commands(
    run: &HeadlessRun,
    validation_commands: &[contract::Command],
) -> Vec<String> {
    validation_commands
        .iter()
        .map(validation_command_display)
        .filter(|required| latest_observation_for_command(run, required).is_none())
        .collect()
}

fn latest_observation_for_command<'a>(
    run: &'a HeadlessRun,
    required: &str,
) -> Option<&'a CargoValidationObservation> {
    run.validations()
        .iter()
        .rev()
        .find(|observation| command_display_matches(required, &observation.display_command))
}

pub(super) fn command_display_matches(required: &str, observed: &str) -> bool {
    observed == required || observed.replace(" -- ", " ") == required
}

pub(super) fn validation_command_display(command: &contract::Command) -> String {
    std::iter::once(command.program.as_str())
        .chain(command.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

async fn run_contract_validations(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    parent_id: Uuid,
    request_id: Uuid,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    validation_commands: &[contract::Command],
) {
    for (idx, command) in validation_commands.iter().enumerate() {
        let required = validation_command_display(command);
        let call_id = format!("declared_validation_{turn}_{idx}");
        let args = match contract_cargo_args(command) {
            Ok(args) => args,
            Err(error) => {
                observer.emit(format!(
                    "attempt {turn} declared_validation_unsupported command={} error={}",
                    required,
                    truncate_chars(&error, 240)
                ));
                continue;
            }
        };
        observer.emit(format!(
            "attempt {turn} declared_validation_start call_id={} command={}",
            call_id, required
        ));

        let ctx = ploke_tui::tools::Ctx {
            state: Arc::clone(&runtime.state),
            event_bus: Arc::new(ploke_tui::EventBus::new(ploke_tui::EventBusCaps::default())),
            request_id,
            parent_id,
            call_id: ploke_core::ArcStr::from(call_id.clone()),
        };
        let params =
            match <ploke_tui::tools::cargo::CargoTool as ploke_tui::tools::Tool>::deserialize_params(
                &args,
            ) {
                Ok(params) => params,
                Err(error) => {
                    let detail =
                        <ploke_tui::tools::cargo::CargoTool as ploke_tui::tools::Tool>::adapt_error(
                            error,
                        )
                        .to_wire_string();
                    observer.emit(format!(
                        "attempt {turn} declared_validation_deserialize_failed call_id={} command={} error={}",
                        call_id,
                        required,
                        truncate_chars(&detail, 240)
                    ));
                    record_validation_failure(run, &call_id, command);
                    continue;
                }
            };
        match <ploke_tui::tools::cargo::CargoTool as ploke_tui::tools::Tool>::execute(params, ctx)
            .await
        {
            Ok(result) => {
                if let Some(validation) =
                    observe_cargo_validation(run, &call_id, &args, &result.content)
                {
                    observer.emit(format!(
                        "attempt {turn} declared_validation_done call_id={} ok={} status={} command={}",
                        call_id,
                        validation.ok,
                        validation.status_reason,
                        validation.display_command
                    ));
                }
            }
            Err(error) => {
                let detail =
                    <ploke_tui::tools::cargo::CargoTool as ploke_tui::tools::Tool>::adapt_error(
                        ploke_tui::tools::ToolInvocationError::Exec(error),
                    )
                    .to_wire_string();
                observer.emit(format!(
                    "attempt {turn} declared_validation_failed call_id={} command={} error={}",
                    call_id,
                    required,
                    truncate_chars(&detail, 240)
                ));
                record_validation_failure(run, &call_id, command);
            }
        }
    }
}

pub(super) fn contract_cargo_args(command: &contract::Command) -> Result<String, String> {
    if command.program != "cargo" {
        return Err(format!(
            "unsupported validation program `{}`",
            command.program
        ));
    }
    let Some((subcommand, rest)) = command.args.split_first() else {
        return Err("cargo validation command is missing subcommand".to_string());
    };
    if !matches!(subcommand.as_str(), "check" | "test") {
        return Err(format!("unsupported cargo subcommand `{subcommand}`"));
    }

    let mut args = serde_json::Map::new();
    args.insert(
        "command".to_string(),
        serde_json::Value::String(subcommand.clone()),
    );
    let mut test_args = Vec::<String>::new();
    let mut rest = rest.iter();
    let mut passthrough = false;

    while let Some(arg) = rest.next() {
        if passthrough {
            test_args.push(arg.clone());
            continue;
        }
        match arg.as_str() {
            "--" => passthrough = true,
            "-p" | "--package" => insert_next(&mut args, &mut rest, "package", arg)?,
            "--features" => insert_features(&mut args, rest.next(), arg)?,
            "--target" => insert_next(&mut args, &mut rest, "target", arg)?,
            "--profile" => insert_next(&mut args, &mut rest, "profile", arg)?,
            "--all-features" => insert_bool(&mut args, "all_features"),
            "--no-default-features" => insert_bool(&mut args, "no_default_features"),
            "--release" => insert_bool(&mut args, "release"),
            "--lib" => insert_bool(&mut args, "lib"),
            "--tests" => insert_bool(&mut args, "tests"),
            "--bins" => insert_bool(&mut args, "bins"),
            "--examples" => insert_bool(&mut args, "examples"),
            "--benches" => insert_bool(&mut args, "benches"),
            _ if arg.starts_with("--package=") => {
                insert_str(&mut args, "package", &arg["--package=".len()..])?
            }
            _ if arg.starts_with("--features=") => {
                insert_features_value(&mut args, &arg["--features=".len()..])?
            }
            _ if arg.starts_with("--target=") => {
                insert_str(&mut args, "target", &arg["--target=".len()..])?
            }
            _ if arg.starts_with("--profile=") => {
                insert_str(&mut args, "profile", &arg["--profile=".len()..])?
            }
            _ if subcommand == "test" => test_args.push(arg.clone()),
            _ => {
                return Err(format!(
                    "unsupported cargo validation argument `{arg}` in `{}`",
                    validation_command_display(command)
                ));
            }
        }
    }

    if !test_args.is_empty() {
        args.insert(
            "test_args".to_string(),
            serde_json::Value::Array(
                test_args
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }

    serde_json::to_string(&serde_json::Value::Object(args))
        .map_err(|error| format!("failed to serialize cargo validation args: {error}"))
}

fn insert_next<'a>(
    args: &mut serde_json::Map<String, serde_json::Value>,
    rest: &mut impl Iterator<Item = &'a String>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    let Some(value) = rest.next() else {
        return Err(format!("missing value for `{flag}`"));
    };
    insert_str(args, key, value)
}

fn insert_str(
    args: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: &str,
) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("empty value for `{key}`"));
    }
    args.insert(
        key.to_string(),
        serde_json::Value::String(value.to_string()),
    );
    Ok(())
}

fn insert_bool(args: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    args.insert(key.to_string(), serde_json::Value::Bool(true));
}

fn insert_features<'a>(
    args: &mut serde_json::Map<String, serde_json::Value>,
    value: Option<&'a String>,
    flag: &str,
) -> Result<(), String> {
    let Some(value) = value else {
        return Err(format!("missing value for `{flag}`"));
    };
    insert_features_value(args, value)
}

fn insert_features_value(
    args: &mut serde_json::Map<String, serde_json::Value>,
    value: &str,
) -> Result<(), String> {
    let features = value
        .split(',')
        .filter(|feature| !feature.is_empty())
        .map(|feature| serde_json::Value::String(feature.to_string()))
        .collect::<Vec<_>>();
    if features.is_empty() {
        return Err("empty value for `features`".to_string());
    }
    args.insert("features".to_string(), serde_json::Value::Array(features));
    Ok(())
}

fn record_validation_failure(run: &mut HeadlessRun, call_id: &str, command: &contract::Command) {
    let command_name = command.args.first().cloned().unwrap_or_default();
    run.validations.push(CargoValidationObservation {
        call_id: call_id.to_string(),
        command: command_name,
        display_command: validation_command_display(command),
        ok: false,
        status_reason: "cargo_failed_or_invalid_args".to_string(),
        exit_code: None,
        manifest_path: String::new(),
        errors: 1,
        warnings: 0,
    });
}

pub(super) fn drain_response_records(
    run: &mut HeadlessRun,
    assistant_message_id: Uuid,
    response_rx: Option<&Mutex<Receiver<RecordedResponse>>>,
) {
    let Some(response_rx) = response_rx else {
        return;
    };
    let Ok(response_rx) = response_rx.lock() else {
        return;
    };
    for recorded_response in response_rx.try_iter() {
        let response_index = run.next_response_index;
        run.next_response_index = run.next_response_index.saturating_add(1);
        run.full_response_records.push(RawFullResponseRecord {
            assistant_message_id,
            recorded_response: RecordedResponse::new(response_index, recorded_response.response),
        });
    }
}

pub(super) fn record_batch_terminal(
    batches: &mut HashMap<Uuid, ToolBatch>,
    request_id: Uuid,
    call_id: ploke_core::ArcStr,
    staged: Option<StagedItem>,
) -> Option<Vec<StagedItem>> {
    let batch = batches.entry(request_id).or_default();
    batch.complete(call_id, staged);
    if batch.is_complete() {
        batches.remove(&request_id).map(|batch| batch.staged)
    } else {
        None
    }
}

async fn observe_staged_item(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    ui_payload: Option<&ploke_tui::tools::ToolUiPayload>,
    request_id: Uuid,
    applied: &[AppliedItem],
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
) -> Option<StagedItem> {
    let Some(payload) = ui_payload else {
        return None;
    };
    if let Some(proposal_id) = payload.proposal_id {
        if applied.contains(&AppliedItem::Edit(proposal_id)) {
            observer.emit(format!(
                "attempt {turn} proposal_already_applied id={proposal_id}"
            ));
            return None;
        }
        let Some(proposal) = runtime
            .state
            .proposals
            .read()
            .await
            .get(&proposal_id)
            .cloned()
        else {
            return None;
        };
        let paths = proposal_paths(&proposal);
        run.events.push(Event::Proposal {
            id: proposal_id.to_string(),
            edit_count: proposal.edits.len() + proposal.edits_ns.len(),
            paths: paths.clone(),
        });
        observer.emit(format!(
            "attempt {turn} proposal id={} edit_count={} paths={}",
            proposal_id,
            proposal.edits.len() + proposal.edits_ns.len(),
            join_paths(&paths)
        ));
        return Some(StagedItem::Edit(proposal_id));
    }

    if payload.tool == ploke_tui::tools::ToolName::CreateFile {
        if applied.contains(&AppliedItem::Create(request_id)) {
            observer.emit(format!(
                "attempt {turn} creation_already_applied id={request_id}"
            ));
            return None;
        }
        let Some(proposal) = runtime
            .state
            .create_proposals
            .read()
            .await
            .get(&request_id)
            .cloned()
        else {
            return None;
        };
        let paths = proposal.files.clone();
        run.events.push(Event::Proposal {
            id: request_id.to_string(),
            edit_count: proposal.creates.len(),
            paths: paths.clone(),
        });
        observer.emit(format!(
            "attempt {turn} creation id={} edit_count={} paths={}",
            request_id,
            proposal.creates.len(),
            join_paths(&paths)
        ));
        return Some(StagedItem::Create(request_id));
    }

    None
}

async fn settle_staged_batch(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    pending_events: &mut VecDeque<ploke_tui::AppEvent>,
    workspace_path: &Path,
    edit_policy: BroadEditPolicy,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    staged: Vec<StagedItem>,
    applied: &[AppliedItem],
) -> Result<BatchOutcome, Error> {
    let mut outcome = BatchOutcome::default();
    let mut candidates = Vec::new();

    for item in staged {
        if applied.contains(&item.applied()) {
            continue;
        }
        let Some(candidate) = candidate_for_item(runtime, item).await else {
            let reason = format!("staged proposal {} disappeared before admission", item.id());
            reject_item(runtime, item, turn, run, observer, reason.clone()).await?;
            outcome.retry = Some(reason);
            continue;
        };
        if candidate.paths.is_empty() {
            let reason = match item {
                StagedItem::Edit(_) => "No material edit was staged; make a concrete bounded edit.",
                StagedItem::Create(_) => {
                    "No material file creation was staged; make a concrete bounded edit."
                }
            }
            .to_string();
            reject_item(runtime, item, turn, run, observer, reason.clone()).await?;
            outcome.feedbacks.push(repair_prompt_feedback(&reason));
            continue;
        }
        if let Some(rejection) = classify_paths(workspace_path, edit_policy, &candidate.paths) {
            let feedback = Feedback::from_outcome(&Outcome::Rejected(rejection));
            reject_item(
                runtime,
                item,
                turn,
                run,
                observer,
                feedback.message().to_string(),
            )
            .await?;
            outcome
                .feedbacks
                .push(repair_prompt_feedback(feedback.message()));
            continue;
        }
        candidates.push(candidate);
    }

    let (selected, rejected) = select_disjoint(candidates, workspace_path);
    for candidate in rejected {
        let reason =
            "Staged edit overlaps a newer valid proposal from the same tool batch".to_string();
        reject_item(runtime, candidate.item, turn, run, observer, reason.clone()).await?;
        outcome.feedbacks.push(repair_prompt_feedback(&reason));
    }

    if selected.is_empty() {
        return Ok(outcome);
    }

    approve_selected(runtime, turn, observer, &selected).await?;
    let applied_outcome = match wait_for_selected(runtime, turn, run, observer, &selected).await {
        Ok(outcome) => outcome,
        Err(error) => {
            record_post_approval_indeterminate(run, turn, observer, &selected, &error.to_string());
            return Err(error);
        }
    };
    outcome.applied.extend(applied_outcome.applied);
    outcome.changed_paths.extend(applied_outcome.changed_paths);
    outcome.mutated |= applied_outcome.mutated;
    if applied_outcome.retry.is_some() {
        outcome.retry = applied_outcome.retry;
    }
    if !outcome.applied.is_empty() || outcome.mutated {
        if let Err(error) = wait_for_refresh(runtime, pending_events, turn, observer).await {
            record_post_approval_indeterminate(run, turn, observer, &selected, &error.to_string());
            return Err(error);
        }
    }
    Ok(outcome)
}

pub(super) fn record_post_approval_indeterminate(
    run: &mut HeadlessRun,
    turn: u32,
    observer: &LiveObserver,
    selected: &[Candidate],
    error: &str,
) {
    for candidate in selected {
        if has_recorded_apply_outcome(run, candidate.item) {
            continue;
        }
        run.attempts.push(HeadlessAttempt {
            turn,
            proposal_id: Some(candidate.item.id()),
            result: HeadlessAttemptResult::PostApprovalIndeterminate {
                paths: candidate.paths.clone(),
                error: error.to_string(),
            },
        });
        observer.emit(format!(
            "attempt {turn} proposal_post_approval_indeterminate id={} error={}",
            candidate.item.id(),
            truncate_chars(error, 240)
        ));
    }
}

fn has_recorded_apply_outcome(run: &HeadlessRun, item: StagedItem) -> bool {
    run.attempts.iter().any(|attempt| {
        attempt.proposal_id == Some(item.id())
            && matches!(
                attempt.result,
                HeadlessAttemptResult::Applied { .. }
                    | HeadlessAttemptResult::Rejected { .. }
                    | HeadlessAttemptResult::PostApprovalIndeterminate { .. }
            )
    })
}

async fn candidate_for_item(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    item: StagedItem,
) -> Option<Candidate> {
    match item {
        StagedItem::Edit(proposal_id) => {
            let proposal = runtime
                .state
                .proposals
                .read()
                .await
                .get(&proposal_id)
                .cloned()?;
            Some(Candidate {
                item,
                proposed_at_ms: proposal.proposed_at_ms,
                paths: proposal_paths(&proposal),
            })
        }
        StagedItem::Create(request_id) => {
            let proposal = runtime
                .state
                .create_proposals
                .read()
                .await
                .get(&request_id)
                .cloned()?;
            Some(Candidate {
                item,
                proposed_at_ms: proposal.proposed_at_ms,
                paths: proposal.files,
            })
        }
    }
}

pub(super) fn select_disjoint(
    mut candidates: Vec<Candidate>,
    workspace_path: &Path,
) -> (Vec<Candidate>, Vec<Candidate>) {
    candidates.sort_by(|a, b| {
        b.proposed_at_ms
            .cmp(&a.proposed_at_ms)
            .then(b.item.id().cmp(&a.item.id()))
    });

    let mut occupied = Vec::<PathBuf>::new();
    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    for candidate in candidates {
        let keys = candidate
            .paths
            .iter()
            .map(|path| path_key(workspace_path, path))
            .collect::<Vec<_>>();
        if keys.iter().any(|key| occupied.contains(key)) {
            rejected.push(candidate);
        } else {
            occupied.extend(keys);
            selected.push(candidate);
        }
    }
    (selected, rejected)
}

fn path_key(workspace_path: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.strip_prefix(workspace_path)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

async fn reject_item(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    item: StagedItem,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    reason: String,
) -> Result<(), Error> {
    deny_item(runtime, item).await?;
    run.attempts.push(HeadlessAttempt {
        turn,
        proposal_id: Some(item.id()),
        result: HeadlessAttemptResult::Rejected {
            reason: reason.clone(),
        },
    });
    observer.emit(format!(
        "attempt {turn} proposal_rejected id={} reason={}",
        item.id(),
        truncate_chars(&reason, 240)
    ));
    Ok(())
}

async fn deny_item(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    item: StagedItem,
) -> Result<(), Error> {
    use ploke_tui::app_state::StateCommand;

    let cmd_tx = runtime.app.state_cmd_tx();
    match item {
        StagedItem::Edit(proposal_id) => {
            send_state(&cmd_tx, StateCommand::DenyEdits { proposal_id }).await
        }
        StagedItem::Create(request_id) => {
            send_state(&cmd_tx, StateCommand::DenyCreations { request_id }).await
        }
    }
}

async fn approve_selected(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    turn: u32,
    observer: &LiveObserver,
    selected: &[Candidate],
) -> Result<(), Error> {
    use ploke_tui::app_state::StateCommand;

    let cmd_tx = runtime.app.state_cmd_tx();
    for candidate in selected {
        match candidate.item {
            StagedItem::Edit(proposal_id) => {
                observer.emit(format!("attempt {turn} proposal_approve id={proposal_id}"));
                send_state(&cmd_tx, StateCommand::ApproveEdits { proposal_id }).await?;
            }
            StagedItem::Create(request_id) => {
                observer.emit(format!("attempt {turn} creation_approve id={request_id}"));
                send_state(&cmd_tx, StateCommand::ApproveCreations { request_id }).await?;
            }
        }
    }
    Ok(())
}

async fn wait_for_selected(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    selected: &[Candidate],
) -> Result<BatchOutcome, Error> {
    use ploke_tui::app_state::core::EditProposalStatus;

    let deadline = Instant::now() + Duration::from_secs(POST_APPLY_STATUS_TIMEOUT_SECS);
    let mut pending = selected
        .iter()
        .map(|candidate| candidate.item)
        .collect::<Vec<_>>();
    let mut outcome = BatchOutcome::default();

    while !pending.is_empty() {
        if Instant::now() >= deadline {
            return Err(Error::HeadlessEvent(format!(
                "timed out waiting for proposal batch apply after {POST_APPLY_STATUS_TIMEOUT_SECS}s"
            )));
        }

        runtime.app.pump_pending_events().await;
        drain_debug_observed(&mut runtime.debug_rx, run, observer, turn);

        let mut still_pending = Vec::new();
        for item in pending {
            let Some((status, paths)) = item_status(runtime, item).await else {
                let reason = format!("staged proposal {} disappeared before apply", item.id());
                run.attempts.push(HeadlessAttempt {
                    turn,
                    proposal_id: Some(item.id()),
                    result: HeadlessAttemptResult::Rejected {
                        reason: reason.clone(),
                    },
                });
                outcome.retry = Some(reason);
                continue;
            };
            match status {
                EditProposalStatus::Applied => {
                    run.attempts.push(HeadlessAttempt {
                        turn,
                        proposal_id: Some(item.id()),
                        result: HeadlessAttemptResult::Applied {
                            paths: paths.clone(),
                        },
                    });
                    observer.emit(format!("attempt {turn} proposal_applied id={}", item.id()));
                    outcome.applied.push(item.applied());
                    push_changed_paths(&mut outcome.changed_paths, paths);
                }
                EditProposalStatus::PartiallyApplied(reason) => {
                    run.attempts.push(HeadlessAttempt {
                        turn,
                        proposal_id: Some(item.id()),
                        result: HeadlessAttemptResult::Rejected {
                            reason: reason.clone(),
                        },
                    });
                    observer.emit(format!(
                        "attempt {turn} proposal_partially_applied id={} reason={}",
                        item.id(),
                        truncate_chars(&reason, 240)
                    ));
                    outcome.mutated = true;
                    outcome.retry = Some(reason);
                }
                EditProposalStatus::Failed(reason) | EditProposalStatus::Stale(reason) => {
                    run.attempts.push(HeadlessAttempt {
                        turn,
                        proposal_id: Some(item.id()),
                        result: HeadlessAttemptResult::Rejected {
                            reason: reason.clone(),
                        },
                    });
                    observer.emit(format!(
                        "attempt {turn} proposal_apply_failed id={} reason={}",
                        item.id(),
                        truncate_chars(&reason, 240)
                    ));
                    outcome.retry = Some(reason);
                }
                EditProposalStatus::Denied => {
                    let reason = "proposal was denied before apply".to_string();
                    run.attempts.push(HeadlessAttempt {
                        turn,
                        proposal_id: Some(item.id()),
                        result: HeadlessAttemptResult::Rejected {
                            reason: reason.clone(),
                        },
                    });
                    observer.emit(format!("attempt {turn} proposal_denied id={}", item.id()));
                    outcome.retry = Some(reason);
                }
                EditProposalStatus::Pending | EditProposalStatus::Approved => {
                    still_pending.push(item);
                }
            }
        }
        pending = still_pending;
        if !pending.is_empty() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    Ok(outcome)
}

async fn item_status(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    item: StagedItem,
) -> Option<(ploke_tui::app_state::core::EditProposalStatus, Vec<PathBuf>)> {
    match item {
        StagedItem::Edit(proposal_id) => {
            let proposal = runtime
                .state
                .proposals
                .read()
                .await
                .get(&proposal_id)
                .cloned()?;
            let paths = proposal_paths(&proposal);
            Some((proposal.status, paths))
        }
        StagedItem::Create(request_id) => {
            let proposal = runtime
                .state
                .create_proposals
                .read()
                .await
                .get(&request_id)
                .cloned()?;
            Some((proposal.status, proposal.files))
        }
    }
}

pub(super) async fn wait_for_refresh(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    pending_events: &mut VecDeque<ploke_tui::AppEvent>,
    turn: u32,
    observer: &LiveObserver,
) -> Result<(), Error> {
    use ploke_tui::app_state::StateCommand;

    let (scan_tx, scan_rx) = oneshot::channel();
    send_state(
        &runtime.app.state_cmd_tx(),
        StateCommand::ScanForChange { scan_tx },
    )
    .await?;
    let changed = scan_rx
        .await
        .map_err(|source| Error::HeadlessEvent(format!("scan barrier failed: {source}")))?;
    runtime.app.pump_pending_events().await;
    observer.emit(format!(
        "attempt {turn} scan_barrier changed={}",
        changed
            .as_ref()
            .map(|paths| join_paths(paths))
            .unwrap_or_else(|| "none".to_string())
    ));
    if wait_for_sparse_search_refresh(runtime, changed.is_some(), turn, observer).await? {
        return Ok(());
    }
    wait_for_index_output(runtime, pending_events, changed.is_some(), turn, observer).await
}

async fn wait_for_sparse_search_refresh(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    changed: bool,
    turn: u32,
    observer: &LiveObserver,
) -> Result<bool, Error> {
    use ploke_db::bm25_index::bm25_service::Bm25Status;

    let sparse_refresh = {
        let cfg = runtime.state.config.read().await;
        sparse_search_refresh_enabled(&cfg.rag.strategy, cfg.rag.strict_bm25_by_default)
    };
    if !sparse_refresh {
        return Ok(false);
    }

    let Some(rag) = runtime.state.rag.as_ref().cloned() else {
        return Err(Error::HeadlessEvent(
            "sparse post-apply refresh requires a RAG service, but none is configured".to_string(),
        ));
    };

    if changed {
        observer.emit(format!("attempt {turn} sparse_refresh bm25_rebuild"));
        rag.bm25_rebuild()
            .await
            .map_err(|source| Error::HeadlessEvent(format!("BM25 rebuild failed: {source}")))?;
    } else {
        observer.emit(format!("attempt {turn} sparse_refresh bm25_status"));
    }

    let deadline = Instant::now() + Duration::from_secs(POST_APPLY_INDEX_TIMEOUT_SECS);
    loop {
        runtime.app.pump_pending_events().await;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Error::HeadlessEvent(format!(
                "timed out waiting for BM25 readiness after applying proposal batch after {POST_APPLY_INDEX_TIMEOUT_SECS}s"
            )));
        }

        let status = rag
            .bm25_status_with_timeout(remaining)
            .await
            .map_err(|source| Error::HeadlessEvent(format!("BM25 status failed: {source}")))?;
        match status {
            Bm25Status::Ready { docs } => {
                if docs > 0 {
                    runtime.app.pump_pending_events().await;
                    observer.emit(format!(
                        "attempt {turn} sparse_refresh bm25_ready docs={docs}"
                    ));
                    return Ok(true);
                }
                return Err(Error::HeadlessEvent(
                    "BM25 index is empty after applying proposal batch".to_string(),
                ));
            }
            Bm25Status::Empty => {
                return Err(Error::HeadlessEvent(
                    "BM25 index is empty after applying proposal batch".to_string(),
                ));
            }
            Bm25Status::Error(detail) => {
                return Err(Error::HeadlessEvent(format!(
                    "BM25 index failed after applying proposal batch: {detail}"
                )));
            }
            Bm25Status::Uninitialized | Bm25Status::Building => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

pub(super) fn sparse_search_refresh_enabled(
    strategy: &ploke_tui::user_config::RetrievalStrategyUser,
    strict_bm25_by_default: bool,
) -> bool {
    match strategy {
        ploke_tui::user_config::RetrievalStrategyUser::Sparse { strict } => {
            *strict || strict_bm25_by_default
        }
        ploke_tui::user_config::RetrievalStrategyUser::Dense
        | ploke_tui::user_config::RetrievalStrategyUser::Hybrid { .. } => false,
    }
}

async fn wait_for_index_output(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    pending_events: &mut VecDeque<ploke_tui::AppEvent>,
    require_index: bool,
    turn: u32,
    observer: &LiveObserver,
) -> Result<(), Error> {
    use ploke_tui::{AppEvent, app_state::events::SystemEvent};

    let full_deadline = Instant::now() + Duration::from_secs(POST_APPLY_INDEX_TIMEOUT_SECS);
    let start_grace = Instant::now() + Duration::from_millis(POST_APPLY_INDEX_START_GRACE_MS);
    let mut saw_index = require_index;

    loop {
        runtime.app.pump_pending_events().await;
        let now = Instant::now();
        if now >= full_deadline {
            return Err(Error::HeadlessEvent(format!(
                "timed out waiting for indexing completion after {POST_APPLY_INDEX_TIMEOUT_SECS}s"
            )));
        }
        if !saw_index && now >= start_grace {
            observer.emit(format!("attempt {turn} index_barrier no_index_output"));
            return Ok(());
        }

        let deadline = if saw_index {
            full_deadline
        } else {
            start_grace
        };
        let wait_for = deadline
            .saturating_duration_since(now)
            .min(Duration::from_millis(250));
        let event = match tokio::time::timeout(wait_for, next_event(runtime)).await {
            Ok(Ok(event)) => event,
            Ok(Err(err)) => return Err(err),
            Err(_) => continue,
        };

        match event {
            AppEvent::System(SystemEvent::ReIndex { target }) => {
                saw_index = true;
                observer.emit(format!(
                    "attempt {turn} reindex_scheduled target={target:?}"
                ));
                runtime.app.pump_pending_events().await;
            }
            AppEvent::IndexingStarted | AppEvent::IndexingProgress(_) => {
                saw_index = true;
            }
            AppEvent::IndexingCompleted => {
                runtime.app.pump_pending_events().await;
                observer.emit(format!("attempt {turn} index_barrier completed"));
                return Ok(());
            }
            AppEvent::IndexingFailed => {
                return Err(Error::HeadlessEvent(
                    "indexing failed after applying proposal batch".to_string(),
                ));
            }
            AppEvent::Error(error) if error.message.contains("Indexing failed") => {
                return Err(Error::HeadlessEvent(error.message));
            }
            other => pending_events.push_back(other),
        }
    }
}

pub(super) fn provider_unavailable_reason(content: &str) -> Option<String> {
    let normalized = content.to_ascii_lowercase();
    if normalized.contains("api error")
        && (normalized.contains("status 401")
            || normalized.contains("status 403")
            || normalized.contains("status 429")
            || normalized.contains("key limit exceeded")
            || normalized.contains("rate limit"))
    {
        return Some(content.to_string());
    }
    if normalized.contains("failed to resolve bearer token")
        || normalized.contains("application default credentials")
        || normalized.contains("reauthentication failed")
        || normalized.contains("auth tokens")
    {
        return Some(content.to_string());
    }
    None
}

pub(super) fn provider_failure_from_message(
    kind: ploke_tui::chat_history::MessageKind,
    status: &ploke_tui::chat_history::MessageStatus,
    content: &str,
) -> Option<String> {
    let ploke_tui::chat_history::MessageStatus::Error { description } = status else {
        return None;
    };
    if !matches!(
        kind,
        ploke_tui::chat_history::MessageKind::Assistant
            | ploke_tui::chat_history::MessageKind::System
            | ploke_tui::chat_history::MessageKind::SysInfo
    ) {
        return None;
    }
    provider_unavailable_reason(content).or_else(|| provider_unavailable_reason(description))
}

async fn provider_failure_from_chat(
    runtime: &crate::runner::WorkspaceTuiRuntime,
) -> Option<String> {
    let chat = runtime.state.chat.0.read().await;
    chat.messages.values().find_map(|message| {
        provider_failure_from_message(message.kind, &message.status, &message.content)
    })
}

fn advance_turn(budget: &Budget, turn: &mut u32) -> bool {
    if *turn >= budget.max_attempts() {
        return false;
    }
    *turn += 1;
    true
}

pub(super) fn attempt_prompt(
    _workspace_path: &Path,
    _edit_policy: BroadEditPolicy,
    _evidence_roots: &[EvidenceRoot],
    request_prompt: &str,
    feedback: Option<&str>,
) -> String {
    let mut prompt = request_prompt.trim_end().to_string();
    if let Some(feedback) = feedback {
        prompt.push_str("\n\nPrevious attempt result:\n");
        prompt.push_str(feedback);
        prompt.push('\n');
    }
    prompt
}

pub(super) fn evidence_read_roots(evidence_roots: &[EvidenceRoot]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for root in evidence_roots {
        if root.kind == EvidenceRootKind::SubmittedResultOutput {
            continue;
        }
        if let Some(path) = prototype_navigation_root(root) {
            roots.push(path);
        }
        match &root.location {
            EvidenceRootLocation::Directory { path } => {
                roots.push(path.clone());
            }
            EvidenceRootLocation::File { path } => {
                if let Some(parent) = path.parent() {
                    roots.push(parent.to_path_buf());
                }
            }
            EvidenceRootLocation::NodeScopedDirectory { nodes_root, .. } => {
                roots.push(nodes_root.clone());
            }
            EvidenceRootLocation::AttachedReport { .. } => {}
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

fn prototype_navigation_root(root: &EvidenceRoot) -> Option<PathBuf> {
    match (&root.kind, &root.location) {
        (
            EvidenceRootKind::Evaluations | EvidenceRootKind::Nodes,
            EvidenceRootLocation::Directory { path },
        ) => path.parent().map(Path::to_path_buf),
        (EvidenceRootKind::HistoryBlocks, EvidenceRootLocation::Directory { path }) => {
            path.parent().and_then(Path::parent).map(Path::to_path_buf)
        }
        (
            EvidenceRootKind::ProtocolArtifacts,
            EvidenceRootLocation::NodeScopedDirectory { nodes_root, .. },
        ) => nodes_root.parent().map(Path::to_path_buf),
        _ => None,
    }
}

pub(super) fn retry_feedback(feedback: &str) -> String {
    #[derive(Deserialize)]
    struct ToolFailure {
        user: String,
    }

    if let Ok(failure) = serde_json::from_str::<ToolFailure>(feedback) {
        return format!("Previous attempt failed: {}", failure.user);
    }

    if feedback.contains("[aborted]") {
        return "Previous attempt aborted before staging an edit.".to_string();
    }

    feedback.to_string()
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LiveObserver {
    enabled: bool,
    resources: bool,
    started: Instant,
}

impl LiveObserver {
    fn from_env() -> Self {
        Self {
            enabled: std::env::var_os(LIVE_TRACE_ENV)
                .and_then(|value| value.into_string().ok())
                .map(|value| {
                    let value = value.trim().to_ascii_lowercase();
                    !matches!(value.as_str(), "" | "0" | "false" | "off" | "no")
                })
                .unwrap_or(false),
            resources: std::env::var_os("PLOKE_EVAL_HEADLESS_TUI_LIVE_RESOURCES")
                .and_then(|value| value.into_string().ok())
                .map(|value| {
                    let value = value.trim().to_ascii_lowercase();
                    !matches!(value.as_str(), "" | "0" | "false" | "off" | "no")
                })
                .unwrap_or(false),
            started: Instant::now(),
        }
    }

    #[cfg(test)]
    pub(super) fn disabled() -> Self {
        Self {
            enabled: false,
            resources: false,
            started: Instant::now(),
        }
    }

    fn emit(&self, message: impl AsRef<str>) {
        if self.enabled {
            if self.resources {
                match current_rss_kb() {
                    Some(rss_kb) => eprintln!(
                        "[headless-tui elapsed_ms={} rss_kb={rss_kb}] {}",
                        self.started.elapsed().as_millis(),
                        message.as_ref()
                    ),
                    None => eprintln!(
                        "[headless-tui elapsed_ms={} rss_kb=unknown] {}",
                        self.started.elapsed().as_millis(),
                        message.as_ref()
                    ),
                }
            } else {
                eprintln!(
                    "[headless-tui elapsed_ms={}] {}",
                    self.started.elapsed().as_millis(),
                    message.as_ref()
                );
            }
        }
    }

    fn emit_workspace_size(&self, label: &str, workspace_path: &Path) {
        if self.enabled && self.resources {
            let workspace = dir_size_limited(workspace_path, 50_000);
            let target = dir_size_limited(&workspace_path.join("target"), 50_000);
            match current_rss_kb() {
                Some(rss_kb) => eprintln!(
                    "[headless-tui elapsed_ms={} rss_kb={rss_kb} {label} workspace_bytes={} workspace_entries={} workspace_truncated={} target_bytes={} target_entries={} target_truncated={}]",
                    self.started.elapsed().as_millis(),
                    workspace.bytes,
                    workspace.entries,
                    workspace.truncated,
                    target.bytes,
                    target.entries,
                    target.truncated,
                ),
                None => eprintln!(
                    "[headless-tui elapsed_ms={} rss_kb=unknown {label} workspace_bytes={} workspace_entries={} workspace_truncated={} target_bytes={} target_entries={} target_truncated={}]",
                    self.started.elapsed().as_millis(),
                    workspace.bytes,
                    workspace.entries,
                    workspace.truncated,
                    target.bytes,
                    target.entries,
                    target.truncated,
                ),
            }
        }
    }
}

#[derive(Debug, Default)]
struct DirSize {
    bytes: u64,
    entries: usize,
    truncated: bool,
}

fn current_rss_kb() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let rest = line.strip_prefix("VmRSS:")?;
        rest.split_whitespace().next()?.parse::<u64>().ok()
    })
}

fn dir_size_limited(path: &Path, max_entries: usize) -> DirSize {
    let mut size = DirSize::default();
    let mut stack = vec![path.to_path_buf()];
    while let Some(path) = stack.pop() {
        if size.entries >= max_entries {
            size.truncated = true;
            break;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        size.entries += 1;
        if metadata.is_file() {
            size.bytes = size.bytes.saturating_add(metadata.len());
            continue;
        }
        if !metadata.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            stack.push(entry.path());
        }
    }
    size
}

pub(super) async fn submit_prompt(
    app: &ploke_tui::app::App,
    content: String,
) -> Result<Uuid, Error> {
    let new_msg_id = Uuid::new_v4();
    let (completion_tx, completion_rx) = oneshot::channel();
    let (scan_tx, scan_rx) = oneshot::channel();
    let cmd_tx = app.state_cmd_tx();
    send_state(
        &cmd_tx,
        ploke_tui::app_state::commands::StateCommand::AddUserMessage {
            content,
            new_user_msg_id: new_msg_id,
            completion_tx,
        },
    )
    .await?;
    send_state(
        &cmd_tx,
        ploke_tui::app_state::commands::StateCommand::ScanForChange { scan_tx },
    )
    .await?;
    send_state(
        &cmd_tx,
        ploke_tui::app_state::commands::StateCommand::EmbedMessage {
            new_msg_id,
            completion_rx,
            scan_rx,
        },
    )
    .await?;
    Ok(new_msg_id)
}

async fn send_state(
    cmd_tx: &tokio::sync::mpsc::Sender<ploke_tui::app_state::commands::StateCommand>,
    cmd: ploke_tui::app_state::commands::StateCommand,
) -> Result<(), Error> {
    cmd_tx
        .send(cmd)
        .await
        .map_err(|source| Error::HeadlessEvent(format!("state command send failed: {source}")))
}

pub(super) async fn next_event(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
) -> Result<ploke_tui::AppEvent, Error> {
    tokio::select! {
        realtime = runtime.realtime_rx.recv() => {
            realtime.map_err(|source| Error::HeadlessEvent(source.to_string()))
        }
        background = runtime.background_rx.recv() => {
            background.map_err(|source| Error::HeadlessEvent(source.to_string()))
        }
    }
}

fn drain_debug(
    debug_rx: &mut tokio::sync::mpsc::Receiver<
        ploke_tui::app::commands::harness::DebugStateCommand,
    >,
    run: &mut HeadlessRun,
) {
    loop {
        match debug_rx.try_recv() {
            Ok(debug) => run.debug_relay.push(debug.as_str()),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
}

fn drain_debug_observed(
    debug_rx: &mut tokio::sync::mpsc::Receiver<
        ploke_tui::app::commands::harness::DebugStateCommand,
    >,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    turn: u32,
) {
    loop {
        match debug_rx.try_recv() {
            Ok(debug) => {
                let text = debug.as_str();
                if text.contains("Provider emitted invalid arguments")
                    || text.contains("Repeated repair attempts")
                {
                    observer.emit(format!(
                        "attempt {turn} repair_event {}",
                        truncate_chars(text, 240)
                    ));
                }
                run.debug_relay.push(text);
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
}

fn proposal_paths(proposal: &ploke_tui::app_state::core::EditProposal) -> Vec<PathBuf> {
    if !proposal.files.is_empty() {
        return proposal.files.clone();
    }
    let mut paths = proposal
        .edits
        .iter()
        .map(|edit| edit.file_path.clone())
        .collect::<Vec<_>>();
    paths.extend(proposal.edits_ns.iter().map(|edit| edit.file_path.clone()));
    paths
}

pub(super) fn classify_paths(
    workspace_path: &Path,
    edit_policy: BroadEditPolicy,
    paths: &[PathBuf],
) -> Option<Reject> {
    use crate::cli::prototype1_state::backend::{
        path_matches_surface_policy, prototype_surface_for_broad_edit_policy,
        validate_normal_repo_relpath,
    };

    let surface = prototype_surface_for_broad_edit_policy(edit_policy);
    let mut protected = Vec::new();
    let mut outside = Vec::new();
    for path in paths {
        let rel = if path.is_absolute() {
            match path.strip_prefix(workspace_path) {
                Ok(rel) => rel.to_path_buf(),
                Err(_) => {
                    outside.push(path.clone());
                    continue;
                }
            }
        } else {
            path.clone()
        };
        if validate_normal_repo_relpath(&rel).is_err() {
            outside.push(path.clone());
        } else if !path_matches_surface_policy(surface, &rel) {
            protected.push(rel);
        }
    }
    if !protected.is_empty() {
        Some(Reject::Protected { paths: protected })
    } else if !outside.is_empty() {
        Some(Reject::Outside { paths: outside })
    } else {
        None
    }
}

fn repair_prompt_feedback(feedback: &str) -> String {
    format!("The headless harness rejected a staged edit before applying it: {feedback}.")
}

pub(super) fn policy_repair_prompt(feedback: &str, has_applied_edits: bool) -> String {
    let mut prompt = String::new();
    prompt.push_str("Previous attempt result:\n");
    prompt.push_str(feedback);
    prompt.push_str("\n\n");
    prompt.push_str(
        "Protected core: see `crates/ploke-eval/src/cli/prototype1_state/backend.rs::EVAL_CORE_SURFACE_ROOT` and `WORKSPACE_EXCEPT_AUTHORITY_*`. Ordinary edits touching that surface will be rejected.\n",
    );
    if has_applied_edits {
        prompt
            .push_str("\nThe workspace already contains allowed edits from earlier tool calls.\n");
    } else {
        prompt.push_str("\nNo allowed source edit has been applied yet.\n");
    }
    prompt
}
