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

use ploke_tui::app::commands::harness::TestAppAccessor;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::{
    ArtifactDelta,
    harness_request::{BroadEditPolicy, EvidenceRoot, request},
    surface, tui,
};

const MAX_DEBUG_RELAY_EVENTS: usize = 128;
const MAX_DEBUG_RELAY_EVENT_CHARS: usize = 2_000;
const MAX_PROMPT_MESSAGE_PREVIEWS: usize = 8;
const MAX_PROMPT_MESSAGE_PREVIEW_CHARS: usize = 500;
const MAX_RAG_PART_PREVIEWS: usize = 8;

pub(crate) mod state {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Ready {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Running {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Done {}
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
) -> Result<HeadlessRun, Error> {
    use ploke_tui::{
        AppEvent,
        app_state::{StateCommand, core::EditProposalStatus, events::SystemEvent},
    };

    let mut runtime = crate::runner::setup_workspace_tui_runtime(workspace_path)
        .await
        .map_err(|source| Error::HeadlessStart(source.to_string()))?;

    let cmd_tx = runtime.app.state_cmd_tx();
    send_state(
        &cmd_tx,
        StateCommand::SetEditingAutoConfirm { enabled: false },
    )
    .await?;

    let mut run = HeadlessRun::new();
    let mut turn = 1_u32;
    let mut pending_retry = None::<String>;
    submit_prompt(&runtime.app, prompt.to_string()).await?;

    let outcome = tokio::time::timeout(Duration::from_secs(budget.timeout_secs()), async {
        loop {
            runtime.app.pump_pending_events().await;
            drain_debug(&mut runtime.debug_rx, &mut run);

            let event = next_event(&mut runtime).await?;

            match event {
                AppEvent::Llm(ploke_tui::llm::LlmEvent::ChatCompletion(
                    ploke_tui::llm::ChatEvt::PromptConstructed {
                        parent_id,
                        formatted_prompt,
                        context_plan,
                    },
                )) => {
                    let diagnostic = PromptDiagnostic::capture(
                        &runtime.state,
                        parent_id,
                        &formatted_prompt,
                        &context_plan,
                    )
                    .await;
                    let context_unavailable = diagnostic.context_unavailable_reason();
                    run.prompt_diagnostics.push(diagnostic);
                    if let Some(reason) = context_unavailable {
                        return Ok::<HeadlessTerminal, Error>(
                            HeadlessTerminal::ContextUnavailable { reason },
                        );
                    }
                }
                AppEvent::System(SystemEvent::ToolCallRequested {
                    request_id,
                    parent_id,
                    tool_call,
                }) => {
                    run.events.push(Event::ToolRequest {
                        request_id: request_id.to_string(),
                        parent_id: parent_id.to_string(),
                        call_id: tool_call.call_id.to_string(),
                        tool: tool_call.function.name.as_str().to_string(),
                        arguments: tool_call.function.arguments.clone(),
                    });
                }
                AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id,
                    call_id,
                    content,
                    ui_payload,
                    ..
                }) => {
                    run.events.push(Event::Tool {
                        call_id: call_id.to_string(),
                        result: Tool::Completed {
                            content: content.clone(),
                        },
                    });
                    if let Some(proposal_id) = ui_payload.and_then(|payload| payload.proposal_id) {
                        let Some(proposal) = runtime.state.proposals.read().await.get(&proposal_id).cloned() else {
                            continue;
                        };
                        let paths = proposal_paths(&proposal);
                        run.events.push(Event::Proposal {
                            id: proposal_id.to_string(),
                            edit_count: proposal.edits.len() + proposal.edits_ns.len(),
                            paths: paths.clone(),
                        });
                        if paths.is_empty() {
                            run.attempts.push(HeadlessAttempt {
                                turn,
                                proposal_id: Some(proposal_id),
                                result: HeadlessAttemptResult::Rejected {
                                    reason: Feedback::from_outcome(&Outcome::Rejected(
                                        Reject::Empty,
                                    ))
                                    .message()
                                    .to_string(),
                                },
                            });
                            pending_retry = Some(
                                "No material edit was staged; make a concrete bounded edit."
                                    .to_string(),
                            );
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
                            pending_retry = Some(feedback.message().to_string());
                            continue;
                        }

                        send_state(&cmd_tx, StateCommand::ApproveEdits { proposal_id }).await?;

                        loop {
                            runtime.app.pump_pending_events().await;
                            drain_debug(&mut runtime.debug_rx, &mut run);
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            let Some(updated) =
                                runtime.state.proposals.read().await.get(&proposal_id).cloned()
                            else {
                                continue;
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
                                    let terminal = HeadlessTerminal::Applied {
                                        proposal_id,
                                        request_id,
                                        changed_paths: paths,
                                    };
                                    return Ok::<HeadlessTerminal, Error>(terminal);
                                }
                                EditProposalStatus::Failed(reason)
                                | EditProposalStatus::Stale(reason) => {
                                    run.attempts.push(HeadlessAttempt {
                                        turn,
                                        proposal_id: Some(proposal_id),
                                        result: HeadlessAttemptResult::Rejected {
                                            reason: reason.clone(),
                                        },
                                    });
                                    pending_retry = Some(reason);
                                    break;
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
                                    pending_retry = Some(reason);
                                    break;
                                }
                                EditProposalStatus::Pending | EditProposalStatus::Approved => {}
                            }
                        }
                    }
                }
                AppEvent::System(SystemEvent::ToolCallFailed { call_id, error, .. }) => {
                    run.events.push(Event::Tool {
                        call_id: call_id.to_string(),
                        result: Tool::Failed {
                            error: error.clone(),
                        },
                    });
                    run.attempts.push(HeadlessAttempt {
                        turn,
                        proposal_id: None,
                        result: HeadlessAttemptResult::ToolFailed {
                            error: error.clone(),
                        },
                    });
                    pending_retry = Some(error);
                }
                AppEvent::System(SystemEvent::ChatTurnFinished {
                    request_id,
                    outcome,
                    attempts,
                    summary,
                    ..
                }) => {
                    run.events.push(Event::Turn {
                        request_id: request_id.to_string(),
                        outcome: outcome.clone(),
                        attempts,
                        summary: summary.clone(),
                    });
                    if let Some(feedback) = pending_retry.take() {
                        if !retry_turn(&runtime.app, &budget, &mut turn, &feedback).await? {
                            return Ok::<HeadlessTerminal, Error>(HeadlessTerminal::Exhausted {
                                attempts: turn,
                                last: feedback,
                            });
                        }
                        continue;
                    }

                    let has_pending = runtime
                        .state
                        .proposals
                        .read()
                        .await
                        .values()
                        .any(|proposal| {
                            matches!(
                                proposal.status,
                                EditProposalStatus::Pending | EditProposalStatus::Approved
                            )
                        });
                    if !has_pending {
                        let feedback = if summary.trim().is_empty() {
                            "The model returned without staging an edit; make a concrete bounded edit."
                        } else {
                            summary.as_str()
                        };
                        run.attempts.push(HeadlessAttempt {
                            turn,
                            proposal_id: None,
                            result: HeadlessAttemptResult::NoEdit {
                                summary: feedback.to_string(),
                            },
                        });
                        if !retry_turn(&runtime.app, &budget, &mut turn, feedback).await? {
                            return Ok::<HeadlessTerminal, Error>(
                                HeadlessTerminal::CompletedWithoutEdit { outcome, summary },
                            );
                        }
                    }
                }
                _ => {}
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
    run.terminal = Some(terminal);
    Ok(run)
}

async fn retry_turn(
    app: &ploke_tui::app::App,
    budget: &Budget,
    turn: &mut u32,
    feedback: &str,
) -> Result<bool, Error> {
    if *turn >= budget.max_attempts() {
        return Ok(false);
    }
    *turn += 1;
    let prompt = format!(
        "The previous edit attempt could not be applied:\n\n{feedback}\n\nTry again. Keep the edit inside the allowed workspace surface and outside protected core. Use the available edit tools to stage a concrete change."
    );
    submit_prompt(app, prompt).await?;
    Ok(true)
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
    TimedOut {
        secs: u64,
    },
}

pub(crate) mod evidence {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::{DebugRelay, HeadlessAttemptResult, HeadlessRun, HeadlessTerminal};

    /// Compact executor observations. Backend admission must still validate the
    /// workspace diff before any loop state advances.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Summary {
        pub(crate) attempts: Vec<Attempt>,
        pub(crate) terminal: Option<Terminal>,
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
        TimedOut {
            secs: u64,
        },
    }

    impl From<&HeadlessRun> for Summary {
        fn from(value: &HeadlessRun) -> Self {
            Self {
                attempts: value.attempts.iter().map(Attempt::from).collect(),
                terminal: value.terminal.as_ref().map(Terminal::from),
                debug_relay: DebugRelaySummary::from(&value.debug_relay),
                prompt_diagnostics: value.prompt_diagnostics.iter().map(Prompt::from).collect(),
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
        assert_prompt_diagnostics_show_loaded_context(&fixture, &run);
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
        assert_prompt_diagnostics_show_loaded_context(&fixture, &run);
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

    fn assert_prompt_diagnostics_show_loaded_context(
        fixture: &LiveCanaryFixture,
        run: &HeadlessRun,
    ) {
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
        assert!(
            diagnostic.fallback_notice.is_none(),
            "prompt unexpectedly fell back without code context: {:?}; artifacts at {}",
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
