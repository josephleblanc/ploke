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

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use super::{
    ArtifactDelta,
    harness_request::{BroadEditPolicy, EvidenceRoot, request},
    surface, tui,
};

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
/// `ploke-tui`'s test-harness app runtime, observes tool/proposal events, and
/// approves only proposals whose paths stay within the broad prototype surface.
/// `ploke-eval` still validates the resulting workspace diff before admission.
pub(crate) async fn run_headless(
    workspace_path: &Path,
    prompt: &str,
    budget: Budget,
    edit_policy: BroadEditPolicy,
) -> Result<HeadlessRun, Error> {
    use ploke_tui::{
        AppEvent, EventPriority,
        app_state::{StateCommand, core::EditProposalStatus, events::SystemEvent},
        test_utils::new_test_harness::AppHarness,
    };

    let harness = AppHarness::spawn()
        .await
        .map_err(|source| Error::HeadlessStart(source.to_string()))?;
    let mut events = harness.event_bus.subscribe(EventPriority::Realtime);

    harness
        .state
        .system
        .set_pwd_for_test(workspace_path.to_path_buf())
        .await;
    harness
        .state
        .with_system_raw(|system| {
            system.set_loaded_workspace(
                workspace_path.to_path_buf(),
                vec![workspace_path.to_path_buf()],
                Some(workspace_path.to_path_buf()),
            );
        })
        .await;

    harness
        .cmd_tx
        .send(StateCommand::SetEditingAutoConfirm { enabled: false })
        .await
        .map_err(|source| Error::HeadlessEvent(format!("state command send failed: {source}")))?;

    let mut run = HeadlessRun::new();
    let mut turn = 1_u32;
    let mut pending_retry = None::<String>;
    harness.add_user_msg(prompt.to_string()).await;

    let outcome = tokio::time::timeout(Duration::from_secs(budget.timeout_secs()), async {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(source) => {
                    return Err(Error::HeadlessEvent(source.to_string()));
                }
            };

            match event {
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
                        let Some(proposal) = harness.state.proposals.read().await.get(&proposal_id).cloned() else {
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
                            harness
                                .cmd_tx
                                .send(StateCommand::DenyEdits { proposal_id })
                                .await
                                .map_err(|source| {
                                    Error::HeadlessEvent(format!(
                                        "state command send failed: {source}"
                                    ))
                                })?;
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

                        harness
                            .cmd_tx
                            .send(StateCommand::ApproveEdits { proposal_id })
                            .await
                            .map_err(|source| {
                                Error::HeadlessEvent(format!("state command send failed: {source}"))
                            })?;

                        loop {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            let Some(updated) =
                                harness.state.proposals.read().await.get(&proposal_id).cloned()
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
                                    return Ok(HeadlessTerminal::Applied {
                                        proposal_id,
                                        request_id,
                                        changed_paths: paths,
                                    });
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
                        if !retry_turn(&harness, &budget, &mut turn, &feedback).await {
                            return Ok(HeadlessTerminal::Exhausted {
                                attempts: turn,
                                last: feedback,
                            });
                        }
                        continue;
                    }

                    let has_pending = harness
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
                        if !retry_turn(&harness, &budget, &mut turn, feedback).await {
                            return Ok(HeadlessTerminal::CompletedWithoutEdit {
                                outcome,
                                summary,
                            });
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
    harness.shutdown().await;
    run.terminal = Some(terminal);
    Ok(run)
}

async fn retry_turn(
    harness: &ploke_tui::test_utils::new_test_harness::AppHarness,
    budget: &Budget,
    turn: &mut u32,
    feedback: &str,
) -> bool {
    if *turn >= budget.max_attempts() {
        return false;
    }
    *turn += 1;
    let prompt = format!(
        "The previous edit attempt was rejected by ploke-eval boundary checks:\n\n{feedback}\n\nTry again. Keep the edit inside the allowed workspace surface and outside protected core. Produce a concrete patch."
    );
    harness.add_user_msg(prompt).await;
    true
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
    terminal: Option<HeadlessTerminal>,
}

impl HeadlessRun {
    fn new() -> Self {
        Self {
            attempts: Vec::new(),
            events: Vec::new(),
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

    pub(crate) fn evidence(&self) -> evidence::Summary {
        evidence::Summary::from(self)
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
    TimedOut {
        secs: u64,
    },
}

pub(crate) mod evidence {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::{HeadlessAttemptResult, HeadlessRun, HeadlessTerminal};

    /// Compact executor observations. Backend admission must still validate the
    /// workspace diff before any loop state advances.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Summary {
        pub(crate) attempts: Vec<Attempt>,
        pub(crate) terminal: Option<Terminal>,
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
        TimedOut {
            secs: u64,
        },
    }

    impl From<&HeadlessRun> for Summary {
        fn from(value: &HeadlessRun) -> Self {
            Self {
                attempts: value.attempts.iter().map(Attempt::from).collect(),
                terminal: value.terminal.as_ref().map(Terminal::from),
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
    use std::path::{Path, PathBuf};

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
