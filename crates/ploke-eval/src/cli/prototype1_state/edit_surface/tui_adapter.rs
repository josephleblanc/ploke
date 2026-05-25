//! Eval-owned carrier boundary for a headless `ploke-tui` edit attempt.
//!
//! This module does not run chat sessions, route model calls, or duplicate the
//! `ploke-tui` app harness. It names the small amount of structure that
//! `ploke-eval` needs around the vanilla headless TUI path: an admitted request,
//! one or more attempts, observed event summaries, retry policy, and terminal
//! outcomes. `ploke-tui` remains the executor; `ploke-eval` owns the bounded
//! grant, surface check, and durable projection of what happened.

use std::{
    collections::{HashMap, VecDeque},
    marker::PhantomData,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use ploke_llm::{
    ModelId, ProviderKey,
    router_only::{RouterVariants, google::Google, openrouter::OpenRouter},
};
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
const POST_APPLY_STATUS_TIMEOUT_SECS: u64 = 120;
const POST_APPLY_INDEX_TIMEOUT_SECS: u64 = 180;
const POST_APPLY_INDEX_START_GRACE_MS: u64 = 2_000;

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
    router: RouterVariants,
}

impl ModelSelection {
    pub(crate) fn openrouter(model_id: ModelId, provider: Option<ProviderKey>) -> Self {
        Self {
            model_id,
            provider,
            router: RouterVariants::OpenRouter(OpenRouter),
        }
    }

    pub(crate) fn direct_google(model_id: ModelId) -> Self {
        Self {
            model_id,
            provider: None,
            router: RouterVariants::Google(Google),
        }
    }

    pub(crate) fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    pub(crate) fn provider(&self) -> Option<&ProviderKey> {
        self.provider.as_ref()
    }

    pub(crate) fn router(&self) -> RouterVariants {
        self.router
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
                edit_policy,
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
        Ok(Ok(terminal)) => terminal,
        Ok(Err(source)) => {
            if !run.has_observed_activity() {
                return Err(source);
            }
            HeadlessTerminal::ToolFailed {
                error: observed_headless_error(source),
            }
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum StagedItem {
    Edit(Uuid),
    Create(Uuid),
}

impl StagedItem {
    fn id(self) -> Uuid {
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
struct ToolBatch {
    expected: Vec<ploke_core::ArcStr>,
    completed: Vec<ploke_core::ArcStr>,
    staged: Vec<StagedItem>,
}

impl ToolBatch {
    fn request(&mut self, call_id: ploke_core::ArcStr) {
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
struct Candidate {
    item: StagedItem,
    proposed_at_ms: i64,
    paths: Vec<PathBuf>,
}

#[derive(Debug, Default)]
struct BatchOutcome {
    applied: Vec<AppliedItem>,
    changed_paths: Vec<PathBuf>,
    feedbacks: Vec<String>,
    retry: Option<String>,
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
    use ploke_tui::{AppEvent, app_state::events::SystemEvent};

    let mut pending_retry = None::<String>;
    let mut provider_failure = None::<String>;
    let mut applied = Vec::<AppliedItem>::new();
    let mut changed_paths = Vec::<PathBuf>::new();
    let mut policy_feedbacks = Vec::<String>::new();
    let mut policy_repair_turns = 0_u32;
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
                    batches.clear();
                    pending_events.clear();
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

                if let Some(feedback) = latest_failed_cargo_validation_feedback(run) {
                    observer.emit(format!(
                        "attempt {turn} validation_failed_after_apply {}",
                        truncate_chars(&feedback, 240)
                    ));
                    return Ok(AttemptEnd::RetryFailure(feedback));
                }

                let (proposal_id, applied_proposal_ids) =
                    terminal_ids(&applied).expect("applied is not empty");

                return Ok(AttemptEnd::Terminal(HeadlessTerminal::Applied {
                    proposal_id,
                    applied_proposal_ids,
                    request_id,
                    changed_paths: changed_paths.clone(),
                }));
            }
            _ => {}
        }
    }
}

fn terminal_ids(applied: &[AppliedItem]) -> Option<(Uuid, Vec<Uuid>)> {
    let proposal_ids = applied.iter().map(|item| item.id()).collect::<Vec<_>>();
    proposal_ids
        .last()
        .copied()
        .map(|primary| (primary, proposal_ids))
}

fn record_batch_terminal(
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
    let applied_outcome = wait_for_selected(runtime, turn, run, observer, &selected).await?;
    outcome.applied.extend(applied_outcome.applied);
    outcome.changed_paths.extend(applied_outcome.changed_paths);
    if applied_outcome.retry.is_some() {
        outcome.retry = applied_outcome.retry;
    }
    if !outcome.applied.is_empty() {
        wait_for_refresh(runtime, pending_events, turn, observer).await?;
    }
    Ok(outcome)
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

fn select_disjoint(
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

async fn wait_for_refresh(
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

fn sparse_search_refresh_enabled(
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadlessRun {
    attempts: Vec<HeadlessAttempt>,
    events: Vec<Event>,
    validations: Vec<CargoValidationObservation>,
    debug_relay: DebugRelay,
    prompt_diagnostics: Vec<PromptDiagnostic>,
    terminal: Option<HeadlessTerminal>,
}

impl HeadlessRun {
    fn new() -> Self {
        Self {
            attempts: Vec::new(),
            events: Vec::new(),
            validations: Vec::new(),
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

    pub(crate) fn validations(&self) -> &[CargoValidationObservation] {
        &self.validations
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

    fn has_observed_activity(&self) -> bool {
        !self.attempts.is_empty() || !self.events.is_empty() || !self.prompt_diagnostics.is_empty()
    }

    pub(crate) fn applied_edit(&self) -> Option<AppliedEdit> {
        let mut proposal_ids = Vec::new();
        let mut changed_paths = Vec::new();
        for attempt in &self.attempts {
            let HeadlessAttemptResult::Applied { paths } = &attempt.result else {
                continue;
            };
            let Some(proposal_id) = attempt.proposal_id else {
                continue;
            };
            proposal_ids.push(proposal_id);
            push_changed_paths(&mut changed_paths, paths.clone());
        }
        proposal_ids.last().copied().map(|proposal_id| AppliedEdit {
            proposal_id,
            proposal_ids,
            changed_paths,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_parts_for_test(
        attempts: Vec<HeadlessAttempt>,
        terminal: Option<HeadlessTerminal>,
    ) -> Self {
        Self {
            attempts,
            events: Vec::new(),
            validations: Vec::new(),
            debug_relay: DebugRelay::new(),
            prompt_diagnostics: Vec::new(),
            terminal,
        }
    }
}

fn observed_headless_error(source: Error) -> String {
    format!("headless runtime failed after observed activity: {source}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CargoValidationObservation {
    call_id: String,
    command: String,
    display_command: String,
    ok: bool,
    status_reason: String,
    exit_code: Option<i32>,
    manifest_path: String,
    errors: u32,
    warnings: u32,
}

impl CargoValidationObservation {
    fn failure_feedback(&self) -> Option<String> {
        if self.ok {
            return None;
        }
        Some(format!(
            "Cargo validation failed after applying edits: `{}` exited {:?} with status `{}` (errors: {}, warnings: {}). Repair the failure before claiming success.",
            self.display_command, self.exit_code, self.status_reason, self.errors, self.warnings
        ))
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CargoRequestArgs {
    command: Option<String>,
    package: Option<String>,
    all_features: Option<bool>,
    no_default_features: Option<bool>,
    features: Option<Vec<String>>,
    target: Option<String>,
    profile: Option<String>,
    release: Option<bool>,
    lib: Option<bool>,
    tests: Option<bool>,
    bins: Option<bool>,
    examples: Option<bool>,
    benches: Option<bool>,
    test_args: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct CargoResultProjection {
    ok: bool,
    status_reason: String,
    command: String,
    manifest_path: String,
    exit_code: Option<i32>,
    summary: CargoSummaryProjection,
}

#[derive(Debug, Deserialize)]
struct CargoSummaryProjection {
    errors: u32,
    warnings: u32,
}

fn observe_cargo_validation(
    run: &mut HeadlessRun,
    call_id: &str,
    arguments: &str,
    content: &str,
) -> Option<CargoValidationObservation> {
    let result = serde_json::from_str::<CargoResultProjection>(content).ok()?;
    let args = serde_json::from_str::<CargoRequestArgs>(arguments).unwrap_or_default();
    let observation = CargoValidationObservation {
        call_id: call_id.to_string(),
        command: result.command.clone(),
        display_command: display_cargo_command(&args, &result.command),
        ok: result.ok,
        status_reason: result.status_reason,
        exit_code: result.exit_code,
        manifest_path: result.manifest_path,
        errors: result.summary.errors,
        warnings: result.summary.warnings,
    };
    run.validations.push(observation.clone());
    Some(observation)
}

fn display_cargo_command(args: &CargoRequestArgs, result_command: &str) -> String {
    let mut parts = vec![
        "cargo".to_string(),
        args.command
            .clone()
            .unwrap_or_else(|| result_command.to_string()),
    ];
    if let Some(package) = &args.package {
        parts.push("-p".to_string());
        parts.push(package.clone());
    }
    if args.all_features.unwrap_or(false) {
        parts.push("--all-features".to_string());
    }
    if args.no_default_features.unwrap_or(false) {
        parts.push("--no-default-features".to_string());
    }
    if let Some(features) = &args.features
        && !features.is_empty()
    {
        parts.push("--features".to_string());
        parts.push(features.join(","));
    }
    if let Some(target) = &args.target {
        parts.push("--target".to_string());
        parts.push(target.clone());
    }
    if let Some(profile) = &args.profile {
        parts.push("--profile".to_string());
        parts.push(profile.clone());
    }
    if args.release.unwrap_or(false) {
        parts.push("--release".to_string());
    }
    for (enabled, flag) in [
        (args.lib, "--lib"),
        (args.tests, "--tests"),
        (args.bins, "--bins"),
        (args.examples, "--examples"),
        (args.benches, "--benches"),
    ] {
        if enabled.unwrap_or(false) {
            parts.push(flag.to_string());
        }
    }
    if let Some(test_args) = &args.test_args
        && !test_args.is_empty()
    {
        parts.push("--".to_string());
        parts.extend(test_args.iter().cloned());
    }
    parts.join(" ")
}

fn latest_failed_cargo_validation_feedback(run: &HeadlessRun) -> Option<String> {
    run.validations()
        .last()
        .and_then(CargoValidationObservation::failure_feedback)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppliedEdit {
    proposal_id: Uuid,
    proposal_ids: Vec<Uuid>,
    changed_paths: Vec<PathBuf>,
}

impl AppliedEdit {
    pub(crate) fn proposal_id(&self) -> Uuid {
        self.proposal_id
    }

    pub(crate) fn proposal_ids(&self) -> &[Uuid] {
        &self.proposal_ids
    }

    pub(crate) fn changed_paths(&self) -> &[PathBuf] {
        &self.changed_paths
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
        .filter(|message| {
            let content = message.content.as_str();
            content.contains("proceeding without code context")
                || content.starts_with("Context mode is Off:")
        })
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

    #[cfg(test)]
    pub(crate) fn applied_for_test(turn: u32, proposal_id: Uuid, paths: Vec<PathBuf>) -> Self {
        Self {
            turn,
            proposal_id: Some(proposal_id),
            result: HeadlessAttemptResult::Applied { paths },
        }
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
        applied_proposal_ids: Vec<Uuid>,
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
                applied_proposal_ids,
                changed_paths,
                ..
            } => format!(
                "applied proposal_id={} applied_proposals={} changed_paths={}",
                proposal_id,
                applied_proposal_ids.len(),
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
    use uuid::Uuid;

    use super::{
        CargoValidationObservation, DebugRelay, HeadlessAttemptResult, HeadlessRun,
        HeadlessTerminal, MAX_EVIDENCE_EVENT_CHARS, truncate_chars,
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
        pub(crate) validations: Vec<CargoValidation>,
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
    pub(crate) struct CargoValidation {
        pub(crate) call_id: String,
        pub(crate) command: String,
        pub(crate) display_command: String,
        pub(crate) ok: bool,
        pub(crate) status_reason: String,
        pub(crate) exit_code: Option<i32>,
        pub(crate) manifest_path: String,
        pub(crate) errors: u32,
        pub(crate) warnings: u32,
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
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            applied_proposal_ids: Vec<Uuid>,
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
                validations: value
                    .validations
                    .iter()
                    .map(CargoValidation::from)
                    .collect(),
                debug_relay: DebugRelaySummary::from(&value.debug_relay),
                prompt_diagnostics: value.prompt_diagnostics.iter().map(Prompt::from).collect(),
            }
        }
    }

    impl From<&CargoValidationObservation> for CargoValidation {
        fn from(value: &CargoValidationObservation) -> Self {
            Self {
                call_id: value.call_id.clone(),
                command: value.command.clone(),
                display_command: value.display_command.clone(),
                ok: value.ok,
                status_reason: value.status_reason.clone(),
                exit_code: value.exit_code,
                manifest_path: value.manifest_path.clone(),
                errors: value.errors,
                warnings: value.warnings,
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
                    applied_proposal_ids,
                    request_id,
                    changed_paths,
                } => Self::Applied {
                    proposal_id: proposal_id.to_string(),
                    applied_proposal_ids: applied_proposal_ids.clone(),
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
    fn model_selection_sets_openrouter_route() {
        let model_id = "moonshotai/kimi-k2".parse().expect("model id");
        let provider = ProviderKey::new("moonshotai").expect("provider");

        let selection = ModelSelection::openrouter(model_id, Some(provider.clone()));

        assert!(matches!(selection.router(), RouterVariants::OpenRouter(_)));
        assert_eq!(selection.provider(), Some(&provider));
    }

    #[test]
    fn model_selection_sets_direct_google_route_without_provider_pin() {
        let model_id = "google/gemini-2.5-flash".parse().expect("model id");

        let selection = ModelSelection::direct_google(model_id);

        assert!(matches!(selection.router(), RouterVariants::Google(_)));
        assert!(selection.provider().is_none());
    }

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
    fn tool_batch_waits_for_every_requested_call() {
        let request_id = Uuid::new_v4();
        let first = ploke_core::ArcStr::from("call-first");
        let second = ploke_core::ArcStr::from("call-second");
        let staged = StagedItem::Edit(Uuid::new_v4());
        let mut batches = HashMap::<Uuid, ToolBatch>::new();

        batches
            .entry(request_id)
            .or_default()
            .request(first.clone());
        batches
            .entry(request_id)
            .or_default()
            .request(second.clone());

        assert!(record_batch_terminal(&mut batches, request_id, first, Some(staged)).is_none());
        assert_eq!(
            record_batch_terminal(&mut batches, request_id, second, None),
            Some(vec![staged])
        );
        assert!(!batches.contains_key(&request_id));
    }

    #[test]
    fn failed_cargo_tool_result_becomes_structured_validation_feedback() {
        let mut run = HeadlessRun::new();
        let arguments = r#"{"command":"test","package":"syn_parser"}"#;
        let content = r#"{
            "ok": false,
            "status_reason": "tests_failed_or_runtime",
            "command": "test",
            "scope": "workspace",
            "manifest_path": "/repo/Cargo.toml",
            "exit_code": 101,
            "duration_ms": 42,
            "summary": {
                "errors": 0,
                "warnings": 0,
                "notes": 0,
                "artifacts": 10,
                "other_messages": 0
            },
            "diagnostics": [],
            "stderr_tail": [],
            "non_json_stdout_tail": [],
            "json_parse_errors_tail": [],
            "raw_messages_truncated": false
        }"#;

        let observation =
            observe_cargo_validation(&mut run, "call-cargo", arguments, content).expect("cargo");

        assert!(!observation.ok);
        assert_eq!(observation.display_command, "cargo test -p syn_parser");
        assert_eq!(
            latest_failed_cargo_validation_feedback(&run).as_deref(),
            Some(
                "Cargo validation failed after applying edits: `cargo test -p syn_parser` exited Some(101) with status `tests_failed_or_runtime` (errors: 0, warnings: 0). Repair the failure before claiming success."
            )
        );
        let evidence = run.evidence();
        assert_eq!(evidence.validations.len(), 1);
        assert_eq!(
            evidence.validations[0].display_command,
            "cargo test -p syn_parser"
        );
        assert!(!evidence.validations[0].ok);
    }

    #[test]
    fn later_successful_cargo_result_clears_latest_failure_gate() {
        let mut run = HeadlessRun::new();
        let failed = r#"{
            "ok": false,
            "status_reason": "tests_failed_or_runtime",
            "command": "test",
            "scope": "workspace",
            "manifest_path": "/repo/Cargo.toml",
            "exit_code": 101,
            "duration_ms": 42,
            "summary": {
                "errors": 0,
                "warnings": 0,
                "notes": 0,
                "artifacts": 10,
                "other_messages": 0
            },
            "diagnostics": [],
            "stderr_tail": [],
            "non_json_stdout_tail": [],
            "json_parse_errors_tail": [],
            "raw_messages_truncated": false
        }"#;
        let passed = r#"{
            "ok": true,
            "status_reason": "success",
            "command": "test",
            "scope": "workspace",
            "manifest_path": "/repo/Cargo.toml",
            "exit_code": 0,
            "duration_ms": 42,
            "summary": {
                "errors": 0,
                "warnings": 0,
                "notes": 0,
                "artifacts": 10,
                "other_messages": 0
            },
            "diagnostics": [],
            "stderr_tail": [],
            "non_json_stdout_tail": [],
            "json_parse_errors_tail": [],
            "raw_messages_truncated": false
        }"#;

        observe_cargo_validation(
            &mut run,
            "call-failed",
            r#"{"command":"test","package":"syn_parser"}"#,
            failed,
        )
        .expect("failed cargo");
        observe_cargo_validation(
            &mut run,
            "call-passed",
            r#"{"command":"test","package":"syn_parser"}"#,
            passed,
        )
        .expect("passed cargo");

        assert!(latest_failed_cargo_validation_feedback(&run).is_none());
        assert_eq!(run.evidence().validations.len(), 2);
    }

    #[test]
    fn select_disjoint_keeps_newest_file_disjoint_candidates() {
        let workspace = Path::new("/repo");
        let newer_same_file = Candidate {
            item: StagedItem::Edit(Uuid::from_u128(2)),
            proposed_at_ms: 200,
            paths: vec![PathBuf::from("/repo/crates/ploke-tui/src/lib.rs")],
        };
        let other_file = Candidate {
            item: StagedItem::Edit(Uuid::from_u128(3)),
            proposed_at_ms: 150,
            paths: vec![PathBuf::from("crates/ploke-rag/src/lib.rs")],
        };
        let older_same_file = Candidate {
            item: StagedItem::Edit(Uuid::from_u128(1)),
            proposed_at_ms: 100,
            paths: vec![PathBuf::from("crates/ploke-tui/src/lib.rs")],
        };

        let (selected, rejected) = select_disjoint(
            vec![
                older_same_file.clone(),
                other_file.clone(),
                newer_same_file.clone(),
            ],
            workspace,
        );

        assert_eq!(
            selected
                .iter()
                .map(|candidate| candidate.item)
                .collect::<Vec<_>>(),
            vec![newer_same_file.item, other_file.item]
        );
        assert_eq!(
            rejected
                .iter()
                .map(|candidate| candidate.item)
                .collect::<Vec<_>>(),
            vec![older_same_file.item]
        );
    }

    #[test]
    fn sparse_post_apply_refresh_gate_uses_sparse_search_config() {
        use ploke_tui::user_config::RetrievalStrategyUser;

        assert!(sparse_search_refresh_enabled(
            &RetrievalStrategyUser::Sparse { strict: true },
            false
        ));
        assert!(!sparse_search_refresh_enabled(
            &RetrievalStrategyUser::Sparse { strict: false },
            false
        ));
        assert!(sparse_search_refresh_enabled(
            &RetrievalStrategyUser::Sparse { strict: false },
            true
        ));
        assert!(!sparse_search_refresh_enabled(
            &RetrievalStrategyUser::Dense,
            true
        ));
        assert!(!sparse_search_refresh_enabled(
            &RetrievalStrategyUser::default(),
            false
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn sparse_post_apply_refresh_returns_on_bm25_without_dense_index_completion() {
        let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
        let fixture = prepare_live_canary(
            "sparse-post-apply-refresh",
            "runtime refresh only; no LLM prompt is submitted",
        )
        .expect("prepare sparse refresh fixture");
        let mut runtime = crate::runner::setup_workspace_tui_runtime(&fixture.workspace)
            .await
            .expect("start sparse refresh runtime");
        runtime.app.pump_pending_events().await;

        fs::write(
            &fixture.src_file,
            r#"pub fn broad_surface_canary() -> &'static str {
    "after"
}
"#,
        )
        .expect("write changed source before refresh");

        let mut pending_events = VecDeque::new();
        tokio::time::timeout(
            Duration::from_secs(10),
            wait_for_refresh(
                &mut runtime,
                &mut pending_events,
                1,
                &LiveObserver { enabled: false },
            ),
        )
        .await
        .expect("sparse refresh should not wait for dense IndexingCompleted")
        .expect("sparse refresh should succeed");

        let status = runtime
            .state
            .rag
            .as_ref()
            .expect("RAG service")
            .bm25_status()
            .await
            .expect("BM25 status after sparse refresh");
        assert!(
            matches!(status, ploke_db::bm25_index::bm25_service::Bm25Status::Ready { docs } if docs > 0),
            "expected BM25 ready after sparse refresh, got {status:?}"
        );
    }

    #[test]
    fn terminal_ids_use_last_applied_proposal_as_primary() {
        let first = AppliedItem::Edit(Uuid::from_u128(1));
        let second = AppliedItem::Edit(Uuid::from_u128(2));
        let third = AppliedItem::Edit(Uuid::from_u128(3));

        let (primary, proposal_ids) = terminal_ids(&[first, second, third]).expect("terminal ids");

        assert_eq!(primary, third.id());
        assert_eq!(proposal_ids, vec![first.id(), second.id(), third.id()]);
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
            "Modify any part of the codebase at `/tmp/prototype1/workspace`.\n\nPast benchmark results live under `/tmp/prototype1/evaluations`.\n",
            Some("tool failed"),
        );

        assert!(prompt.starts_with("Modify any part of the codebase at"));
        assert!(prompt.contains("Past benchmark results live under"));
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
        assert!(prompt.contains("Protected core: see"));
        assert!(prompt.contains("WORKSPACE_EXCEPT_AUTHORITY_*"));
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
            validations: Vec::new(),
            debug_relay: DebugRelay::new(),
            prompt_diagnostics: Vec::new(),
            terminal: Some(HeadlessTerminal::Applied {
                proposal_id,
                applied_proposal_ids: vec![proposal_id],
                request_id,
                changed_paths: changed_paths.clone(),
            }),
        };

        let summary = run.evidence();
        let proposal_uuid = proposal_id;
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
                applied_proposal_ids,
                request_id: observed_request,
                changed_paths: observed_paths,
            }) if observed_proposal == &proposal_id
                && applied_proposal_ids.as_slice() == &[proposal_uuid]
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
            validations: Vec::new(),
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
    fn evidence_can_preserve_observed_headless_runtime_error() {
        let proposal_id = Uuid::from_u128(4);
        let changed_paths = vec![PathBuf::from("crates/ploke-llm/src/types/meta.rs")];
        let error = observed_headless_error(Error::HeadlessEvent(
            "timed out waiting for indexing completion after applying proposal batch after 180s"
                .to_string(),
        ));
        let run = HeadlessRun::from_parts_for_test(
            vec![HeadlessAttempt::applied_for_test(
                1,
                proposal_id,
                changed_paths.clone(),
            )],
            Some(HeadlessTerminal::ToolFailed {
                error: error.clone(),
            }),
        );

        assert!(run.has_observed_activity());
        let summary = run.evidence();

        assert!(matches!(
            &summary.attempts[0].result,
            evidence::Result::Applied { paths } if paths == &changed_paths
        ));
        assert!(matches!(
            summary.terminal.as_ref(),
            Some(evidence::Terminal::ToolFailed { error: observed })
                if observed == &error
                    && observed.contains("headless runtime failed after observed activity")
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
            validations: Vec::new(),
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
        let fallback = "Context mode is Off: Context will not automatically be attached to the user message.; Context search via request_code_context is still available. workspace loaded /tmp/candidate.";
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

    // regr:protectedstaged:19-05-26_15-43
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn recorded_replay_rejects_protected_ns_patch_before_staged_success_reaches_model() {
        let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
        let fixture = prepare_live_canary(
            "recorded-protected-ns-patch-replay",
            "Use non_semantic_patch to edit crates/ploke-eval/src/lib.rs.",
        )
        .expect("prepare recorded replay fixture");
        let protected_rel = Path::new("crates/ploke-eval/src/lib.rs");
        let protected_abs = fixture.workspace.join(protected_rel);
        fs::create_dir_all(protected_abs.parent().expect("protected file parent"))
            .expect("create protected file parent");
        fs::write(
            &protected_abs,
            r#"pub fn protected_replay_canary() -> &'static str {
    "before"
}
"#,
        )
        .expect("write protected file");

        let call_id = "call_protected_ns_patch";
        let tape = recorded_protected_ns_patch_tape(&fixture.artifact_root, call_id, protected_rel);
        ploke_tui::llm::install_recorded_response_tape(tape);
        let _clear_tape = ClearRecordedTapeOnDrop;

        let (request_tx, request_rx) = std::sync::mpsc::channel();
        let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);
        let (mut runtime, parent_id) = start_attempt_runtime(
            &fixture.workspace,
            &[],
            fixture.prompt.clone(),
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            None,
        )
        .await
        .expect("start recorded replay runtime");

        let mut snapshots = Vec::new();
        let mut failed_tool = None;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            runtime.app.pump_pending_events().await;
            collect_request_snapshots(&request_rx, &mut snapshots);
            if snapshots.len() >= 2 && failed_tool.is_some() {
                break;
            }

            let event =
                match tokio::time::timeout(Duration::from_millis(100), next_event(&mut runtime))
                    .await
                {
                    Ok(Ok(event)) => event,
                    Ok(Err(err)) => panic!("recorded replay event stream failed: {err}"),
                    Err(_) => continue,
                };

            if let ploke_tui::AppEvent::System(
                ploke_tui::app_state::events::SystemEvent::ToolCallFailed {
                    parent_id: event_parent_id,
                    call_id: event_call_id,
                    error,
                    ..
                },
            ) = event
                && event_parent_id == parent_id
                && event_call_id.as_ref() == call_id
            {
                let wire = ploke_tui::tools::ToolErrorWire::parse(&error)
                    .expect("protected ns_patch failure should use tool error wire");
                assert!(!wire.llm.ok);
                assert!(
                    wire.llm.message.contains("protected"),
                    "expected protected-path failure, got {error}"
                );
                failed_tool = Some(error);
            }
        }
        collect_request_snapshots(&request_rx, &mut snapshots);
        let proposals = runtime.state.proposals.read().await;
        assert!(
            proposals.is_empty(),
            "protected ns_patch should fail before staging a proposal, got {:?}",
            proposals.keys().collect::<Vec<_>>()
        );
        drop(proposals);

        let second_request = snapshots.get(1).unwrap_or_else(|| {
            panic!(
                "expected replay to capture the second provider request; captured {} requests",
                snapshots.len()
            )
        });
        assert!(
            model_request_contains_tool_rejection(second_request, call_id),
            "expected second request to contain a tool rejection for protected call {call_id}; request={second_request:#?}"
        );
        assert!(
            !model_request_contains_staged_success(second_request, call_id),
            "protected tool failure must not be replayed as staged success; request={second_request:#?}"
        );
        assert!(
            failed_tool.is_some(),
            "expected protected tool failure before assertion"
        );
    }

    /// Regression test for the protected-manifest retry loop observed in the
    /// `node-01c9e8fdc70e3ee8` headless TUI trace.
    ///
    /// This is fixed-contract regression coverage tracked as resolved, not an
    /// expected-failing case. It replays only the relevant failure shape instead
    /// of the full trace.
    ///
    /// The historical run repeatedly attempted the same `non_semantic_patch`
    /// against workspace `Cargo.toml`, and the old tool response gave the
    /// model another generic path hint instead of recording that this was a
    /// repeated protected write. This test reads the historical headless trace
    /// through the typed `evidence::Summary` projection, extracts the first two
    /// real provider-emitted `non_semantic_patch` requests, decodes them as
    /// typed `NsPatchParamsOwned`, rebases only the old workspace root onto this
    /// test's isolated workspace, and then wraps the recovered model output as
    /// `RawFullResponseRecord` lines loaded through `load_recorded_response_tape`.
    ///
    /// The assertions pin the fixed contract: both protected attempts fail before
    /// staging, no success/completion is emitted for either call, the second
    /// denial is marked `retry_context.repeated = true`, and the next model
    /// requests receive structured rejection messages rather than staged-success
    /// payloads.
    // regr:protectedrepeat:19-05-26_15-43
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn historical_trace_replay_marks_repeated_protected_ns_patch_before_staging() {
        let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
        let fixture = prepare_live_canary(
            "historical-trace-protected-cargo-repeat",
            "Replay historical repeated Cargo.toml protected edit attempts.",
        )
        .expect("prepare recorded replay fixture");

        let historical_requests =
            historical_repeated_cargo_ns_patch_requests(&fixture.workspace, 2);
        let call_ids = historical_requests
            .iter()
            .map(|request| request.call_id.as_str())
            .collect::<Vec<_>>();
        let tape =
            recorded_historical_ns_patch_tape(&fixture.artifact_root, historical_requests.clone());
        ploke_tui::llm::install_recorded_response_tape(tape);
        let _clear_tape = ClearRecordedTapeOnDrop;

        let (request_tx, request_rx) = std::sync::mpsc::channel();
        let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);
        let (mut runtime, parent_id) = start_attempt_runtime(
            &fixture.workspace,
            &[],
            fixture.prompt.clone(),
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            None,
        )
        .await
        .expect("start recorded replay runtime");

        let mut snapshots = Vec::new();
        let mut failures = Vec::new();
        let mut completions = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            runtime.app.pump_pending_events().await;
            collect_request_snapshots(&request_rx, &mut snapshots);
            if failures.len() >= 2 && snapshots.len() >= 3 {
                break;
            }

            let event =
                match tokio::time::timeout(Duration::from_millis(100), next_event(&mut runtime))
                    .await
                {
                    Ok(Ok(event)) => event,
                    Ok(Err(err)) => panic!("recorded replay event stream failed: {err}"),
                    Err(_) => continue,
                };

            match event {
                ploke_tui::AppEvent::System(
                    ploke_tui::app_state::events::SystemEvent::ToolCallFailed {
                        parent_id: event_parent_id,
                        call_id: event_call_id,
                        error,
                        ..
                    },
                ) if event_parent_id == parent_id
                    && call_ids
                        .iter()
                        .any(|expected| event_call_id.as_ref() == *expected) =>
                {
                    failures.push((event_call_id.to_string(), error));
                }
                ploke_tui::AppEvent::System(
                    ploke_tui::app_state::events::SystemEvent::ToolCallCompleted {
                        parent_id: event_parent_id,
                        call_id: event_call_id,
                        ..
                    },
                ) if event_parent_id == parent_id
                    && call_ids
                        .iter()
                        .any(|expected| event_call_id.as_ref() == *expected) =>
                {
                    completions.push(event_call_id.to_string());
                }
                _ => {}
            }
        }
        collect_request_snapshots(&request_rx, &mut snapshots);

        assert_eq!(
            failures.len(),
            2,
            "both historical protected attempts should fail before staging"
        );
        assert!(
            completions.is_empty(),
            "protected preflight should not emit completions for historical calls: {completions:?}"
        );
        let proposals = runtime.state.proposals.read().await;
        assert!(
            proposals.is_empty(),
            "historical protected Cargo.toml replay should not stage proposals, got {:?}",
            proposals.keys().collect::<Vec<_>>()
        );
        drop(proposals);

        let first = ploke_tui::tools::ToolErrorWire::parse(&failures[0].1)
            .expect("first historical protected failure should use tool error wire");
        let second = ploke_tui::tools::ToolErrorWire::parse(&failures[1].1)
            .expect("second historical protected failure should use tool error wire");
        assert_eq!(
            first.llm.code,
            ploke_tui::tools::ToolErrorCode::InvalidFormat
        );
        assert_eq!(
            second.llm.code,
            ploke_tui::tools::ToolErrorCode::InvalidFormat
        );
        assert_eq!(retry_context_bool(&first, "repeated"), Some(false));
        assert_eq!(retry_context_bool(&second, "repeated"), Some(true));
        assert!(
            second
                .llm
                .retry_hint
                .as_deref()
                .is_some_and(|hint| hint.contains("already denied")),
            "repeat denial should tell the model the target was already denied: {:?}",
            second.llm.retry_hint
        );

        let second_request = snapshots.get(1).unwrap_or_else(|| {
            panic!(
                "expected second provider request after first rejection; captured {} requests",
                snapshots.len()
            )
        });
        assert!(
            model_request_contains_tool_rejection(second_request, call_ids[0]),
            "expected second request to carry first protected rejection; request={second_request:#?}"
        );
        assert!(
            !model_request_contains_staged_success(second_request, call_ids[0]),
            "first protected rejection must not be converted to staged success; request={second_request:#?}"
        );

        let third_request = snapshots.get(2).unwrap_or_else(|| {
            panic!(
                "expected third provider request after repeated rejection; captured {} requests",
                snapshots.len()
            )
        });
        assert!(
            model_request_contains_tool_rejection(third_request, call_ids[1]),
            "expected third request to carry repeated protected rejection; request={third_request:#?}"
        );
        assert!(
            !model_request_contains_staged_success(third_request, call_ids[1]),
            "repeated protected rejection must not be converted to staged success; request={third_request:#?}"
        );
    }

    /// Fixed-contract replay coverage for RF-05: repeated same-file repair
    /// attempts must settle through the real tool/proposal loop without leaving
    /// a malformed intermediate artifact.
    ///
    /// The provider tape asks for one valid `non_semantic_patch` edit and then a
    /// stale same-file repair against the pre-apply content. The fixed behavior
    /// is: the first edit applies, the stale repair fails before staging a
    /// second proposal, the next provider request receives a rejection for the
    /// stale call, and the workspace remains at the first valid edit.
    ///
    /// Related RF-05 bug report:
    /// docs/active/bugs/2026-05-19-rf-05-edit-composition-same-file-repair.md.
    // regr:samefile:19-05-26_06-42
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn recorded_replay_rejects_stale_same_file_repair_after_first_apply() {
        let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
        let fixture = prepare_live_canary(
            "recorded-same-file-stale-repair",
            "Replay repeated same-file non_semantic_patch repair attempts.",
        )
        .expect("prepare recorded same-file replay fixture");

        let first_call_id = "call_same_file_first_apply";
        let stale_call_id = "call_same_file_stale_repair";
        let tape =
            recorded_same_file_repair_tape(&fixture.artifact_root, first_call_id, stale_call_id);
        ploke_tui::llm::install_recorded_response_tape(tape);
        let _clear_tape = ClearRecordedTapeOnDrop;

        let (request_tx, request_rx) = std::sync::mpsc::channel();
        let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);
        let (mut runtime, parent_id) = start_attempt_runtime(
            &fixture.workspace,
            &[],
            fixture.prompt.clone(),
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            None,
        )
        .await
        .expect("start same-file recorded replay runtime");

        let mut run = HeadlessRun::new();
        let outcome = run_attempt(
            &mut runtime,
            parent_id,
            &fixture.workspace,
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            1,
            &mut run,
            &LiveObserver { enabled: false },
        )
        .await
        .expect("same-file recorded replay should finish");

        let AttemptEnd::Terminal(HeadlessTerminal::Applied {
            applied_proposal_ids,
            changed_paths,
            ..
        }) = outcome
        else {
            panic!("same-file replay should finish with one applied edit, got {outcome:?}");
        };
        assert_eq!(
            applied_proposal_ids.len(),
            1,
            "stale same-file repair must not become a second applied proposal"
        );
        assert_eq!(
            changed_paths,
            vec![fixture.src_file.clone()],
            "only src/lib.rs should change"
        );

        let stale_failed = run.events().iter().any(|event| {
            matches!(
                event,
                Event::Tool {
                    call_id,
                    result: Tool::Failed { error },
                } if call_id == stale_call_id
                    && (error.contains("failed to patch")
                        || error.contains("No non-semantic edits were applied")
                        || error.contains("No patches were found")
                        || error.contains("Patch applied partially"))
            )
        });
        assert!(
            stale_failed,
            "stale same-file repair should fail before staging; events={:#?}",
            run.events()
        );
        assert!(
            !run.events().iter().any(|event| {
                matches!(
                    event,
                    Event::Tool {
                        call_id,
                        result: Tool::Completed { .. },
                    } if call_id == stale_call_id
                )
            }),
            "stale same-file repair must not produce a ToolCallCompleted success"
        );

        let proposals = runtime.state.proposals.read().await;
        assert_eq!(
            proposals.len(),
            1,
            "only the first same-file proposal should remain recorded, got {:?}",
            proposals.keys().collect::<Vec<_>>()
        );
        drop(proposals);

        let final_src =
            fs::read_to_string(&fixture.src_file).expect("read final same-file replay source");
        assert!(
            final_src.contains(r#""after""#),
            "first same-file edit should apply, got:\n{final_src}"
        );
        assert!(
            !final_src.contains("repair") && !final_src.contains("START RESTORE"),
            "stale repair artifacts must not be written, got:\n{final_src}"
        );

        let mut snapshots = Vec::new();
        collect_request_snapshots(&request_rx, &mut snapshots);
        let second_request = snapshots.get(1).unwrap_or_else(|| {
            panic!(
                "expected second provider request after first apply; captured {} requests",
                snapshots.len()
            )
        });
        assert!(
            model_request_contains_applied_success(second_request, first_call_id),
            "expected second request to contain settled applied result; request={second_request:#?}"
        );
        assert!(
            !model_request_contains_staged_success(second_request, first_call_id),
            "first same-file edit must not be replayed as staged-only success; request={second_request:#?}"
        );

        let third_request = snapshots.get(2).unwrap_or_else(|| {
            panic!(
                "expected third provider request after stale repair rejection; captured {} requests",
                snapshots.len()
            )
        });
        assert!(
            model_request_contains_ns_patch_failure(third_request, stale_call_id),
            "expected third request to carry stale same-file rejection; request={third_request:#?}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn gated_replay_sends_applied_ns_patch_instead_of_staged_success() {
        let _recorded_replay_guard = recorded_replay_test_mutex().lock().await;
        let fixture = prepare_live_canary(
            "recorded-gated-applied-ns-patch",
            "Use non_semantic_patch to update src/lib.rs.",
        )
        .expect("prepare recorded gated fixture");

        let call_id = "call_gated_allowed_ns_patch";
        let tape = recorded_allowed_ns_patch_tape(&fixture.artifact_root, call_id);
        ploke_tui::llm::install_recorded_response_tape(tape);
        let _clear_tape = ClearRecordedTapeOnDrop;

        let (request_tx, request_rx) = std::sync::mpsc::channel();
        let _tap_guard = ploke_tui::llm::install_request_tap(request_tx);
        let (mut runtime, parent_id) = start_attempt_runtime(
            &fixture.workspace,
            &[],
            fixture.prompt.clone(),
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            None,
        )
        .await
        .expect("start gated recorded replay runtime");

        let mut run = HeadlessRun::new();
        let outcome = run_attempt(
            &mut runtime,
            parent_id,
            &fixture.workspace,
            BroadEditPolicy::WorkspaceExceptPlokeEval,
            1,
            &mut run,
            &LiveObserver { enabled: false },
        )
        .await
        .expect("gated recorded replay should finish");
        assert!(
            matches!(
                outcome,
                AttemptEnd::Terminal(HeadlessTerminal::Applied { .. })
            ),
            "allowed ns_patch replay should apply, got {outcome:?}"
        );

        let mut snapshots = Vec::new();
        collect_request_snapshots(&request_rx, &mut snapshots);
        let second_request = snapshots.get(1).unwrap_or_else(|| {
            panic!(
                "expected second provider request after settled apply; captured {} requests",
                snapshots.len()
            )
        });
        assert!(
            model_request_contains_applied_success(second_request, call_id),
            "expected second request to contain settled applied result; request={second_request:#?}"
        );
        assert!(
            !model_request_contains_staged_success(second_request, call_id),
            "gated tool loop must not replay staged success before eval admission; request={second_request:#?}"
        );
        let final_src =
            fs::read_to_string(&fixture.src_file).expect("read final gated replay source");
        assert!(
            final_src.contains(r#""after""#),
            "gated replay should update source after admission, got:\n{final_src}"
        );
    }

    fn recorded_replay_test_mutex() -> &'static tokio::sync::Mutex<()> {
        crate::test_support::llm_lock()
    }

    struct ClearRecordedTapeOnDrop;

    impl Drop for ClearRecordedTapeOnDrop {
        fn drop(&mut self) {
            ploke_tui::llm::clear_recorded_response_tape();
        }
    }

    fn collect_request_snapshots(
        request_rx: &std::sync::mpsc::Receiver<Vec<ploke_tui::llm::RequestMessage>>,
        snapshots: &mut Vec<Vec<ploke_tui::llm::RequestMessage>>,
    ) {
        loop {
            match request_rx.try_recv() {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
    }

    fn recorded_protected_ns_patch_tape(
        run_dir: &Path,
        call_id: &str,
        protected_rel: &Path,
    ) -> ploke_llm::manager::RecordedResponseTape {
        let assistant_id = Uuid::new_v4();
        let request = ns_patch_request(
            call_id,
            protected_rel.display().to_string(),
            protected_ns_patch_diff(protected_rel),
            "exercise protected-path staged proposal replay",
            Some(0.9),
        );
        load_recorded_tape(
            run_dir,
            assistant_id,
            vec![
                tool_response_record(assistant_id, 0, "recorded-protected-ns-patch", &request),
                stop_response_record(assistant_id, 1, "recorded-final"),
            ],
        )
    }

    fn recorded_historical_ns_patch_tape(
        run_dir: &Path,
        requests: Vec<ploke_records::agent_turn::ToolRequestRecord>,
    ) -> ploke_llm::manager::RecordedResponseTape {
        let assistant_id = Uuid::new_v4();
        let mut records = requests
            .iter()
            .enumerate()
            .map(|(index, request)| {
                tool_response_record(
                    assistant_id,
                    index,
                    format!("historical-protected-cargo-{index}"),
                    request,
                )
            })
            .collect::<Vec<_>>();
        records.push(stop_response_record(
            assistant_id,
            requests.len(),
            "historical-protected-cargo-final",
        ));
        load_recorded_tape(run_dir, assistant_id, records)
    }

    fn historical_repeated_cargo_ns_patch_requests(
        workspace: &Path,
        count: usize,
    ) -> Vec<ploke_records::agent_turn::ToolRequestRecord> {
        let trace_path = Path::new(
            "/home/brasides/.ploke-eval/campaigns/p1-broad-batch-admission-20260518-2/prototype1/messages/edit-harness-result/node-01c9e8fdc70e3ee8.headless-tui.json",
        );
        let trace = fs::read_to_string(trace_path).unwrap_or_else(|source| {
            panic!(
                "read historical headless trace {}: {source}",
                trace_path.display()
            )
        });
        let summary: evidence::Summary = serde_json::from_str(&trace).unwrap_or_else(|source| {
            panic!(
                "parse historical headless trace {} as evidence::Summary: {source}",
                trace_path.display()
            )
        });

        let mut requests = Vec::new();
        for event in summary.events {
            let evidence::Event::ToolRequest {
                request_id,
                parent_id,
                call_id,
                tool,
                arguments,
            } = event
            else {
                continue;
            };
            if tool != "non_semantic_patch" {
                continue;
            }
            assert_eq!(
                arguments.chars,
                arguments.preview.chars().count(),
                "historical ns_patch arguments must be untruncated for typed replay"
            );
            let mut params = historical_ns_patch_params(&tool, &arguments.preview, &call_id);
            if !is_workspace_cargo_ns_patch(&params) {
                continue;
            }
            rebase_ns_patch_workspace(&mut params, workspace);
            let encoded =
                serde_json::to_string(&params).expect("serialize rebased historical ns_patch");
            let record = ploke_records::agent_turn::ToolRequestRecord {
                request_id,
                parent_id,
                call_id,
                tool,
                arguments: ploke_records::tool_contracts::ToolArgumentsJson::from(encoded),
            };
            assert_decodes_as_ns_patch(&record);
            requests.push(record);
            if requests.len() == count {
                break;
            }
        }
        assert_eq!(
            requests.len(),
            count,
            "expected {count} historical Cargo.toml ns_patch requests in replay trace"
        );
        requests
    }

    fn historical_ns_patch_params(
        tool: &str,
        arguments: &str,
        call_id: &str,
    ) -> ploke_records::tool_contracts::NsPatchParamsOwned {
        let captured = ploke_records::tool_contracts::ToolArgumentsJson::from(arguments);
        let decoded = captured.decode_for_tool(tool);
        let ploke_records::tool_contracts::PersistedToolCallArguments::Decoded(
            ploke_records::tool_contracts::ToolCallArguments::NsPatch(params),
        ) = decoded
        else {
            panic!("historical tool request {call_id} did not decode as non_semantic_patch");
        };
        params
    }

    fn is_workspace_cargo_ns_patch(
        params: &ploke_records::tool_contracts::NsPatchParamsOwned,
    ) -> bool {
        params.patches.len() == 1
            && params.patches[0].file.ends_with("/Cargo.toml")
            && params.patches[0].diff.contains("--- a/Cargo.toml")
    }

    fn rebase_ns_patch_workspace(
        params: &mut ploke_records::tool_contracts::NsPatchParamsOwned,
        workspace: &Path,
    ) {
        for patch in &mut params.patches {
            if patch.file.ends_with("/Cargo.toml") {
                patch.file = workspace.join("Cargo.toml").display().to_string();
            }
        }
    }

    fn recorded_allowed_ns_patch_tape(
        run_dir: &Path,
        call_id: &str,
    ) -> ploke_llm::manager::RecordedResponseTape {
        let assistant_id = Uuid::new_v4();
        let request = ns_patch_request(
            call_id,
            "src/lib.rs".to_string(),
            allowed_canary_ns_patch_diff(),
            "Change broad_surface_canary from before to after",
            Some(0.95),
        );
        load_recorded_tape(
            run_dir,
            assistant_id,
            vec![
                tool_response_record(assistant_id, 0, "recorded-allowed-ns-patch", &request),
                stop_response_record(assistant_id, 1, "recorded-allowed-final"),
            ],
        )
    }

    fn recorded_same_file_repair_tape(
        run_dir: &Path,
        first_call_id: &str,
        stale_call_id: &str,
    ) -> ploke_llm::manager::RecordedResponseTape {
        let assistant_id = Uuid::new_v4();
        let first = ns_patch_request(
            first_call_id,
            "src/lib.rs".to_string(),
            allowed_canary_ns_patch_diff(),
            "First same-file edit changes broad_surface_canary from before to after",
            Some(0.95),
        );
        let stale = ns_patch_request(
            stale_call_id,
            "src/lib.rs".to_string(),
            stale_canary_repair_ns_patch_diff(),
            "Stale repair attempt generated against the pre-apply same-file content",
            Some(0.80),
        );
        load_recorded_tape(
            run_dir,
            assistant_id,
            vec![
                tool_response_record(assistant_id, 0, "recorded-same-file-first", &first),
                tool_response_record(assistant_id, 1, "recorded-same-file-stale-repair", &stale),
                stop_response_record(assistant_id, 2, "recorded-same-file-final"),
            ],
        )
    }

    fn ns_patch_request(
        call_id: &str,
        file: String,
        diff: String,
        reasoning: &str,
        confidence: Option<f32>,
    ) -> ploke_records::agent_turn::ToolRequestRecord {
        let params = ploke_records::tool_contracts::NsPatchParamsOwned {
            patches: vec![ploke_records::tool_contracts::NsPatchOwned {
                file,
                diff,
                reasoning: reasoning.to_string(),
            }],
            confidence,
        };
        let arguments = serde_json::to_string(&params).expect("serialize typed ns_patch params");
        let record = ploke_records::agent_turn::ToolRequestRecord {
            request_id: format!("{call_id}-request"),
            parent_id: "recorded-replay-parent".to_string(),
            call_id: call_id.to_string(),
            tool: "non_semantic_patch".to_string(),
            arguments: ploke_records::tool_contracts::ToolArgumentsJson::from(arguments),
        };
        assert_decodes_as_ns_patch(&record);
        record
    }

    fn assert_decodes_as_ns_patch(record: &ploke_records::agent_turn::ToolRequestRecord) {
        let decoded = record.arguments.decode_for_tool(&record.tool);
        let ploke_records::tool_contracts::PersistedToolCallArguments::Decoded(
            ploke_records::tool_contracts::ToolCallArguments::NsPatch(arguments),
        ) = decoded
        else {
            panic!("expected typed ns_patch arguments for {}", record.call_id);
        };
        assert_eq!(arguments.patches.len(), 1);
        assert!(
            arguments.patches[0].diff.starts_with("--- a/"),
            "historical replay should preserve a unified diff"
        );
    }

    fn load_recorded_tape(
        run_dir: &Path,
        assistant_id: Uuid,
        records: Vec<ploke_records::llm_response::RawFullResponseRecord>,
    ) -> ploke_llm::manager::RecordedResponseTape {
        fs::create_dir_all(run_dir).expect("create recorded response fixture dir");
        let path = run_dir.join(ploke_records::llm_response::FULL_RESPONSE_TRACE_FILE);
        let mut jsonl = String::new();
        for record in &records {
            assert!(record.matches_assistant_message(assistant_id));
            jsonl.push_str(&serde_json::to_string(record).expect("serialize full response record"));
            jsonl.push('\n');
        }
        fs::write(&path, jsonl).expect("write full response fixture");
        crate::replay::llm::load_recorded_response_tape(run_dir, &assistant_id.to_string())
            .expect("load recorded response tape through full-response record loader")
    }

    fn tool_response_record(
        assistant_id: Uuid,
        response_index: usize,
        response_id: impl Into<String>,
        request: &ploke_records::agent_turn::ToolRequestRecord,
    ) -> ploke_records::llm_response::RawFullResponseRecord {
        assert_decodes_as_ns_patch(request);
        let response = serde_json::from_value(serde_json::json!({
            "id": response_id.into(),
            "choices": [{
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": request.call_id,
                        "type": "function",
                        "function": {
                            "name": request.tool,
                            "arguments": request.arguments.as_str(),
                        }
                    }]
                }
            }],
            "created": response_index,
            "model": "test/model",
            "object": "chat.completion"
        }))
        .expect("recorded ns_patch provider response should parse");
        ploke_records::llm_response::RawFullResponseRecord {
            assistant_message_id: assistant_id,
            recorded_response: ploke_llm::manager::RecordedResponse::new(response_index, response),
        }
    }

    fn stop_response_record(
        assistant_id: Uuid,
        response_index: usize,
        response_id: impl Into<String>,
    ) -> ploke_records::llm_response::RawFullResponseRecord {
        let response = serde_json::from_value(serde_json::json!({
            "id": response_id.into(),
            "choices": [{
                "index": 0,
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": "done"
                }
            }],
            "created": response_index,
            "model": "test/model",
            "object": "chat.completion"
        }))
        .expect("recorded final provider response should parse");
        ploke_records::llm_response::RawFullResponseRecord {
            assistant_message_id: assistant_id,
            recorded_response: ploke_llm::manager::RecordedResponse::new(response_index, response),
        }
    }

    fn protected_ns_patch_diff(protected_rel: &Path) -> String {
        let path = protected_rel.display();
        format!(
            r#"--- a/{path}
+++ b/{path}
@@ -1,3 +1,3 @@
 pub fn protected_replay_canary() -> &'static str {{
-    "before"
+    "after"
 }}
"#
        )
    }

    fn allowed_canary_ns_patch_diff() -> String {
        [
            "--- a/src/lib.rs",
            "+++ b/src/lib.rs",
            "@@ -1,3 +1,3 @@",
            " pub fn broad_surface_canary() -> &'static str {",
            "-    \"before\"",
            "+    \"after\"",
            " }",
            "",
        ]
        .join("\n")
    }

    fn stale_canary_repair_ns_patch_diff() -> String {
        [
            "--- a/src/lib.rs",
            "+++ b/src/lib.rs",
            "@@ -1,3 +1,3 @@",
            " pub fn broad_surface_canary() -> &'static str {",
            "-    \"before\"",
            "+    \"repair\"",
            " }",
            "",
        ]
        .join("\n")
    }

    fn retry_context_bool(wire: &ploke_tui::tools::ToolErrorWire, field: &str) -> Option<bool> {
        match wire.llm["retry_context"].as_object()?.get(field)? {
            ploke_tui::tools::ToolLlmErrorValue::Bool(value) => Some(*value),
            _ => None,
        }
    }

    fn model_request_contains_staged_success(
        messages: &[ploke_tui::llm::RequestMessage],
        call_id: &str,
    ) -> bool {
        messages.iter().any(|message| {
            message.role == ploke_llm::manager::Role::Tool
                && message
                    .tool_call_id
                    .as_ref()
                    .is_some_and(|tool_call_id| tool_call_id.as_ref() == call_id)
                && message.content.contains(r#""ok":true"#)
                && message.content.contains(r#""staged":1"#)
                && message.content.contains(r#""applied":0"#)
        })
    }

    fn model_request_contains_ns_patch_failure(
        messages: &[ploke_tui::llm::RequestMessage],
        call_id: &str,
    ) -> bool {
        messages.iter().any(|message| {
            message.role == ploke_llm::manager::Role::Tool
                && message
                    .tool_call_id
                    .as_ref()
                    .is_some_and(|tool_call_id| tool_call_id.as_ref() == call_id)
                && message.content.contains(r#""ok":false"#)
                && message.content.contains(r#""tool":"non_semantic_patch""#)
        })
    }

    fn model_request_contains_tool_rejection(
        messages: &[ploke_tui::llm::RequestMessage],
        call_id: &str,
    ) -> bool {
        messages.iter().any(|message| {
            message.role == ploke_llm::manager::Role::Tool
                && message
                    .tool_call_id
                    .as_ref()
                    .is_some_and(|tool_call_id| tool_call_id.as_ref() == call_id)
                && message.content.contains(r#""ok":false"#)
                && message.content.contains(r#""tool":"non_semantic_patch""#)
                && message.content.contains("protected")
        })
    }

    fn model_request_contains_applied_success(
        messages: &[ploke_tui::llm::RequestMessage],
        call_id: &str,
    ) -> bool {
        messages.iter().any(|message| {
            message.role == ploke_llm::manager::Role::Tool
                && message
                    .tool_call_id
                    .as_ref()
                    .is_some_and(|tool_call_id| tool_call_id.as_ref() == call_id)
                && message.content.contains(r#""ok":true"#)
                && message.content.contains(r#""applied":1"#)
        })
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
                    if notice.starts_with("Context mode is Off:")
                        && notice.contains("request_code_context is still available")
                        && notice.contains("workspace loaded ")
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
                    token_budget_per_result: Some(300),
                    token_budget_total: Some(1_200),
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
                    if notice.starts_with("Context mode is Off:")
                        && notice.contains("request_code_context is still available")
                        && notice.contains("workspace loaded ")
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
