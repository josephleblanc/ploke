//! Eval-owned carrier boundary for a headless `ploke-tui` edit attempt.
//!
//! This module does not run chat sessions, route model calls, or duplicate the
//! `ploke-tui` app harness. It names the small amount of structure that
//! `ploke-eval` needs around the vanilla headless TUI path: an admitted request,
//! one or more attempts, observed event summaries, retry policy, and terminal
//! outcomes. `ploke-tui` remains the executor; `ploke-eval` owns the bounded
//! grant, surface check, and durable projection of what happened.

use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
    time::Duration,
};

use ploke_llm::{ModelId, ProviderKey};
use ploke_tui::app::commands::harness::TestAppAccessor;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::{
    ArtifactDelta,
    harness_request::{
        BroadEditPolicy, EvidenceRoot, EvidenceRootKind, EvidenceRootLocation, request,
    },
    surface, tui,
};

#[cfg(test)]
use super::harness_request::EvidenceRole;

const MAX_DEBUG_RELAY_EVENTS: usize = 128;
const MAX_DEBUG_RELAY_EVENT_CHARS: usize = 2_000;
const MAX_EVIDENCE_EVENT_CHARS: usize = 1_000;
const MAX_PROMPT_MESSAGE_PREVIEWS: usize = 8;
const MAX_PROMPT_MESSAGE_PREVIEW_CHARS: usize = 500;
const MAX_RAG_PART_PREVIEWS: usize = 8;
const LIVE_TRACE_ENV: &str = "PLOKE_EVAL_HEADLESS_TUI_LIVE";

pub(crate) mod state {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Ready {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Running {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Done {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelSelection {
    model_id: ModelId,
    provider: Option<ProviderKey>,
}

impl ModelSelection {
    pub(crate) fn new(model_id: ModelId, provider: Option<ProviderKey>) -> Self {
        Self { model_id, provider }
    }

    pub(crate) fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    pub(crate) fn provider(&self) -> Option<&ProviderKey> {
        self.provider.as_ref()
    }
}

/// Run one vanilla headless `ploke-tui` edit session for a broad request.
///
/// This is intentionally an executor adapter, not a second edit engine. It uses
/// a workspace-indexed `ploke-tui` runtime, observes tool/proposal events, and
/// approves only proposals whose paths stay within the broad prototype surface.
/// `ploke-eval` still validates the resulting workspace diff before admission.
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
    let mut run = HeadlessRun::new();
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

    let outcome = tokio::time::timeout(Duration::from_secs(budget.timeout_secs()), async {
        loop {
            observer.emit(format!("attempt {turn} start"));
            let (mut runtime, parent_id) = start_attempt_runtime(
                workspace_path,
                &extra_read_roots,
                next_prompt.clone(),
                model.as_ref(),
            )
            .await?;
            let end = run_attempt(
                &mut runtime,
                parent_id,
                workspace_path,
                edit_policy,
                turn,
                &mut run,
                &observer,
            )
            .await?;
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
                    observer.emit(format!(
                        "retry attempt={turn} feedback={}",
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
        Ok(result) => result?,
        Err(_) => HeadlessTerminal::TimedOut {
            secs: budget.timeout_secs(),
        },
    };
    observer.emit(format!("done {}", terminal.live_summary()));
    run.terminal = Some(terminal);
    Ok(run)
}

async fn start_attempt_runtime(
    workspace_path: &Path,
    extra_read_roots: &[PathBuf],
    prompt: String,
    model: Option<&ModelSelection>,
) -> Result<(crate::runner::WorkspaceTuiRuntime, Uuid), Error> {
    let runtime = crate::runner::setup_workspace_tui_runtime_with_read_roots(
        workspace_path,
        extra_read_roots,
    )
    .await
    .map_err(|source| Error::HeadlessStart(source.to_string()))?;

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
            cfg.model_registry
                .select_model_provider(&model.model_id, model.provider.as_ref());
        }
    }
    let parent_id = submit_prompt(&runtime.app, prompt).await?;
    Ok((runtime, parent_id))
}

enum AttemptEnd {
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
enum AppliedItem {
    Edit(Uuid),
    Create(Uuid),
}

impl AppliedItem {
    fn id(self) -> Uuid {
        match self {
            Self::Edit(id) | Self::Create(id) => id,
        }
    }
}

async fn run_attempt(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    mut active_parent_id: Uuid,
    workspace_path: &Path,
    edit_policy: BroadEditPolicy,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
) -> Result<AttemptEnd, Error> {
    use ploke_tui::{
        AppEvent,
        app_state::{StateCommand, events::SystemEvent},
    };

    let cmd_tx = runtime.app.state_cmd_tx();
    let mut pending_retry = None::<String>;
    let mut provider_failure = None::<String>;
    let mut applied = Vec::<AppliedItem>::new();
    let mut changed_paths = Vec::<PathBuf>::new();
    let mut policy_feedbacks = Vec::<String>::new();
    let mut policy_repair_turns = 0_u32;

    loop {
        runtime.app.pump_pending_events().await;
        drain_debug_observed(&mut runtime.debug_rx, run, observer, turn);

        let event = next_event(runtime).await?;

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
                observer.emit(format!(
                    "attempt {turn} tool_request call_id={} tool={} args={}",
                    tool_call.call_id,
                    tool_call.function.name.as_str(),
                    truncate_chars(&tool_call.function.arguments, 240)
                ));
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
                observer.emit(format!(
                    "attempt {turn} tool_completed call_id={} content={}",
                    call_id,
                    truncate_chars(&content, 240)
                ));
                let Some(payload) = ui_payload else {
                    continue;
                };
                if let Some(proposal_id) = payload.proposal_id {
                    let applied_item = AppliedItem::Edit(proposal_id);
                    if applied.contains(&applied_item) {
                        observer.emit(format!(
                            "attempt {turn} proposal_already_applied id={proposal_id}"
                        ));
                        continue;
                    }
                    let Some(proposal) = runtime
                        .state
                        .proposals
                        .read()
                        .await
                        .get(&proposal_id)
                        .cloned()
                    else {
                        continue;
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
                    if paths.is_empty() {
                        send_state(&cmd_tx, StateCommand::DenyEdits { proposal_id }).await?;
                        run.attempts.push(HeadlessAttempt {
                            turn,
                            proposal_id: Some(proposal_id),
                            result: HeadlessAttemptResult::Rejected {
                                reason: Feedback::from_outcome(&Outcome::Rejected(Reject::Empty))
                                    .message()
                                    .to_string(),
                            },
                        });
                        let feedback = "No material edit was staged; make a concrete bounded edit."
                            .to_string();
                        policy_feedbacks.push(repair_prompt_feedback(&feedback));
                        observer.emit(format!("attempt {turn} proposal_rejected empty"));
                        continue;
                    }

                    let rejection = classify_paths(workspace_path, edit_policy, &paths);
                    if let Some(rejection) = rejection {
                        let feedback = Feedback::from_outcome(&Outcome::Rejected(rejection));
                        send_state(&cmd_tx, StateCommand::DenyEdits { proposal_id }).await?;
                        run.attempts.push(HeadlessAttempt {
                            turn,
                            proposal_id: Some(proposal_id),
                            result: HeadlessAttemptResult::Rejected {
                                reason: feedback.message().to_string(),
                            },
                        });
                        let retry = feedback.message().to_string();
                        policy_feedbacks.push(repair_prompt_feedback(&retry));
                        observer.emit(format!(
                            "attempt {turn} proposal_rejected {}",
                            truncate_chars(feedback.message(), 240)
                        ));
                        continue;
                    }

                    match apply_edit(runtime, proposal_id, turn, run, observer).await? {
                        Ok(paths) => {
                            applied.push(applied_item);
                            push_changed_paths(&mut changed_paths, paths);
                        }
                        Err(feedback) => {
                            pending_retry = Some(feedback);
                        }
                    }
                } else if payload.tool == ploke_tui::tools::ToolName::CreateFile {
                    let applied_item = AppliedItem::Create(request_id);
                    if applied.contains(&applied_item) {
                        observer.emit(format!(
                            "attempt {turn} creation_already_applied id={request_id}"
                        ));
                        continue;
                    }
                    let Some(proposal) = runtime
                        .state
                        .create_proposals
                        .read()
                        .await
                        .get(&request_id)
                        .cloned()
                    else {
                        continue;
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
                    if paths.is_empty() {
                        send_state(&cmd_tx, StateCommand::DenyCreations { request_id }).await?;
                        run.attempts.push(HeadlessAttempt {
                            turn,
                            proposal_id: Some(request_id),
                            result: HeadlessAttemptResult::Rejected {
                                reason: Feedback::from_outcome(&Outcome::Rejected(Reject::Empty))
                                    .message()
                                    .to_string(),
                            },
                        });
                        let feedback =
                            "No material file creation was staged; make a concrete bounded edit."
                                .to_string();
                        policy_feedbacks.push(repair_prompt_feedback(&feedback));
                        observer.emit(format!("attempt {turn} creation_rejected empty"));
                        continue;
                    }

                    let rejection = classify_paths(workspace_path, edit_policy, &paths);
                    if let Some(rejection) = rejection {
                        let feedback = Feedback::from_outcome(&Outcome::Rejected(rejection));
                        send_state(&cmd_tx, StateCommand::DenyCreations { request_id }).await?;
                        run.attempts.push(HeadlessAttempt {
                            turn,
                            proposal_id: Some(request_id),
                            result: HeadlessAttemptResult::Rejected {
                                reason: feedback.message().to_string(),
                            },
                        });
                        let retry = feedback.message().to_string();
                        policy_feedbacks.push(repair_prompt_feedback(&retry));
                        observer.emit(format!(
                            "attempt {turn} creation_rejected {}",
                            truncate_chars(feedback.message(), 240)
                        ));
                        continue;
                    }

                    match apply_create(runtime, request_id, turn, run, observer).await? {
                        Ok(paths) => {
                            applied.push(applied_item);
                            push_changed_paths(&mut changed_paths, paths);
                        }
                        Err(feedback) => {
                            pending_retry = Some(feedback);
                        }
                    }
                }
            }
            AppEvent::System(SystemEvent::ToolCallFailed {
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
            }
            AppEvent::MessageUpdated(message) => {
                let assistant_error = {
                    let chat = runtime.state.chat.0.read().await;
                    chat.messages.get(&message.0).and_then(|message| {
                        if !matches!(
                            message.kind,
                            ploke_tui::chat_history::MessageKind::Assistant
                        ) {
                            return None;
                        }
                        if !matches!(
                            message.status,
                            ploke_tui::chat_history::MessageStatus::Error { .. }
                        ) {
                            return None;
                        }
                        Some((message.id, message.content.clone()))
                    })
                };
                let Some((message_id, content)) = assistant_error else {
                    continue;
                };
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
                if provider_failure.is_none() {
                    provider_failure = provider_unavailable_reason(&content);
                }
            }
            AppEvent::System(SystemEvent::ChatTurnFinished {
                request_id,
                parent_id,
                outcome,
                attempts,
                summary,
                ..
            }) if parent_id == active_parent_id => {
                run.events.push(Event::Turn {
                    request_id: request_id.to_string(),
                    outcome: outcome.clone(),
                    attempts,
                    summary: summary.clone(),
                });
                observer.emit(format!(
                    "attempt {turn} turn_finished outcome={} attempts={} summary={}",
                    outcome,
                    attempts,
                    truncate_chars(&summary, 240)
                ));
                if let Some(reason) = provider_failure.take() {
                    return Ok(AttemptEnd::Terminal(
                        HeadlessTerminal::ProviderUnavailable { reason },
                    ));
                }
                let repaired_failure = pending_retry.take();

                if !policy_feedbacks.is_empty() && policy_repair_turns < MAX_POLICY_REPAIR_TURNS {
                    let feedback = policy_feedbacks.join("\n");
                    policy_feedbacks.clear();
                    policy_repair_turns += 1;
                    let prompt = policy_repair_prompt(&feedback, !applied.is_empty());
                    active_parent_id = submit_prompt(&runtime.app, prompt).await?;
                    observer.emit(format!(
                        "attempt {turn} policy_repair_prompt parent={} count={} feedback={}",
                        active_parent_id,
                        policy_repair_turns,
                        truncate_chars(&feedback, 240)
                    ));
                    continue;
                }

                if outcome != "completed" {
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

                let terminal_id = applied
                    .first()
                    .map(|item| item.id())
                    .expect("applied is not empty");

                return Ok(AttemptEnd::Terminal(HeadlessTerminal::Applied {
                    proposal_id: terminal_id,
                    request_id,
                    changed_paths: changed_paths.clone(),
                }));
            }
            _ => {}
        }
    }
}

async fn apply_edit(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    proposal_id: Uuid,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
) -> Result<Result<Vec<PathBuf>, String>, Error> {
    use ploke_tui::app_state::{StateCommand, core::EditProposalStatus};

    let cmd_tx = runtime.app.state_cmd_tx();
    observer.emit(format!("attempt {turn} proposal_approve id={proposal_id}"));
    send_state(&cmd_tx, StateCommand::ApproveEdits { proposal_id }).await?;

    loop {
        runtime.app.pump_pending_events().await;
        drain_debug_observed(&mut runtime.debug_rx, run, observer, turn);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let Some(updated) = runtime
            .state
            .proposals
            .read()
            .await
            .get(&proposal_id)
            .cloned()
        else {
            let reason = format!("staged proposal {proposal_id} disappeared before apply");
            run.attempts.push(HeadlessAttempt {
                turn,
                proposal_id: Some(proposal_id),
                result: HeadlessAttemptResult::Rejected {
                    reason: reason.clone(),
                },
            });
            return Ok(Err(reason));
        };
        match updated.status {
            EditProposalStatus::Applied => {
                let paths = proposal_paths(&updated);
                run.attempts.push(HeadlessAttempt {
                    turn,
                    proposal_id: Some(proposal_id),
                    result: HeadlessAttemptResult::Applied {
                        paths: paths.clone(),
                    },
                });
                observer.emit(format!("attempt {turn} proposal_applied id={proposal_id}"));
                return Ok(Ok(paths));
            }
            EditProposalStatus::Failed(reason) | EditProposalStatus::Stale(reason) => {
                run.attempts.push(HeadlessAttempt {
                    turn,
                    proposal_id: Some(proposal_id),
                    result: HeadlessAttemptResult::Rejected {
                        reason: reason.clone(),
                    },
                });
                observer.emit(format!(
                    "attempt {turn} proposal_apply_failed id={} reason={}",
                    proposal_id,
                    truncate_chars(&reason, 240)
                ));
                return Ok(Err(reason));
            }
            EditProposalStatus::Denied => {
                let reason = "proposal was denied before apply".to_string();
                run.attempts.push(HeadlessAttempt {
                    turn,
                    proposal_id: Some(proposal_id),
                    result: HeadlessAttemptResult::Rejected {
                        reason: reason.clone(),
                    },
                });
                observer.emit(format!("attempt {turn} proposal_denied id={proposal_id}"));
                return Ok(Err(reason));
            }
            EditProposalStatus::Pending | EditProposalStatus::Approved => {}
        }
    }
}

async fn apply_create(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    request_id: Uuid,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
) -> Result<Result<Vec<PathBuf>, String>, Error> {
    use ploke_tui::app_state::{StateCommand, core::EditProposalStatus};

    let cmd_tx = runtime.app.state_cmd_tx();
    observer.emit(format!("attempt {turn} creation_approve id={request_id}"));
    send_state(&cmd_tx, StateCommand::ApproveCreations { request_id }).await?;

    loop {
        runtime.app.pump_pending_events().await;
        drain_debug_observed(&mut runtime.debug_rx, run, observer, turn);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let Some(updated) = runtime
            .state
            .create_proposals
            .read()
            .await
            .get(&request_id)
            .cloned()
        else {
            let reason = format!("staged file creation {request_id} disappeared before apply");
            run.attempts.push(HeadlessAttempt {
                turn,
                proposal_id: Some(request_id),
                result: HeadlessAttemptResult::Rejected {
                    reason: reason.clone(),
                },
            });
            return Ok(Err(reason));
        };
        match updated.status {
            EditProposalStatus::Applied => {
                let paths = updated.files.clone();
                run.attempts.push(HeadlessAttempt {
                    turn,
                    proposal_id: Some(request_id),
                    result: HeadlessAttemptResult::Applied {
                        paths: paths.clone(),
                    },
                });
                observer.emit(format!("attempt {turn} creation_applied id={request_id}"));
                return Ok(Ok(paths));
            }
            EditProposalStatus::Failed(reason) | EditProposalStatus::Stale(reason) => {
                run.attempts.push(HeadlessAttempt {
                    turn,
                    proposal_id: Some(request_id),
                    result: HeadlessAttemptResult::Rejected {
                        reason: reason.clone(),
                    },
                });
                observer.emit(format!(
                    "attempt {turn} creation_apply_failed id={} reason={}",
                    request_id,
                    truncate_chars(&reason, 240)
                ));
                return Ok(Err(reason));
            }
            EditProposalStatus::Denied => {
                let reason = "file creation was denied before apply".to_string();
                run.attempts.push(HeadlessAttempt {
                    turn,
                    proposal_id: Some(request_id),
                    result: HeadlessAttemptResult::Rejected {
                        reason: reason.clone(),
                    },
                });
                observer.emit(format!("attempt {turn} creation_denied id={request_id}"));
                return Ok(Err(reason));
            }
            EditProposalStatus::Pending | EditProposalStatus::Approved => {}
        }
    }
}

fn provider_unavailable_reason(content: &str) -> Option<String> {
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
    None
}

fn advance_turn(budget: &Budget, turn: &mut u32) -> bool {
    if *turn >= budget.max_attempts() {
        return false;
    }
    *turn += 1;
    true
}

fn attempt_prompt(
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

fn evidence_read_roots(evidence_roots: &[EvidenceRoot]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for root in evidence_roots {
        if root.kind == EvidenceRootKind::SubmittedResultOutput {
            continue;
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

fn retry_feedback(feedback: &str) -> String {
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
struct LiveObserver {
    enabled: bool,
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
        }
    }

    fn emit(&self, message: impl AsRef<str>) {
        if self.enabled {
            eprintln!("[headless-tui] {}", message.as_ref());
        }
    }
}

async fn submit_prompt(app: &ploke_tui::app::App, content: String) -> Result<Uuid, Error> {
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

async fn next_event(
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

fn classify_paths(
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

fn push_changed_paths(changed_paths: &mut Vec<PathBuf>, paths: Vec<PathBuf>) {
    for path in paths {
        if !changed_paths.contains(&path) {
            changed_paths.push(path);
        }
    }
}

fn repair_prompt_feedback(feedback: &str) -> String {
    format!("The headless harness rejected a staged edit before applying it: {feedback}.")
}

fn policy_repair_prompt(feedback: &str, has_applied_edits: bool) -> String {
    let mut prompt = String::new();
    prompt.push_str("Previous attempt result:\n");
    prompt.push_str(feedback);
    prompt.push_str("\n\n");
    push_blocked_paths(&mut prompt);
    if has_applied_edits {
        prompt
            .push_str("\nThe workspace already contains allowed edits from earlier tool calls.\n");
    } else {
        prompt.push_str("\nNo allowed source edit has been applied yet.\n");
    }
    prompt
}

fn push_blocked_paths(prompt: &mut String) {
    use crate::cli::prototype1_state::backend::{
        WORKSPACE_EXCEPT_AUTHORITY_FILENAMES, WORKSPACE_EXCEPT_AUTHORITY_PREFIXES,
    };

    prompt.push_str("Do not edit files under:\n");
    for prefix in WORKSPACE_EXCEPT_AUTHORITY_PREFIXES {
        prompt.push_str(&format!("- `{}/`\n", prefix.trim_end_matches('/')));
    }

    prompt.push_str("\nDo not edit files named:\n");
    for filename in WORKSPACE_EXCEPT_AUTHORITY_FILENAMES {
        prompt.push_str(&format!("- `{}`\n", filename));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadlessRun {
    attempts: Vec<HeadlessAttempt>,
    events: Vec<Event>,
    debug_relay: DebugRelay,
    prompt_diagnostics: Vec<PromptDiagnostic>,
    terminal: Option<HeadlessTerminal>,
}

impl HeadlessRun {
    fn new() -> Self {
        Self {
            attempts: Vec::new(),
            events: Vec::new(),
            debug_relay: DebugRelay::new(),
            prompt_diagnostics: Vec::new(),
            terminal: None,
        }
    }

    pub(crate) fn attempts(&self) -> &[HeadlessAttempt] {
        &self.attempts
    }

    pub(crate) fn events(&self) -> &[Event] {
        &self.events
    }

    pub(crate) fn terminal(&self) -> Option<&HeadlessTerminal> {
        self.terminal.as_ref()
    }

    pub(crate) fn debug_relay(&self) -> &DebugRelay {
        &self.debug_relay
    }

    pub(crate) fn prompt_diagnostics(&self) -> &[PromptDiagnostic] {
        &self.prompt_diagnostics
    }

    pub(crate) fn evidence(&self) -> evidence::Summary {
        evidence::Summary::from(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptDiagnostic {
    parent_id: String,
    workspace: WorkspaceDiagnostic,
    bm25: Option<Bm25Diagnostic>,
    context_mode: String,
    max_leased_tokens: usize,
    estimated_total_tokens: usize,
    message_count: usize,
    message_previews: Vec<MessagePreview>,
    included_rag_parts: usize,
    rag_part_previews: Vec<RagPartPreview>,
    rag_stats: Option<ContextStatsDiagnostic>,
    fallback_notice: Option<String>,
}

impl PromptDiagnostic {
    async fn capture(
        state: &std::sync::Arc<ploke_tui::app_state::AppState>,
        parent_id: Uuid,
        formatted_prompt: &[ploke_tui::llm::RequestMessage],
        context_plan: &ploke_tui::llm::ContextPlan,
    ) -> Self {
        let (root, member_roots, focused_root) = state
            .with_system_read(|sys| {
                (
                    sys.loaded_workspace_root(),
                    sys.loaded_workspace_member_roots(),
                    sys.focused_crate_root(),
                )
            })
            .await;
        let (context_mode, max_leased_tokens) = {
            let cfg = state.config.read().await;
            (
                format!("{:?}", cfg.context_management.mode),
                cfg.context_management.max_leased_tokens,
            )
        };
        let bm25 = match state.rag.as_ref() {
            Some(rag) => match rag.bm25_status().await {
                Ok(status) => Some(Bm25Diagnostic::from(status)),
                Err(err) => Some(Bm25Diagnostic {
                    status: "status_error".to_string(),
                    docs: None,
                    error: Some(err.to_string()),
                }),
            },
            None => None,
        };
        Self {
            parent_id: parent_id.to_string(),
            workspace: WorkspaceDiagnostic {
                loaded: root.is_some(),
                root,
                member_count: member_roots.len(),
                focused_root,
            },
            bm25,
            context_mode,
            max_leased_tokens,
            estimated_total_tokens: context_plan.estimated_total_tokens,
            message_count: formatted_prompt.len(),
            message_previews: formatted_prompt
                .iter()
                .take(MAX_PROMPT_MESSAGE_PREVIEWS)
                .map(MessagePreview::from)
                .collect(),
            included_rag_parts: context_plan.included_rag_parts.len(),
            rag_part_previews: context_plan
                .included_rag_parts
                .iter()
                .take(MAX_RAG_PART_PREVIEWS)
                .map(RagPartPreview::from)
                .collect(),
            rag_stats: context_plan
                .rag_stats
                .as_ref()
                .map(ContextStatsDiagnostic::from),
            fallback_notice: fallback_notice(formatted_prompt),
        }
    }

    fn context_unavailable_reason(&self) -> Option<String> {
        if self.context_mode == "Off" {
            return None;
        }
        self.fallback_notice
            .as_ref()
            .filter(|notice| notice.contains("without code context"))
            .cloned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceDiagnostic {
    loaded: bool,
    root: Option<PathBuf>,
    member_count: usize,
    focused_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bm25Diagnostic {
    status: String,
    docs: Option<usize>,
    error: Option<String>,
}

impl From<ploke_db::bm25_index::bm25_service::Bm25Status> for Bm25Diagnostic {
    fn from(value: ploke_db::bm25_index::bm25_service::Bm25Status) -> Self {
        match value {
            ploke_db::bm25_index::bm25_service::Bm25Status::Uninitialized => Self {
                status: "uninitialized".to_string(),
                docs: None,
                error: None,
            },
            ploke_db::bm25_index::bm25_service::Bm25Status::Building => Self {
                status: "building".to_string(),
                docs: None,
                error: None,
            },
            ploke_db::bm25_index::bm25_service::Bm25Status::Ready { docs } => Self {
                status: "ready".to_string(),
                docs: Some(docs),
                error: None,
            },
            ploke_db::bm25_index::bm25_service::Bm25Status::Empty => Self {
                status: "empty".to_string(),
                docs: Some(0),
                error: None,
            },
            ploke_db::bm25_index::bm25_service::Bm25Status::Error(error) => Self {
                status: "error".to_string(),
                docs: None,
                error: Some(error),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MessagePreview {
    role: String,
    chars: usize,
    preview: String,
}

impl From<&ploke_tui::llm::RequestMessage> for MessagePreview {
    fn from(value: &ploke_tui::llm::RequestMessage) -> Self {
        Self {
            role: format!("{:?}", value.role),
            chars: value.content.chars().count(),
            preview: truncate_chars(&value.content, MAX_PROMPT_MESSAGE_PREVIEW_CHARS),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RagPartPreview {
    file_path: String,
    kind: String,
    estimated_tokens: usize,
    score: f32,
}

impl Eq for RagPartPreview {}

impl From<&ploke_tui::llm::ContextPlanRagPart> for RagPartPreview {
    fn from(value: &ploke_tui::llm::ContextPlanRagPart) -> Self {
        Self {
            file_path: value.file_path.clone(),
            kind: format!("{:?}", value.kind),
            estimated_tokens: value.estimated_tokens,
            score: value.score,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextStatsDiagnostic {
    total_tokens: usize,
    files: usize,
    parts: usize,
    truncated_parts: usize,
    dedup_removed: usize,
}

impl From<&ploke_core::rag_types::ContextStats> for ContextStatsDiagnostic {
    fn from(value: &ploke_core::rag_types::ContextStats) -> Self {
        Self {
            total_tokens: value.total_tokens,
            files: value.files,
            parts: value.parts,
            truncated_parts: value.truncated_parts,
            dedup_removed: value.dedup_removed,
        }
    }
}

fn fallback_notice(messages: &[ploke_tui::llm::RequestMessage]) -> Option<String> {
    messages
        .first()
        .filter(|message| message.content.contains("proceeding without code context"))
        .map(|message| message.content.clone())
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in input.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DebugRelay {
    retained: Vec<String>,
    dropped: u64,
    truncated: u64,
}

impl DebugRelay {
    fn new() -> Self {
        Self {
            retained: Vec::new(),
            dropped: 0,
            truncated: 0,
        }
    }

    fn push(&mut self, message: &str) {
        let mut chars = message.chars();
        let retained = chars
            .by_ref()
            .take(MAX_DEBUG_RELAY_EVENT_CHARS)
            .collect::<String>();
        if chars.next().is_some() {
            self.truncated += 1;
        }

        if self.retained.len() == MAX_DEBUG_RELAY_EVENTS {
            self.retained.remove(0);
            self.dropped += 1;
        }
        self.retained.push(retained);
    }

    pub(crate) fn retained(&self) -> &[String] {
        &self.retained
    }

    pub(crate) fn dropped(&self) -> u64 {
        self.dropped
    }

    pub(crate) fn truncated(&self) -> u64 {
        self.truncated
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadlessAttempt {
    turn: u32,
    proposal_id: Option<Uuid>,
    result: HeadlessAttemptResult,
}

impl HeadlessAttempt {
    pub(crate) fn turn(&self) -> u32 {
        self.turn
    }

    pub(crate) fn proposal_id(&self) -> Option<Uuid> {
        self.proposal_id
    }

    pub(crate) fn result(&self) -> &HeadlessAttemptResult {
        &self.result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HeadlessAttemptResult {
    Applied { paths: Vec<PathBuf> },
    Rejected { reason: String },
    NoEdit { summary: String },
    ToolFailed { error: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HeadlessTerminal {
    Applied {
        proposal_id: Uuid,
        request_id: Uuid,
        changed_paths: Vec<PathBuf>,
    },
    Exhausted {
        attempts: u32,
        last: String,
    },
    CompletedWithoutEdit {
        outcome: String,
        summary: String,
    },
    ToolFailed {
        error: String,
    },
    NoEdit,
    ContextUnavailable {
        reason: String,
    },
    ProviderUnavailable {
        reason: String,
    },
    TimedOut {
        secs: u64,
    },
}

impl HeadlessTerminal {
    fn live_summary(&self) -> String {
        match self {
            Self::Applied {
                proposal_id,
                changed_paths,
                ..
            } => format!(
                "applied proposal_id={} changed_paths={}",
                proposal_id,
                join_paths(changed_paths)
            ),
            Self::Exhausted { attempts, last } => format!(
                "exhausted attempts={} last={}",
                attempts,
                truncate_chars(last, 240)
            ),
            Self::CompletedWithoutEdit { outcome, summary } => format!(
                "completed_without_edit outcome={} summary={}",
                outcome,
                truncate_chars(summary, 240)
            ),
            Self::ToolFailed { error } => {
                format!("tool_failed error={}", truncate_chars(error, 240))
            }
            Self::NoEdit => "no_edit".to_string(),
            Self::ContextUnavailable { reason } => {
                format!("context_unavailable reason={}", truncate_chars(reason, 240))
            }
            Self::ProviderUnavailable { reason } => {
                format!(
                    "provider_unavailable reason={}",
                    truncate_chars(reason, 240)
                )
            }
            Self::TimedOut { secs } => format!("timed_out secs={secs}"),
        }
    }
}

pub(crate) mod evidence {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::{
        DebugRelay, HeadlessAttemptResult, HeadlessRun, HeadlessTerminal, MAX_EVIDENCE_EVENT_CHARS,
        truncate_chars,
    };

    /// Compact executor observations. Backend admission must still validate the
    /// workspace diff before any loop state advances.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Summary {
        pub(crate) attempts: Vec<Attempt>,
        pub(crate) terminal: Option<Terminal>,
        #[serde(default)]
        pub(crate) events: Vec<Event>,
        #[serde(default)]
        pub(crate) debug_relay: DebugRelaySummary,
        #[serde(default)]
        pub(crate) prompt_diagnostics: Vec<Prompt>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
    pub(crate) struct DebugRelaySummary {
        pub(crate) retained: Vec<String>,
        pub(crate) dropped: u64,
        pub(crate) truncated: u64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Text {
        pub(crate) chars: usize,
        pub(crate) preview: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub(crate) enum Event {
        Proposal {
            id: String,
            edit_count: usize,
            paths: Vec<PathBuf>,
        },
        ToolRequest {
            request_id: String,
            parent_id: String,
            call_id: String,
            tool: String,
            arguments: Text,
        },
        ToolCompleted {
            call_id: String,
            content: Text,
        },
        ToolFailed {
            call_id: String,
            error: Text,
        },
        AssistantMessage {
            id: String,
            status: String,
            content: Text,
        },
        Turn {
            request_id: String,
            outcome: String,
            attempts: u32,
            summary: Text,
        },
        Outcome {
            outcome: super::record::Outcome,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub(crate) struct Prompt {
        pub(crate) parent_id: String,
        pub(crate) workspace: Workspace,
        pub(crate) bm25: Option<Bm25>,
        pub(crate) context_mode: String,
        pub(crate) max_leased_tokens: usize,
        pub(crate) estimated_total_tokens: usize,
        pub(crate) message_count: usize,
        pub(crate) message_previews: Vec<Message>,
        pub(crate) included_rag_parts: usize,
        pub(crate) rag_part_previews: Vec<RagPart>,
        pub(crate) rag_stats: Option<ContextStats>,
        pub(crate) fallback_notice: Option<String>,
    }

    impl Eq for Prompt {}

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Workspace {
        pub(crate) loaded: bool,
        pub(crate) root: Option<PathBuf>,
        pub(crate) member_count: usize,
        pub(crate) focused_root: Option<PathBuf>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Bm25 {
        pub(crate) status: String,
        pub(crate) docs: Option<usize>,
        pub(crate) error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Message {
        pub(crate) role: String,
        pub(crate) chars: usize,
        pub(crate) preview: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub(crate) struct RagPart {
        pub(crate) file_path: String,
        pub(crate) kind: String,
        pub(crate) estimated_tokens: usize,
        pub(crate) score: f32,
    }

    impl Eq for RagPart {}

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct ContextStats {
        pub(crate) total_tokens: usize,
        pub(crate) files: usize,
        pub(crate) parts: usize,
        pub(crate) truncated_parts: usize,
        pub(crate) dedup_removed: usize,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Attempt {
        pub(crate) turn: u32,
        pub(crate) proposal_id: Option<String>,
        pub(crate) result: Result,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "result", rename_all = "snake_case")]
    pub(crate) enum Result {
        Applied { paths: Vec<PathBuf> },
        Rejected { feedback: String },
        NoEdit { summary: String },
        ToolFailed { error: String },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "terminal", rename_all = "snake_case")]
    pub(crate) enum Terminal {
        Applied {
            proposal_id: String,
            request_id: String,
            changed_paths: Vec<PathBuf>,
        },
        Exhausted {
            attempts: u32,
            last_feedback: String,
        },
        CompletedWithoutEdit {
            outcome: String,
            summary: String,
        },
        ToolFailed {
            error: String,
        },
        NoEdit,
        ContextUnavailable {
            reason: String,
        },
        ProviderUnavailable {
            reason: String,
        },
        TimedOut {
            secs: u64,
        },
    }

    impl From<&HeadlessRun> for Summary {
        fn from(value: &HeadlessRun) -> Self {
            Self {
                attempts: value.attempts.iter().map(Attempt::from).collect(),
                terminal: value.terminal.as_ref().map(Terminal::from),
                events: value.events.iter().map(Event::from).collect(),
                debug_relay: DebugRelaySummary::from(&value.debug_relay),
                prompt_diagnostics: value.prompt_diagnostics.iter().map(Prompt::from).collect(),
            }
        }
    }

    impl From<&super::Event> for Event {
        fn from(value: &super::Event) -> Self {
            match value {
                super::Event::Proposal {
                    id,
                    edit_count,
                    paths,
                } => Self::Proposal {
                    id: id.clone(),
                    edit_count: *edit_count,
                    paths: paths.clone(),
                },
                super::Event::ToolRequest {
                    request_id,
                    parent_id,
                    call_id,
                    tool,
                    arguments,
                } => Self::ToolRequest {
                    request_id: request_id.clone(),
                    parent_id: parent_id.clone(),
                    call_id: call_id.clone(),
                    tool: tool.clone(),
                    arguments: Text::from(arguments.as_str()),
                },
                super::Event::Tool { call_id, result } => match result {
                    super::Tool::Completed { content } => Self::ToolCompleted {
                        call_id: call_id.clone(),
                        content: Text::from(content.as_str()),
                    },
                    super::Tool::Failed { error } => Self::ToolFailed {
                        call_id: call_id.clone(),
                        error: Text::from(error.as_str()),
                    },
                },
                super::Event::AssistantMessage {
                    id,
                    status,
                    content,
                } => Self::AssistantMessage {
                    id: id.clone(),
                    status: status.clone(),
                    content: Text::from(content.as_str()),
                },
                super::Event::Turn {
                    request_id,
                    outcome,
                    attempts,
                    summary,
                } => Self::Turn {
                    request_id: request_id.clone(),
                    outcome: outcome.clone(),
                    attempts: *attempts,
                    summary: Text::from(summary.as_str()),
                },
                super::Event::Outcome(outcome) => Self::Outcome {
                    outcome: outcome.clone(),
                },
            }
        }
    }

    impl From<&str> for Text {
        fn from(value: &str) -> Self {
            Self {
                chars: value.chars().count(),
                preview: truncate_chars(value, MAX_EVIDENCE_EVENT_CHARS),
            }
        }
    }

    impl From<&super::PromptDiagnostic> for Prompt {
        fn from(value: &super::PromptDiagnostic) -> Self {
            Self {
                parent_id: value.parent_id.clone(),
                workspace: Workspace::from(&value.workspace),
                bm25: value.bm25.as_ref().map(Bm25::from),
                context_mode: value.context_mode.clone(),
                max_leased_tokens: value.max_leased_tokens,
                estimated_total_tokens: value.estimated_total_tokens,
                message_count: value.message_count,
                message_previews: value.message_previews.iter().map(Message::from).collect(),
                included_rag_parts: value.included_rag_parts,
                rag_part_previews: value.rag_part_previews.iter().map(RagPart::from).collect(),
                rag_stats: value.rag_stats.as_ref().map(ContextStats::from),
                fallback_notice: value.fallback_notice.clone(),
            }
        }
    }

    impl From<&super::WorkspaceDiagnostic> for Workspace {
        fn from(value: &super::WorkspaceDiagnostic) -> Self {
            Self {
                loaded: value.loaded,
                root: value.root.clone(),
                member_count: value.member_count,
                focused_root: value.focused_root.clone(),
            }
        }
    }

    impl From<&super::Bm25Diagnostic> for Bm25 {
        fn from(value: &super::Bm25Diagnostic) -> Self {
            Self {
                status: value.status.clone(),
                docs: value.docs,
                error: value.error.clone(),
            }
        }
    }

    impl From<&super::MessagePreview> for Message {
        fn from(value: &super::MessagePreview) -> Self {
            Self {
                role: value.role.clone(),
                chars: value.chars,
                preview: value.preview.clone(),
            }
        }
    }

    impl From<&super::RagPartPreview> for RagPart {
        fn from(value: &super::RagPartPreview) -> Self {
            Self {
                file_path: value.file_path.clone(),
                kind: value.kind.clone(),
                estimated_tokens: value.estimated_tokens,
                score: value.score,
            }
        }
    }

    impl From<&super::ContextStatsDiagnostic> for ContextStats {
        fn from(value: &super::ContextStatsDiagnostic) -> Self {
            Self {
                total_tokens: value.total_tokens,
                files: value.files,
                parts: value.parts,
                truncated_parts: value.truncated_parts,
                dedup_removed: value.dedup_removed,
            }
        }
    }

    impl From<&DebugRelay> for DebugRelaySummary {
        fn from(value: &DebugRelay) -> Self {
            Self {
                retained: value.retained().to_vec(),
                dropped: value.dropped(),
                truncated: value.truncated(),
            }
        }
    }

    impl From<&super::HeadlessAttempt> for Attempt {
        fn from(value: &super::HeadlessAttempt) -> Self {
            Self {
                turn: value.turn,
                proposal_id: value.proposal_id.map(|id| id.to_string()),
                result: Result::from(&value.result),
            }
        }
    }

    impl From<&HeadlessAttemptResult> for Result {
        fn from(value: &HeadlessAttemptResult) -> Self {
            match value {
                HeadlessAttemptResult::Applied { paths } => Self::Applied {
                    paths: paths.clone(),
                },
                HeadlessAttemptResult::Rejected { reason } => Self::Rejected {
                    feedback: reason.clone(),
                },
                HeadlessAttemptResult::NoEdit { summary } => Self::NoEdit {
                    summary: summary.clone(),
                },
                HeadlessAttemptResult::ToolFailed { error } => Self::ToolFailed {
                    error: error.clone(),
                },
            }
        }
    }

    impl From<&HeadlessTerminal> for Terminal {
        fn from(value: &HeadlessTerminal) -> Self {
            match value {
                HeadlessTerminal::Applied {
                    proposal_id,
                    request_id,
                    changed_paths,
                } => Self::Applied {
                    proposal_id: proposal_id.to_string(),
                    request_id: request_id.to_string(),
                    changed_paths: changed_paths.clone(),
                },
                HeadlessTerminal::Exhausted { attempts, last } => Self::Exhausted {
                    attempts: *attempts,
                    last_feedback: last.clone(),
                },
                HeadlessTerminal::CompletedWithoutEdit { outcome, summary } => {
                    Self::CompletedWithoutEdit {
                        outcome: outcome.clone(),
                        summary: summary.clone(),
                    }
                }
                HeadlessTerminal::ToolFailed { error } => Self::ToolFailed {
                    error: error.clone(),
                },
                HeadlessTerminal::NoEdit => Self::NoEdit,
                HeadlessTerminal::ContextUnavailable { reason } => Self::ContextUnavailable {
                    reason: reason.clone(),
                },
                HeadlessTerminal::ProviderUnavailable { reason } => Self::ProviderUnavailable {
                    reason: reason.clone(),
                },
                HeadlessTerminal::TimedOut { secs } => Self::TimedOut { secs: *secs },
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    reference: request::Reference<request::Broad, request::Published>,
    request_path: PathBuf,
    prompt_path: PathBuf,
    workspace_path: PathBuf,
    evidence_roots: Vec<EvidenceRoot>,
    grant: surface::Grant,
    bounds: tui::Bounds,
    budget: Budget,
}

impl Request {
    pub(crate) fn from_published(
        published: &request::Request<request::Broad, request::Published>,
        grant: surface::Grant,
        bounds: tui::Bounds,
        budget: Budget,
    ) -> Self {
        Self {
            reference: published.reference(),
            request_path: published.request_path().to_path_buf(),
            prompt_path: published.prompt_path().to_path_buf(),
            workspace_path: published.workspace_path().to_path_buf(),
            evidence_roots: published.request().evidence_roots.clone(),
            grant,
            bounds,
            budget,
        }
    }

    pub(crate) fn reference(&self) -> &request::Reference<request::Broad, request::Published> {
        &self.reference
    }

    pub(crate) fn request_path(&self) -> &Path {
        &self.request_path
    }

    pub(crate) fn prompt_path(&self) -> &Path {
        &self.prompt_path
    }

    pub(crate) fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    pub(crate) fn evidence_roots(&self) -> &[EvidenceRoot] {
        &self.evidence_roots
    }

    pub(crate) fn grant(&self) -> &surface::Grant {
        &self.grant
    }

    pub(crate) fn bounds(&self) -> &tui::Bounds {
        &self.bounds
    }

    pub(crate) fn budget(&self) -> &Budget {
        &self.budget
    }

    pub(crate) fn attempt(&self, number: u32) -> Attempt<state::Ready> {
        Attempt {
            core: Core {
                id: Id::for_request(self.reference.request_id(), number),
                number,
                request: self.clone(),
                events: Vec::new(),
            },
            state: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Budget {
    max_attempts: u32,
    timeout_secs: u64,
}

impl Budget {
    pub(crate) fn new(max_attempts: u32, timeout_secs: u64) -> Result<Self, Error> {
        if max_attempts == 0 {
            return Err(Error::EmptyBudget);
        }
        if timeout_secs == 0 {
            return Err(Error::EmptyTimeout);
        }
        Ok(Self {
            max_attempts,
            timeout_secs,
        })
    }

    pub(crate) fn max_attempts(self) -> u32 {
        self.max_attempts
    }

    pub(crate) fn timeout_secs(self) -> u64 {
        self.timeout_secs
    }

    pub(crate) fn retry(self) -> Retry {
        Retry {
            max_attempts: self.max_attempts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Id(String);

impl Id {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn for_request(request_id: &str, number: u32) -> Self {
        Self(format!("{request_id}:attempt-{number}"))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Core {
    id: Id,
    number: u32,
    request: Request,
    events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attempt<S> {
    core: Core,
    state: PhantomData<fn() -> S>,
}

impl Attempt<state::Ready> {
    pub(crate) fn start(self) -> Attempt<state::Running> {
        Attempt {
            core: self.core,
            state: PhantomData,
        }
    }
}

impl Attempt<state::Running> {
    pub(crate) fn observe(mut self, event: Event) -> Self {
        self.core.events.push(event);
        self
    }

    pub(crate) fn stage(&self, proposal: Proposal) -> Result<surface::Check, Error> {
        proposal
            .check(self.core.request.grant())
            .map_err(Error::Surface)
    }

    pub(crate) fn finish(self, outcome: Outcome) -> Attempt<state::Done> {
        let mut core = self.core;
        core.events.push(Event::Outcome(outcome.to_record()));
        Attempt {
            core,
            state: PhantomData,
        }
    }
}

impl<S> Attempt<S> {
    pub(crate) fn id(&self) -> &Id {
        &self.core.id
    }

    pub(crate) fn number(&self) -> u32 {
        self.core.number
    }

    pub(crate) fn request(&self) -> &Request {
        &self.core.request
    }

    pub(crate) fn events(&self) -> &[Event] {
        &self.core.events
    }
}

impl Attempt<state::Done> {
    pub(crate) fn outcome(&self) -> Option<Outcome> {
        self.core.events.iter().rev().find_map(Event::outcome)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Proposal {
    id: String,
    base: surface::Ref,
    after: surface::Ref,
    touches: Vec<surface::Touch>,
}

impl Proposal {
    pub(crate) fn new(
        id: impl Into<String>,
        base: surface::Ref,
        after: surface::Ref,
        touches: Vec<surface::Touch>,
    ) -> Self {
        Self {
            id: id.into(),
            base,
            after,
            touches,
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn touches(&self) -> &[surface::Touch] {
        &self.touches
    }

    pub(crate) fn check(&self, grant: &surface::Grant) -> Result<surface::Check, surface::Error> {
        grant.check(surface::Draft {
            proposal: &self.id,
            base: &self.base,
            after: &self.after,
            touches: &self.touches,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    Applied(Applied),
    Rejected(Reject),
    NoEdit(NoEdit),
    Tool(Fail),
    TimedOut(Timeout),
    Validation(Fail),
}

impl Outcome {
    pub(crate) fn applied(check: surface::Check, validation: Validation) -> Self {
        Self::Applied(Applied {
            delta: ArtifactDelta::from_check(check),
            validation,
        })
    }

    pub(crate) fn retryable(&self) -> bool {
        match self {
            Self::Applied(_) => false,
            Self::Rejected(_)
            | Self::NoEdit(_)
            | Self::Tool(_)
            | Self::TimedOut(_)
            | Self::Validation(_) => true,
        }
    }

    fn to_record(&self) -> record::Outcome {
        record::Outcome::from(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Applied {
    delta: ArtifactDelta,
    validation: Validation,
}

impl Applied {
    pub(crate) fn delta(&self) -> &ArtifactDelta {
        &self.delta
    }

    pub(crate) fn validation(&self) -> &Validation {
        &self.validation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reject {
    Protected { paths: Vec<PathBuf> },
    Outside { paths: Vec<PathBuf> },
    Empty,
    Invalid { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoEdit {
    summary: String,
}

impl NoEdit {
    pub(crate) fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
        }
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fail {
    kind: FailKind,
    detail: String,
}

impl Fail {
    pub(crate) fn tool(detail: impl Into<String>) -> Self {
        Self {
            kind: FailKind::Tool,
            detail: detail.into(),
        }
    }

    pub(crate) fn validation(detail: impl Into<String>) -> Self {
        Self {
            kind: FailKind::Validation,
            detail: detail.into(),
        }
    }

    pub(crate) fn detail(&self) -> &str {
        &self.detail
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailKind {
    Tool,
    Validation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Timeout {
    secs: u64,
}

impl Timeout {
    pub(crate) fn new(secs: u64) -> Self {
        Self { secs }
    }

    pub(crate) fn secs(self) -> u64 {
        self.secs
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Validation {
    NotRun,
    Passed { commands: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Event {
    Proposal {
        id: String,
        edit_count: usize,
        paths: Vec<PathBuf>,
    },
    ToolRequest {
        request_id: String,
        parent_id: String,
        call_id: String,
        tool: String,
        arguments: String,
    },
    Tool {
        call_id: String,
        result: Tool,
    },
    AssistantMessage {
        id: String,
        status: String,
        content: String,
    },
    Turn {
        request_id: String,
        outcome: String,
        attempts: u32,
        summary: String,
    },
    Outcome(record::Outcome),
}

impl Event {
    fn outcome(&self) -> Option<Outcome> {
        match self {
            Self::Outcome(record) => Some(Outcome::from(record)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Tool {
    Completed { content: String },
    Failed { error: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Retry {
    max_attempts: u32,
}

impl Retry {
    pub(crate) fn decide(&self, attempt: &Attempt<state::Done>) -> Step {
        let Some(outcome) = attempt.outcome() else {
            return Step::Terminal(Terminal::Invalid {
                reason: "finished attempt did not record an outcome".to_string(),
            });
        };
        if !outcome.retryable() {
            return Step::Terminal(Terminal::Accepted(match outcome {
                Outcome::Applied(applied) => applied,
                _ => unreachable!("non-retryable outcome must be applied"),
            }));
        }
        if attempt.number() >= self.max_attempts {
            Step::Terminal(Terminal::Exhausted {
                attempts: attempt.number(),
                last: outcome,
            })
        } else {
            Step::Retry {
                next_attempt: attempt.number() + 1,
                feedback: Feedback::from_outcome(&outcome),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    Retry {
        next_attempt: u32,
        feedback: Feedback,
    },
    Terminal(Terminal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Terminal {
    Accepted(Applied),
    Exhausted { attempts: u32, last: Outcome },
    Invalid { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Feedback {
    message: String,
}

impl Feedback {
    fn from_outcome(outcome: &Outcome) -> Self {
        let message = match outcome {
            Outcome::Applied(_) => "candidate accepted".to_string(),
            Outcome::Rejected(Reject::Protected { paths }) => {
                format!("Rejected protected paths: {}", join_paths(paths))
            }
            Outcome::Rejected(Reject::Outside { paths }) => {
                format!("Rejected out-of-surface paths: {}", join_paths(paths))
            }
            Outcome::Rejected(Reject::Empty) => {
                "No material patch was produced; try a concrete edit.".to_string()
            }
            Outcome::Rejected(Reject::Invalid { reason }) => reason.clone(),
            Outcome::NoEdit(no_edit) => no_edit.summary().to_string(),
            Outcome::Tool(fail) | Outcome::Validation(fail) => fail.detail().to_string(),
            Outcome::TimedOut(timeout) => format!(
                "The headless TUI attempt timed out after {} seconds; try again with a smaller edit.",
                timeout.secs()
            ),
        };
        Self { message }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("adapter attempt budget must allow at least one attempt")]
    EmptyBudget,
    #[error("adapter timeout must be nonzero")]
    EmptyTimeout,
    #[error("failed to start headless ploke-tui harness: {0}")]
    HeadlessStart(String),
    #[error("headless ploke-tui event stream failed: {0}")]
    HeadlessEvent(String),
    #[error("proposal failed surface check: {0}")]
    Surface(#[from] surface::Error),
}

fn join_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) mod record {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::{Applied, Fail, NoEdit, Outcome as ActiveOutcome, Reject, Timeout, Validation};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Run {
        pub(crate) request_id: String,
        pub(crate) request_hash: String,
        pub(crate) attempts: Vec<Attempt>,
        pub(crate) terminal: Option<Terminal>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Attempt {
        pub(crate) id: String,
        pub(crate) number: u32,
        pub(crate) outcome: Option<Outcome>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub(crate) enum Terminal {
        Accepted { changed_paths: Vec<PathBuf> },
        Exhausted { attempts: u32, last: Outcome },
        Invalid { reason: String },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "outcome", rename_all = "snake_case")]
    pub(crate) enum Outcome {
        Applied {
            base_hash: String,
            after_hash: String,
            changed_paths: Vec<PathBuf>,
            validation: Validation,
        },
        Rejected {
            reason: RejectRecord,
        },
        NoEdit {
            summary: String,
        },
        Failed {
            fail_kind: super::FailKind,
            detail: String,
        },
        TimedOut {
            secs: u64,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub(crate) enum RejectRecord {
        Protected { paths: Vec<PathBuf> },
        Outside { paths: Vec<PathBuf> },
        Empty,
        Invalid { reason: String },
    }

    impl From<&ActiveOutcome> for Outcome {
        fn from(value: &ActiveOutcome) -> Self {
            match value {
                ActiveOutcome::Applied(applied) => Outcome::from(applied),
                ActiveOutcome::Rejected(reject) => Self::Rejected {
                    reason: RejectRecord::from(reject),
                },
                ActiveOutcome::NoEdit(no_edit) => Outcome::from(no_edit),
                ActiveOutcome::Tool(fail) | ActiveOutcome::Validation(fail) => Outcome::from(fail),
                ActiveOutcome::TimedOut(timeout) => Outcome::from(timeout),
            }
        }
    }

    impl From<&Applied> for Outcome {
        fn from(value: &Applied) -> Self {
            let delta = value.delta();
            Self::Applied {
                base_hash: delta.base().hash().as_str().to_string(),
                after_hash: delta.after().hash().as_str().to_string(),
                changed_paths: delta
                    .touches()
                    .iter()
                    .map(|touch| touch.span().path().clone())
                    .collect(),
                validation: value.validation().clone(),
            }
        }
    }

    impl From<&Reject> for RejectRecord {
        fn from(value: &Reject) -> Self {
            match value {
                Reject::Protected { paths } => Self::Protected {
                    paths: paths.clone(),
                },
                Reject::Outside { paths } => Self::Outside {
                    paths: paths.clone(),
                },
                Reject::Empty => Self::Empty,
                Reject::Invalid { reason } => Self::Invalid {
                    reason: reason.clone(),
                },
            }
        }
    }

    impl From<&NoEdit> for Outcome {
        fn from(value: &NoEdit) -> Self {
            Self::NoEdit {
                summary: value.summary().to_string(),
            }
        }
    }

    impl From<&Fail> for Outcome {
        fn from(value: &Fail) -> Self {
            Self::Failed {
                fail_kind: value.kind,
                detail: value.detail().to_string(),
            }
        }
    }

    impl From<&Timeout> for Outcome {
        fn from(value: &Timeout) -> Self {
            Self::TimedOut { secs: value.secs() }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        borrow::Cow,
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::Arc,
    };

    use super::*;

    #[test]
    fn classifier_rejects_absolute_path_outside_workspace() {
        let rejection = classify_paths(
            Path::new("/tmp/prototype1/workspace"),
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            &[PathBuf::from("/tmp/other/crates/ploke-tui/src/lib.rs")],
        )
        .expect("outside path should reject");

        assert!(matches!(rejection, Reject::Outside { .. }));
    }

    #[test]
    fn classifier_rejects_non_normal_relative_path() {
        let rejection = classify_paths(
            Path::new("/tmp/prototype1/workspace"),
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            &[PathBuf::from("crates/../crates/ploke-tui/src/lib.rs")],
        )
        .expect("non-normal path should reject");

        assert!(matches!(rejection, Reject::Outside { .. }));
    }

    #[test]
    fn classifier_uses_broad_policy_for_protected_core() {
        let rejection = classify_paths(
            Path::new("/tmp/prototype1/workspace"),
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            &[PathBuf::from("crates/ploke-eval/src/lib.rs")],
        )
        .expect("protected path should reject");

        assert!(matches!(rejection, Reject::Protected { .. }));
    }

    #[test]
    fn attempt_prompt_preserves_minimal_request_text() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let existing_evidence = tmp.path().join("evaluations");
        let missing_evidence = tmp.path().join("missing-evaluations");
        fs::create_dir_all(&existing_evidence).expect("existing evidence dir");

        let prompt = attempt_prompt(
            Path::new("/tmp/prototype1/workspace"),
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            &[
                EvidenceRoot {
                    kind: EvidenceRootKind::Evaluations,
                    location: EvidenceRootLocation::Directory {
                        path: existing_evidence.clone(),
                    },
                    role: EvidenceRole::EvaluationPayloads,
                },
                EvidenceRoot {
                    kind: EvidenceRootKind::Evaluations,
                    location: EvidenceRootLocation::Directory {
                        path: missing_evidence.clone(),
                    },
                    role: EvidenceRole::EvaluationPayloads,
                },
            ],
            "Modify any part of the codebase at `/tmp/prototype1/workspace`.\n\nPast benchmark scores, failures, and metrics can be found here:\n- `/tmp/prototype1/evaluations`\n",
            Some("tool failed"),
        );

        assert!(prompt.starts_with("Modify any part of the codebase at"));
        assert!(prompt.contains("Past benchmark scores, failures, and metrics"));
        assert!(prompt.contains("Previous attempt result:"));
        assert!(prompt.contains("tool failed"));
        assert!(!prompt.contains("Headless TUI harness boundary"));
        assert!(!prompt.contains("Read-only evidence available to tools"));
        assert!(!prompt.contains("Original broad request"));
        assert!(!prompt.contains("stage one"));
    }

    #[test]
    fn evidence_read_roots_include_request_evidence_but_not_result_output() {
        let roots = evidence_read_roots(&[
            EvidenceRoot {
                kind: EvidenceRootKind::Evaluations,
                location: EvidenceRootLocation::Directory {
                    path: PathBuf::from("/tmp/prototype1/evaluations"),
                },
                role: EvidenceRole::EvaluationPayloads,
            },
            EvidenceRoot {
                kind: EvidenceRootKind::Oracle,
                location: EvidenceRootLocation::File {
                    path: PathBuf::from("/tmp/prototype1/final_report.json"),
                },
                role: EvidenceRole::GuidanceOnly,
            },
            EvidenceRoot {
                kind: EvidenceRootKind::SubmittedResultOutput,
                location: EvidenceRootLocation::File {
                    path: PathBuf::from("/tmp/prototype1/messages/result.json"),
                },
                role: EvidenceRole::OutputBox,
            },
        ]);

        assert_eq!(
            roots,
            vec![
                PathBuf::from("/tmp/prototype1"),
                PathBuf::from("/tmp/prototype1/evaluations")
            ]
        );
    }

    #[test]
    fn retry_feedback_summarizes_tool_json_without_replaying_payload() {
        let feedback = retry_feedback(
            r#"{"user":"read_file: read_file expects a file path, not a directory.","llm":{"ok":false}}"#,
        );

        assert!(feedback.contains("Previous attempt failed"));
        assert!(feedback.contains("read_file expects a file path"));
        assert!(!feedback.contains("stage one"));
        assert!(!feedback.contains("\"llm\""));
    }

    #[test]
    fn retry_feedback_turns_aborted_summary_into_terse_result() {
        let feedback = retry_feedback("Request summary: [aborted] error_id=abc");

        assert!(feedback.contains("Previous attempt aborted before staging an edit"));
        assert!(!feedback.contains("stage one small concrete source edit"));
        assert!(!feedback.contains("error_id=abc"));
    }

    #[test]
    fn policy_repair_prompt_preserves_applied_workspace_state() {
        let prompt =
            policy_repair_prompt("Rejected protected paths: crates/example/Cargo.toml", true);

        assert!(prompt.contains("Previous attempt result"));
        assert!(prompt.contains("Do not edit files under:"));
        assert!(prompt.contains("Cargo.toml"));
        assert!(prompt.contains("The workspace already contains allowed edits"));
        assert!(!prompt.contains("authority/runtime directories"));
        assert!(!prompt.contains("stage only allowed follow-up source edits"));
    }

    #[test]
    fn policy_repair_prompt_handles_no_applied_edits() {
        let prompt =
            policy_repair_prompt("Rejected protected paths: crates/example/Cargo.toml", false);

        assert!(prompt.contains("No allowed source edit has been applied yet"));
        assert!(!prompt.contains("stage a concrete candidate"));
    }

    #[test]
    fn evidence_applied_attempt_carries_changed_paths() {
        let proposal_id = Uuid::from_u128(1);
        let request_id = Uuid::from_u128(2);
        let changed_paths = vec![
            PathBuf::from("crates/ploke-tui/src/app.rs"),
            PathBuf::from("crates/ploke-tui/src/lib.rs"),
        ];
        let run = HeadlessRun {
            attempts: vec![HeadlessAttempt {
                turn: 1,
                proposal_id: Some(proposal_id),
                result: HeadlessAttemptResult::Applied {
                    paths: changed_paths.clone(),
                },
            }],
            events: Vec::new(),
            debug_relay: DebugRelay::new(),
            prompt_diagnostics: Vec::new(),
            terminal: Some(HeadlessTerminal::Applied {
                proposal_id,
                request_id,
                changed_paths: changed_paths.clone(),
            }),
        };

        let summary = run.evidence();
        let proposal_id = proposal_id.to_string();
        let request_id = request_id.to_string();

        assert_eq!(summary.attempts.len(), 1);
        assert_eq!(summary.attempts[0].turn, 1);
        assert_eq!(
            summary.attempts[0].proposal_id.as_deref(),
            Some(proposal_id.as_str())
        );
        assert!(matches!(
            &summary.attempts[0].result,
            evidence::Result::Applied { paths } if paths == &changed_paths
        ));
        assert!(matches!(
            summary.terminal.as_ref(),
            Some(evidence::Terminal::Applied {
                proposal_id: observed_proposal,
                request_id: observed_request,
                changed_paths: observed_paths,
            }) if observed_proposal == &proposal_id
                && observed_request == &request_id
                && observed_paths == &changed_paths
        ));
    }

    #[test]
    fn evidence_rejected_attempt_carries_feedback() {
        let proposal_id = Uuid::from_u128(3);
        let feedback = "Rejected protected paths: crates/ploke-eval/src/lib.rs".to_string();
        let run = HeadlessRun {
            attempts: vec![HeadlessAttempt {
                turn: 2,
                proposal_id: Some(proposal_id),
                result: HeadlessAttemptResult::Rejected {
                    reason: feedback.clone(),
                },
            }],
            events: Vec::new(),
            debug_relay: DebugRelay::new(),
            prompt_diagnostics: Vec::new(),
            terminal: Some(HeadlessTerminal::Exhausted {
                attempts: 2,
                last: feedback.clone(),
            }),
        };

        let summary = run.evidence();

        assert_eq!(summary.attempts.len(), 1);
        assert_eq!(summary.attempts[0].turn, 2);
        assert!(matches!(
            &summary.attempts[0].result,
            evidence::Result::Rejected { feedback: observed } if observed == &feedback
        ));
        assert!(matches!(
            summary.terminal.as_ref(),
            Some(evidence::Terminal::Exhausted {
                attempts: 2,
                last_feedback,
            }) if last_feedback == &feedback
        ));
    }

    #[test]
    fn evidence_retains_bounded_debug_relay() {
        let mut run = HeadlessRun::new();
        run.debug_relay.push("first relay command");
        run.debug_relay
            .push(&"x".repeat(MAX_DEBUG_RELAY_EVENT_CHARS + 3));
        for index in 0..MAX_DEBUG_RELAY_EVENTS {
            run.debug_relay.push(&format!("relay command {index}"));
        }

        let summary = run.evidence();

        assert_eq!(summary.debug_relay.retained.len(), MAX_DEBUG_RELAY_EVENTS);
        assert_eq!(summary.debug_relay.dropped, 2);
        assert_eq!(summary.debug_relay.truncated, 1);
        let expected_last = format!("relay command {}", MAX_DEBUG_RELAY_EVENTS - 1);
        assert_eq!(
            summary.debug_relay.retained.last().map(String::as_str),
            Some(expected_last.as_str())
        );
        assert!(
            summary
                .debug_relay
                .retained
                .iter()
                .all(|message| message.chars().count() <= MAX_DEBUG_RELAY_EVENT_CHARS)
        );
    }

    #[test]
    fn evidence_carries_bounded_tool_event_stream() {
        let mut run = HeadlessRun::new();
        let long_args = format!(
            r#"{{"file_path":"src/lib.rs","payload":"{}"}}"#,
            "x".repeat(MAX_EVIDENCE_EVENT_CHARS + 10)
        );
        run.events.push(Event::ToolRequest {
            request_id: "request-1".to_string(),
            parent_id: "parent-1".to_string(),
            call_id: "call-1".to_string(),
            tool: "apply_code_edit".to_string(),
            arguments: long_args,
        });
        run.events.push(Event::Tool {
            call_id: "call-1".to_string(),
            result: Tool::Failed {
                error: "tool rejected invalid path".to_string(),
            },
        });

        let summary = run.evidence();

        assert_eq!(summary.events.len(), 2);
        assert!(matches!(
            &summary.events[0],
            evidence::Event::ToolRequest {
                tool,
                arguments,
                ..
            } if tool == "apply_code_edit"
                && arguments.chars > MAX_EVIDENCE_EVENT_CHARS
                && arguments.preview.chars().count() <= MAX_EVIDENCE_EVENT_CHARS + 3
        ));
        assert!(matches!(
            &summary.events[1],
            evidence::Event::ToolFailed { error, .. }
                if error.preview.contains("tool rejected invalid path")
        ));
        serde_json::to_string_pretty(&summary).expect("event diagnostics serialize");
    }

    #[test]
    fn evidence_carries_prompt_context_diagnostics() {
        let mut run = HeadlessRun::new();
        run.prompt_diagnostics.push(PromptDiagnostic {
            parent_id: Uuid::from_u128(7).to_string(),
            workspace: WorkspaceDiagnostic {
                loaded: true,
                root: Some(PathBuf::from("/tmp/candidate")),
                member_count: 3,
                focused_root: Some(PathBuf::from("/tmp/candidate/crates/ploke-eval")),
            },
            bm25: Some(Bm25Diagnostic {
                status: "ready".to_string(),
                docs: Some(42),
                error: None,
            }),
            context_mode: "Light".to_string(),
            max_leased_tokens: 2400,
            estimated_total_tokens: 900,
            message_count: 2,
            message_previews: vec![MessagePreview {
                role: "System".to_string(),
                chars: 11,
                preview: "RAG context".to_string(),
            }],
            included_rag_parts: 1,
            rag_part_previews: vec![RagPartPreview {
                file_path: "src/lib.rs".to_string(),
                kind: "Code".to_string(),
                estimated_tokens: 100,
                score: 0.75,
            }],
            rag_stats: Some(ContextStatsDiagnostic {
                total_tokens: 100,
                files: 1,
                parts: 1,
                truncated_parts: 0,
                dedup_removed: 0,
            }),
            fallback_notice: None,
        });

        let summary = run.evidence();

        assert_eq!(summary.prompt_diagnostics.len(), 1);
        let diagnostic = &summary.prompt_diagnostics[0];
        assert!(diagnostic.workspace.loaded);
        assert_eq!(diagnostic.workspace.member_count, 3);
        assert!(matches!(
            diagnostic.bm25.as_ref(),
            Some(evidence::Bm25 { status, docs: Some(42), .. }) if status == "ready"
        ));
        assert_eq!(diagnostic.included_rag_parts, 1);
        serde_json::to_string_pretty(&summary).expect("prompt diagnostics serialize");
    }

    #[test]
    fn fallback_prompt_becomes_context_unavailable_terminal() {
        let fallback = "No workspace context loaded; proceeding without code context. Index or load a workspace to enable RAG.";
        let diagnostic = PromptDiagnostic {
            parent_id: Uuid::from_u128(8).to_string(),
            workspace: WorkspaceDiagnostic {
                loaded: false,
                root: None,
                member_count: 0,
                focused_root: None,
            },
            bm25: None,
            context_mode: "Light".to_string(),
            max_leased_tokens: 2400,
            estimated_total_tokens: 26,
            message_count: 1,
            message_previews: vec![MessagePreview {
                role: "System".to_string(),
                chars: fallback.chars().count(),
                preview: fallback.to_string(),
            }],
            included_rag_parts: 0,
            rag_part_previews: Vec::new(),
            rag_stats: None,
            fallback_notice: Some(fallback.to_string()),
        };

        let reason = diagnostic
            .context_unavailable_reason()
            .expect("fallback should be hard failure");
        let run = HeadlessRun {
            attempts: Vec::new(),
            events: Vec::new(),
            debug_relay: DebugRelay::new(),
            prompt_diagnostics: vec![diagnostic],
            terminal: Some(HeadlessTerminal::ContextUnavailable {
                reason: reason.clone(),
            }),
        };

        let summary = run.evidence();

        assert!(matches!(
            summary.terminal.as_ref(),
            Some(evidence::Terminal::ContextUnavailable { reason: observed }) if observed == &reason
        ));
        assert_eq!(
            summary.prompt_diagnostics[0].fallback_notice.as_deref(),
            Some(fallback)
        );
    }

    #[test]
    fn off_context_prompt_is_not_context_unavailable() {
        let fallback = "Context mode is Off; workspace loaded at /tmp/candidate. Proceeding without code context.";
        let diagnostic = PromptDiagnostic {
            parent_id: Uuid::from_u128(9).to_string(),
            workspace: WorkspaceDiagnostic {
                loaded: true,
                root: Some(PathBuf::from("/tmp/candidate")),
                member_count: 1,
                focused_root: Some(PathBuf::from("/tmp/candidate")),
            },
            bm25: Some(Bm25Diagnostic {
                status: "ready".to_string(),
                docs: Some(12),
                error: None,
            }),
            context_mode: "Off".to_string(),
            max_leased_tokens: 2400,
            estimated_total_tokens: 20,
            message_count: 2,
            message_previews: vec![MessagePreview {
                role: "System".to_string(),
                chars: fallback.chars().count(),
                preview: fallback.to_string(),
            }],
            included_rag_parts: 0,
            rag_part_previews: Vec::new(),
            rag_stats: None,
            fallback_notice: Some(fallback.to_string()),
        };

        assert!(
            diagnostic.context_unavailable_reason().is_none(),
            "broad harness intentionally disables automatic prompt context"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "live provider test for the ploke-eval/ploke-tui broad edit surface"]
    async fn live_tui_adapter_canary_shows_inputs_outputs_and_applied_edit() {
        let prompt = r#"Use the available edit tools to stage exactly one code edit in src/lib.rs.

Change broad_surface_canary so it returns "after" instead of "before".
Do not edit Cargo.toml. Do not create report, result, control, or bookkeeping files.
"#;
        let fixture =
            prepare_live_canary("live-tui-adapter-direct-file", prompt).expect("prepare canary");

        let run = run_live_canary(&fixture).await;
        let final_lib = write_live_canary_artifacts(&fixture, &run);
        assert_live_canary_applied(&fixture, &run, &final_lib);
        assert_workspace_index_ready(&fixture, &run);
        assert_auto_context_off(&fixture, &run);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "live provider test for indexed ploke-tui retrieval plus broad edit application"]
    async fn live_tui_adapter_canary_uses_indexed_context_before_applied_edit() {
        let prompt = r#"Use request_code_context to locate the Rust function named broad_surface_canary, then use the available edit tools to stage exactly one code edit.

Change broad_surface_canary so it returns "after" instead of "before".
Do not edit Cargo.toml. Do not create report, result, control, or bookkeeping files.
"#;
        let fixture = prepare_live_canary("live-tui-adapter-indexed-context", prompt)
            .expect("prepare canary");

        let run = run_live_canary(&fixture).await;
        let final_lib = write_live_canary_artifacts(&fixture, &run);
        let context_request = tool_request_position(&run, "request_code_context");
        let edit_request = tool_request_position(&run, "apply_code_edit");
        assert!(
            matches!((context_request, edit_request), (Some(context), Some(edit)) if context < edit),
            "expected request_code_context before edit; artifacts at {}",
            fixture.artifact_root.display()
        );
        assert_live_canary_applied(&fixture, &run, &final_lib);
        assert_workspace_index_ready(&fixture, &run);
        assert_auto_context_off(&fixture, &run);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "runtime setup canary for child-local parse/transform DB plus BM25 readiness"]
    async fn live_tui_runtime_setup_uses_sparse_child_db() {
        let fixture = prepare_live_canary(
            "live-tui-adapter-sparse-child-db",
            "runtime setup only; no LLM prompt is submitted",
        )
        .expect("prepare canary");

        let mut runtime = crate::runner::setup_workspace_tui_runtime(&fixture.workspace)
            .await
            .unwrap_or_else(|err| {
                fs::write(
                    fixture.artifact_root.join("setup-error.txt"),
                    err.to_string(),
                )
                .expect("write setup error");
                panic!(
                    "runtime setup failed; artifacts at {}",
                    fixture.artifact_root.display()
                );
            });
        runtime.app.pump_pending_events().await;

        let cfg = runtime.state.config.read().await;
        assert!(
            matches!(
                cfg.rag.strategy,
                ploke_tui::user_config::RetrievalStrategyUser::Sparse { strict: true }
            ),
            "expected sparse-strict retrieval; artifacts at {}",
            fixture.artifact_root.display()
        );
        assert!(cfg.rag.strict_bm25_by_default);
        drop(cfg);

        let Some(rag) = runtime.state.rag.as_ref() else {
            panic!(
                "expected RAG service; artifacts at {}",
                fixture.artifact_root.display()
            );
        };
        let status = rag.bm25_status().await.expect("bm25 status");
        assert!(
            matches!(status, ploke_db::bm25_index::bm25_service::Bm25Status::Ready { docs } if docs > 0),
            "expected BM25 ready with documents, got {status:?}; artifacts at {}",
            fixture.artifact_root.display()
        );

        assert!(
            runtime.state.indexing_state.read().await.is_none(),
            "expected no dense indexing status from /index; artifacts at {}",
            fixture.artifact_root.display()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "operator canary for an existing broad harness workspace"]
    async fn live_tui_runtime_setup_existing_workspace_from_env() {
        let Some(workspace) =
            std::env::var_os("PLOKE_EVAL_EXISTING_TUI_WORKSPACE").map(PathBuf::from)
        else {
            println!("skipping: set PLOKE_EVAL_EXISTING_TUI_WORKSPACE to a workspace root");
            return;
        };

        let mut runtime = crate::runner::setup_workspace_tui_prompt_runtime(&workspace)
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "runtime setup failed for existing workspace '{}': {err}",
                    workspace.display()
                );
            });
        runtime.app.pump_pending_events().await;

        let cfg = runtime.state.config.read().await;
        assert!(
            matches!(
                cfg.rag.strategy,
                ploke_tui::user_config::RetrievalStrategyUser::Sparse { strict: true }
            ),
            "expected sparse-strict retrieval for '{}'",
            workspace.display()
        );
        drop(cfg);

        let Some(rag) = runtime.state.rag.as_ref() else {
            panic!("expected RAG service for '{}'", workspace.display());
        };
        let status = rag
            .bm25_status_with_timeout(Duration::from_secs(5))
            .await
            .expect("bm25 status");
        assert!(
            matches!(status, ploke_db::bm25_index::bm25_service::Bm25Status::Ready { docs } if docs > 0),
            "expected BM25 ready with documents for '{}', got {status:?}",
            workspace.display()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "operator canary for initial prompt RAG against the ploke workspace"]
    async fn live_tui_initial_prompt_ploke_workspace_includes_rag_parts() {
        let workspace = std::env::var_os("PLOKE_EVAL_EXISTING_TUI_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(ploke_workspace_root_for_test);
        let mut runtime = crate::runner::setup_workspace_tui_prompt_runtime(&workspace)
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "runtime setup failed for initial prompt RAG canary '{}': {err}",
                    workspace.display()
                );
            });
        runtime.app.pump_pending_events().await;

        let prompt = r#"Inspect the indexed workspace context for setup_workspace_tui_runtime and run_broad_headless_tui_attempt.
Do not call tools. Do not propose edits. This canary only checks initial prompt context assembly."#;
        let parent_id = submit_prompt(&runtime.app, prompt.to_string())
            .await
            .expect("submit canary prompt");

        let diagnostic = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                runtime.app.pump_pending_events().await;
                match next_event(&mut runtime).await.expect("next app event") {
                    ploke_tui::AppEvent::Llm(ploke_tui::llm::LlmEvent::ChatCompletion(
                        ploke_tui::llm::ChatEvt::PromptConstructed {
                            parent_id: observed,
                            formatted_prompt,
                            context_plan,
                        },
                    )) if observed == parent_id => {
                        break PromptDiagnostic::capture(
                            &runtime.state,
                            observed,
                            &formatted_prompt,
                            &context_plan,
                        )
                        .await;
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "timed out waiting for initial PromptConstructed event in '{}'",
                workspace.display()
            )
        });

        println!(
            "initial prompt RAG workspace={} bm25={:?} included_rag_parts={} fallback={:?}",
            workspace.display(),
            diagnostic.bm25,
            diagnostic.included_rag_parts,
            diagnostic.fallback_notice
        );
        assert!(
            diagnostic.workspace.loaded,
            "expected loaded workspace for '{}'",
            workspace.display()
        );
        assert!(
            matches!(diagnostic.bm25.as_ref(), Some(Bm25Diagnostic { status, docs: Some(docs), .. }) if status == "ready" && *docs > 0),
            "expected ready BM25 with documents for '{}', got {:?}",
            workspace.display(),
            diagnostic.bm25
        );
        assert!(
            diagnostic.fallback_notice.is_none(),
            "initial prompt fell back without code context for '{}': {:?}",
            workspace.display(),
            diagnostic.fallback_notice
        );
        assert!(
            diagnostic.included_rag_parts > 0,
            "initial prompt included no RAG parts for '{}': {:?}",
            workspace.display(),
            diagnostic.rag_stats
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "operator canary for ploke workspace prompt construction with context mode Off"]
    async fn live_tui_initial_prompt_off_skips_rag_parts() {
        let workspace = std::env::var_os("PLOKE_EVAL_EXISTING_TUI_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(ploke_workspace_root_for_test);
        let mut runtime = crate::runner::setup_workspace_tui_prompt_runtime(&workspace)
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "runtime setup failed for initial prompt Off canary '{}': {err}",
                    workspace.display()
                );
            });
        {
            let mut cfg = runtime.state.config.write().await;
            cfg.context_management.mode = ploke_tui::user_config::CtxMode::Off;
        }
        runtime.app.pump_pending_events().await;

        let prompt = r#"Inspect the indexed workspace context for setup_workspace_tui_runtime.
Do not call tools. Do not propose edits. This canary only checks context mode Off prompt assembly."#;
        let parent_id = submit_prompt(&runtime.app, prompt.to_string())
            .await
            .expect("submit canary prompt");

        let diagnostic = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                runtime.app.pump_pending_events().await;
                match next_event(&mut runtime).await.expect("next app event") {
                    ploke_tui::AppEvent::Llm(ploke_tui::llm::LlmEvent::ChatCompletion(
                        ploke_tui::llm::ChatEvt::PromptConstructed {
                            parent_id: observed,
                            formatted_prompt,
                            context_plan,
                        },
                    )) if observed == parent_id => {
                        break PromptDiagnostic::capture(
                            &runtime.state,
                            observed,
                            &formatted_prompt,
                            &context_plan,
                        )
                        .await;
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "timed out waiting for initial PromptConstructed event in '{}'",
                workspace.display()
            )
        });

        println!(
            "initial prompt Off workspace={} bm25={:?} included_rag_parts={} fallback={:?}",
            workspace.display(),
            diagnostic.bm25,
            diagnostic.included_rag_parts,
            diagnostic.fallback_notice
        );
        assert!(
            diagnostic.workspace.loaded,
            "expected loaded workspace for '{}'",
            workspace.display()
        );
        assert!(
            matches!(diagnostic.bm25.as_ref(), Some(Bm25Diagnostic { status, docs: Some(docs), .. }) if status == "ready" && *docs > 0),
            "expected ready BM25 with documents for '{}', got {:?}",
            workspace.display(),
            diagnostic.bm25
        );
        assert_eq!(diagnostic.context_mode, "Off");
        assert_eq!(diagnostic.included_rag_parts, 0);
        assert!(diagnostic.rag_part_previews.is_empty());
        assert!(diagnostic.rag_stats.is_none());
        assert!(
            matches!(
                diagnostic.fallback_notice.as_deref(),
                Some(notice)
                    if notice.starts_with("Context mode is Off; workspace loaded at ")
                        && notice.contains("Proceeding without code context.")
            ),
            "expected Off fallback notice, got {:?}",
            diagnostic.fallback_notice
        );
        assert!(
            diagnostic.context_unavailable_reason().is_none(),
            "Off mode should not terminate a broad harness attempt"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "operator canary for request_code_context against the ploke workspace"]
    async fn live_tui_request_code_context_ploke_workspace_returns_results() {
        let workspace = std::env::var_os("PLOKE_EVAL_EXISTING_TUI_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(ploke_workspace_root_for_test);
        let terms = std::env::var("PLOKE_EVAL_TUI_SEARCH_TERMS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|term| !term.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|terms| !terms.is_empty())
            .unwrap_or_else(|| {
                vec![
                    "setup_workspace_tui_runtime".to_string(),
                    "RequestCodeContextGat".to_string(),
                    "run_broad_headless_tui_attempt".to_string(),
                ]
            });

        let mut runtime = crate::runner::setup_workspace_tui_runtime(&workspace)
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "runtime setup failed for request_code_context canary '{}': {err}",
                    workspace.display()
                );
            });
        runtime.app.pump_pending_events().await;

        let ctx = ploke_tui::tools::Ctx {
            state: Arc::clone(&runtime.state),
            event_bus: Arc::new(ploke_tui::EventBus::new(ploke_tui::EventBusCaps::default())),
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            call_id: ploke_core::ArcStr::from("ploke-workspace-context-canary"),
        };
        let rag = runtime
            .state
            .rag
            .as_ref()
            .expect("RAG service must be configured")
            .clone();

        let mut misses = Vec::new();
        for term in terms {
            let raw_hits = rag
                .search_bm25_strict(&term, 6, ploke_core::RetrievalScope::LoadedWorkspace)
                .await
                .unwrap_or_else(|err| {
                    panic!(
                        "raw BM25 search failed for term '{term}' in '{}': {err}",
                        workspace.display()
                    )
                });
            let raw_ids = raw_hits.iter().map(|(id, _score)| *id).collect::<Vec<_>>();
            let raw_nodes = runtime
                .state
                .db
                .get_snippet_nodes_ordered(raw_ids)
                .unwrap_or_else(|err| panic!("failed to resolve raw BM25 nodes: {err}"));
            let snippet_checks = runtime
                .state
                .io_handle
                .get_snippets_batch(raw_nodes.clone())
                .await
                .unwrap_or_else(|err| panic!("snippet batch request failed: {err}"));
            let snippet_ok = snippet_checks.iter().filter(|res| res.is_ok()).count();
            let result = <ploke_tui::tools::request_code_context::RequestCodeContextGat as ploke_tui::tools::Tool>::execute(
                ploke_tui::tools::request_code_context::RequestCodeContextParams {
                    token_budget: Some(1_200),
                    search_term: Some(Cow::Owned(term.clone())),
                },
                ctx.clone(),
            )
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "request_code_context failed for term '{term}' in '{}': {err}",
                    workspace.display()
                );
            });

            let payload: ploke_core::rag_types::RequestCodeContextResult =
                serde_json::from_str(&result.content).unwrap_or_else(|err| {
                    panic!("request_code_context returned invalid JSON for term '{term}': {err}")
                });
            println!(
                "request_code_context term={term:?} raw_bm25_hits={} snippet_ok={} first_node_path={:?} top_k={} returned={} first_path={:?}",
                raw_hits.len(),
                snippet_ok,
                raw_nodes
                    .first()
                    .map(|node| node.file_path.to_string_lossy().into_owned()),
                payload.top_k,
                payload.context.len(),
                payload
                    .context
                    .first()
                    .map(|context| context.file_path.0.as_str())
            );
            if payload.context.is_empty() {
                misses.push(format!("{term}: {:?}", payload.note));
            }
        }
        assert!(
            misses.is_empty(),
            "expected request_code_context to return snippets for all terms in '{}'; misses: {}",
            workspace.display(),
            misses.join("; ")
        );
    }

    fn ploke_workspace_root_for_test() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("ploke-eval crate lives under <workspace>/crates/ploke-eval")
            .to_path_buf()
    }

    fn assert_live_canary_applied(fixture: &LiveCanaryFixture, run: &HeadlessRun, final_lib: &str) {
        assert!(
            matches!(run.terminal(), Some(HeadlessTerminal::Applied { .. })),
            "expected applied terminal; artifacts at {}",
            fixture.artifact_root.display()
        );
        assert!(
            final_lib.contains("\"after\"") && !final_lib.contains("\"before\""),
            "expected sentinel change in src/lib.rs; artifacts at {}",
            fixture.artifact_root.display()
        );
        assert!(
            run.events().iter().any(|event| matches!(
                event,
                Event::Proposal { paths, .. }
                    if paths.iter().any(|path| path.ends_with("src/lib.rs"))
            )),
            "expected proposal evidence for src/lib.rs; artifacts at {}",
            fixture.artifact_root.display()
        );
    }

    fn assert_workspace_index_ready(fixture: &LiveCanaryFixture, run: &HeadlessRun) {
        let diagnostic = run.prompt_diagnostics().first().unwrap_or_else(|| {
            panic!(
                "expected prompt diagnostics; artifacts at {}",
                fixture.artifact_root.display()
            )
        });
        assert!(
            diagnostic.workspace.loaded,
            "expected loaded workspace in prompt diagnostics; artifacts at {}",
            fixture.artifact_root.display()
        );
        assert!(
            diagnostic.workspace.member_count > 0,
            "expected loaded workspace members in prompt diagnostics; artifacts at {}",
            fixture.artifact_root.display()
        );
        assert!(
            matches!(diagnostic.bm25.as_ref(), Some(Bm25Diagnostic { status, docs: Some(docs), .. }) if status == "ready" && *docs > 0),
            "expected ready BM25 in prompt diagnostics, got {:?}; artifacts at {}",
            diagnostic.bm25,
            fixture.artifact_root.display()
        );
    }

    fn assert_auto_context_off(fixture: &LiveCanaryFixture, run: &HeadlessRun) {
        let diagnostic = run.prompt_diagnostics().first().unwrap_or_else(|| {
            panic!(
                "expected prompt diagnostics; artifacts at {}",
                fixture.artifact_root.display()
            )
        });
        assert!(
            diagnostic.context_mode == "Off",
            "expected automatic prompt context off, got {}; artifacts at {}",
            diagnostic.context_mode,
            fixture.artifact_root.display()
        );
        assert_eq!(
            diagnostic.included_rag_parts,
            0,
            "expected no automatic prompt RAG parts; artifacts at {}",
            fixture.artifact_root.display()
        );
        assert!(
            matches!(
                diagnostic.fallback_notice.as_deref(),
                Some(notice)
                    if notice.starts_with("Context mode is Off; workspace loaded at ")
                        && notice.contains("Proceeding without code context.")
            ),
            "expected intentional Off fallback notice, got {:?}; artifacts at {}",
            diagnostic.fallback_notice,
            fixture.artifact_root.display()
        );
    }

    async fn run_live_canary(fixture: &LiveCanaryFixture) -> HeadlessRun {
        let budget = Budget::new(2, 600).expect("valid live canary budget");
        match run_headless(
            &fixture.workspace,
            &fixture.prompt,
            budget,
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            &[],
        )
        .await
        {
            Ok(run) => run,
            Err(err) => {
                fs::write(fixture.artifact_root.join("run-error.txt"), err.to_string())
                    .expect("write run error");
                panic!(
                    "live TUI adapter canary failed before returning HeadlessRun; artifacts at {}",
                    fixture.artifact_root.display()
                );
            }
        }
    }

    fn write_live_canary_artifacts(fixture: &LiveCanaryFixture, run: &HeadlessRun) -> String {
        let final_lib = fs::read_to_string(&fixture.src_file).expect("read final lib");
        fs::write(&fixture.final_file, &final_lib).expect("write final artifact");
        fs::write(
            &fixture.evidence_path,
            serde_json::to_string_pretty(&run.evidence()).expect("serialize evidence"),
        )
        .expect("write evidence");
        fs::write(
            &fixture.events_path,
            serde_json::to_string_pretty(run.events()).expect("serialize events"),
        )
        .expect("write events");
        let diff = command_output(&fixture.workspace, "git", &["diff", "--", "src/lib.rs"]);
        fs::write(&fixture.diff_path, &diff).expect("write diff");
        fs::write(
            &fixture.report_path,
            serde_json::to_string_pretty(&LiveCanaryReport {
                artifact_root: fixture.artifact_root.clone(),
                workspace: fixture.workspace.clone(),
                prompt_path: fixture.prompt_path.clone(),
                initial_file: fixture.initial_file.clone(),
                final_file: fixture.final_file.clone(),
                evidence_path: fixture.evidence_path.clone(),
                events_path: fixture.events_path.clone(),
                diff_path: fixture.diff_path.clone(),
                terminal: run.evidence().terminal,
                requested_tools: requested_tools(run),
                final_contains_after: final_lib.contains("\"after\""),
                final_contains_before: final_lib.contains("\"before\""),
            })
            .expect("serialize report"),
        )
        .expect("write report");
        print_live_canary_summary(fixture, run, &diff);
        final_lib
    }

    fn prepare_live_canary(name: &str, prompt: &str) -> std::io::Result<LiveCanaryFixture> {
        let base = std::env::var_os("PLOKE_EVAL_LIVE_TUI_CANARY_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "ploke-eval-live-tui-canary-{}",
                    Uuid::new_v4().simple()
                ))
            });
        let artifact_root = base.join(name);
        fs::create_dir_all(&artifact_root)?;
        println!(
            "live TUI adapter canary artifacts: {}",
            artifact_root.display()
        );

        let workspace = artifact_root.join("workspace");
        let src_dir = workspace.join("src");
        fs::create_dir_all(&src_dir)?;

        let cargo_toml = r#"[package]
name = "ploke-eval-live-tui-canary"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#;
        let initial_lib = r#"pub fn broad_surface_canary() -> &'static str {
    "before"
}
"#;
        let src_file = src_dir.join("lib.rs");
        let prompt_path = artifact_root.join("prompt.txt");
        let initial_file = artifact_root.join("initial-src-lib.rs");
        let final_file = artifact_root.join("final-src-lib.rs");
        let evidence_path = artifact_root.join("headless-evidence.json");
        let events_path = artifact_root.join("headless-events.json");
        let diff_path = artifact_root.join("workspace.diff");
        let report_path = artifact_root.join("report.json");

        fs::write(workspace.join("Cargo.toml"), cargo_toml)?;
        fs::write(&src_file, initial_lib)?;
        fs::write(&prompt_path, prompt)?;
        fs::write(&initial_file, initial_lib)?;

        command_output(&workspace, "git", &["init"]);
        command_output(&workspace, "git", &["add", "Cargo.toml", "src/lib.rs"]);
        command_output(
            &workspace,
            "git",
            &[
                "-c",
                "user.email=ploke-eval-live-canary@example.invalid",
                "-c",
                "user.name=ploke eval live canary",
                "commit",
                "--allow-empty",
                "-m",
                "initial canary",
            ],
        );

        Ok(LiveCanaryFixture {
            artifact_root,
            workspace,
            src_file,
            prompt: prompt.to_string(),
            prompt_path,
            initial_file,
            final_file,
            evidence_path,
            events_path,
            diff_path,
            report_path,
        })
    }

    fn tool_request_position(run: &HeadlessRun, tool: &str) -> Option<usize> {
        run.events().iter().position(
            |event| matches!(event, Event::ToolRequest { tool: observed, .. } if observed == tool),
        )
    }

    fn requested_tools(run: &HeadlessRun) -> Vec<String> {
        run.events()
            .iter()
            .filter_map(|event| match event {
                Event::ToolRequest { tool, .. } => Some(tool.clone()),
                _ => None,
            })
            .collect()
    }

    fn print_live_canary_summary(fixture: &LiveCanaryFixture, run: &HeadlessRun, diff: &str) {
        println!(
            "live TUI adapter canary report: {}",
            fixture.report_path.display()
        );
        println!("terminal: {:?}", run.terminal());
        println!("requested tools: {}", requested_tools(run).join(", "));
        for event in run.events() {
            if let Event::ToolRequest {
                tool,
                call_id,
                arguments,
                ..
            } = event
            {
                println!("tool request {tool} {call_id}: {arguments}");
            }
        }
        println!("workspace diff:\n{diff}");
    }

    #[derive(Debug)]
    struct LiveCanaryFixture {
        artifact_root: PathBuf,
        workspace: PathBuf,
        src_file: PathBuf,
        prompt: String,
        prompt_path: PathBuf,
        initial_file: PathBuf,
        final_file: PathBuf,
        evidence_path: PathBuf,
        events_path: PathBuf,
        diff_path: PathBuf,
        report_path: PathBuf,
    }

    #[derive(Debug, Serialize)]
    struct LiveCanaryReport {
        artifact_root: PathBuf,
        workspace: PathBuf,
        prompt_path: PathBuf,
        initial_file: PathBuf,
        final_file: PathBuf,
        evidence_path: PathBuf,
        events_path: PathBuf,
        diff_path: PathBuf,
        terminal: Option<evidence::Terminal>,
        requested_tools: Vec<String>,
        final_contains_after: bool,
        final_contains_before: bool,
    }

    fn command_output(cwd: &Path, program: &str, args: &[&str]) -> String {
        let output = Command::new(program)
            .current_dir(cwd)
            .args(args)
            .output()
            .unwrap_or_else(|err| panic!("failed to run {program} {args:?}: {err}"));
        let mut rendered = String::new();
        rendered.push_str(&String::from_utf8_lossy(&output.stdout));
        rendered.push_str(&String::from_utf8_lossy(&output.stderr));
        if !output.status.success() {
            panic!(
                "{program} {args:?} failed with status {:?}: {rendered}",
                output.status.code()
            );
        }
        rendered
    }
}

impl From<&record::Outcome> for Outcome {
    fn from(value: &record::Outcome) -> Self {
        match value {
            record::Outcome::Applied { .. } => Self::Rejected(Reject::Invalid {
                reason: "durable applied record cannot reconstruct active artifact delta"
                    .to_string(),
            }),
            record::Outcome::Rejected { reason } => Self::Rejected(Reject::from(reason)),
            record::Outcome::NoEdit { summary } => Self::NoEdit(NoEdit::new(summary.clone())),
            record::Outcome::Failed { fail_kind, detail } => match fail_kind {
                FailKind::Tool => Self::Tool(Fail::tool(detail.clone())),
                FailKind::Validation => Self::Validation(Fail::validation(detail.clone())),
            },
            record::Outcome::TimedOut { secs } => Self::TimedOut(Timeout::new(*secs)),
        }
    }
}

impl From<&record::RejectRecord> for Reject {
    fn from(value: &record::RejectRecord) -> Self {
        match value {
            record::RejectRecord::Protected { paths } => Self::Protected {
                paths: paths.clone(),
            },
            record::RejectRecord::Outside { paths } => Self::Outside {
                paths: paths.clone(),
            },
            record::RejectRecord::Empty => Self::Empty,
            record::RejectRecord::Invalid { reason } => Self::Invalid {
                reason: reason.clone(),
            },
        }
    }
}
