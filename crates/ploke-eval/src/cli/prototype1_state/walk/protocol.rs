//! Request and response DTOs for the typestate walk socket protocol.
//!
//! The protocol is private to `ploke-eval` and intentionally uses serde JSON
//! over explicit frame lengths. It mirrors the current CLI command surface just
//! enough for the server to construct the real `Prototype1StateCommand` before
//! entering `R0`.

use std::{fmt, path::PathBuf, str::FromStr};

use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cli::{
    Prototype1StateWalkAuditScope, Prototype1StateWalkAuditTransition,
    Prototype1StateWalkLlmStepSource,
    prototype1_state::{
        driver::control::RecoveryDirective,
        session::{Cursor, SessionId},
    },
};

use super::{audit::WalkAuditReport, epoch::ServerEpoch, phase::WalkPhase};

/// Serializable identity needed to attach to a setup-derived session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WalkStartConfig {
    pub(crate) campaign: Option<CampaignId>,
    pub(crate) repo_root: Option<PathBuf>,
}

/// Client-selected identity for one semantic walk mutation.
///
/// This is distinct from the server-local numeric job id and from typestate
/// transition ids. A client may deliberately reuse it to attach to the exact
/// same accepted request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct OperationId(Uuid);

impl OperationId {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[cfg(test)]
    pub(crate) const fn for_test(value: u128) -> Self {
        Self(Uuid::from_u128(value))
    }
}

impl fmt::Display for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for OperationId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

/// Exact durable controller-session position observed by a client.
///
/// `Cursor` already owns the phase/evidence relationship, so the socket
/// protocol does not flatten or duplicate those fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SessionVersion {
    pub(crate) session_id: Option<SessionId>,
    pub(crate) cursor: Option<Cursor>,
    pub(crate) journal_revision: usize,
}

impl SessionVersion {
    pub(crate) const fn empty() -> Self {
        Self {
            session_id: None,
            cursor: None,
            journal_revision: 0,
        }
    }

    pub(crate) fn phase(&self) -> WalkPhase {
        self.cursor
            .as_ref()
            .map_or(WalkPhase::Empty, |cursor| cursor.phase)
    }
}

/// Admission envelope for a live socket mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MutationGuard {
    pub(crate) operation: OperationId,
    pub(crate) expected: SessionVersion,
}

/// One framed client-to-server request.
///
/// Mutating requests include a freshly captured `client_epoch`; read-only
/// requests may omit it so stale servers remain inspectable. `Stop` also binds
/// shutdown to the caller's selected repository rather than trusting an
/// explicitly supplied socket as repository authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WalkRequest {
    pub(crate) client_epoch: Option<ServerEpoch>,
    pub(crate) body: WalkRequestBody,
}

/// Operation requested over the walk socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum WalkRequestBody {
    /// Probe liveness and receive the current phase without mutating state.
    Health,
    /// Attach to the durable controller session and optionally advance.
    Start {
        /// Idempotency and exact durable-session admission guard.
        guard: MutationGuard,
        /// Session coordinate selected by the client.
        config: WalkStartConfig,
        /// Phase to stop at after attaching.
        until: WalkPhase,
        /// Admit typestate edges that call a configured live provider.
        #[serde(default)]
        allow_live_api: bool,
    },
    /// Advance the existing in-memory walk.
    Step {
        /// Idempotency and exact durable-session admission guard.
        guard: MutationGuard,
        /// If present, advance repeatedly until this phase; otherwise one step.
        until: Option<WalkPhase>,
        /// Ask the short-lived client to follow the accepted server job.
        #[serde(default)]
        watch: bool,
        /// Admit typestate edges that call a configured live provider.
        #[serde(default)]
        allow_live_api: bool,
        /// Admit typed edges that mutate the active checkout during handoff.
        #[serde(default)]
        allow_git_changes: bool,
    },
    /// Clear the in-memory walk while keeping the server process alive.
    Reset {
        /// Idempotency and exact durable-session admission guard.
        guard: MutationGuard,
    },
    /// Inspect or explicitly resolve one exact durable recovery cause.
    Recover {
        /// Resolution to inspect or apply.
        directive: RecoveryDirective,
        /// Required for a mutating resolution and forbidden for inspection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        guard: Option<MutationGuard>,
    },
    /// Print tracked output files produced or touched by the current walk.
    Files,
    /// Inspect current phase and summary without mutating state.
    Show,
    /// Inspect only the last successful step delta.
    ShowDelta {
        /// Include changed axis values plus added/removed nested type structures.
        verbose: bool,
        /// Use ANSI colors in the human-readable message.
        color: bool,
    },
    /// Audit file/database persistence surfaces for a transition.
    Audit {
        /// Optional campaign id used like R0 command input; defaults to parent identity.
        campaign: Option<CampaignId>,
        /// Audit scope to run.
        scope: Prototype1StateWalkAuditScope,
        /// Optional transition checklist filter.
        transition: Option<Prototype1StateWalkAuditTransition>,
        /// Reconstruct and verify durable typestate before auditing.
        #[serde(default)]
        verify: bool,
        /// Include verbose table rendering details.
        #[serde(default)]
        verbose: bool,
        /// Include checklist note column in table output.
        #[serde(default)]
        with_note: bool,
    },
    /// List nested LLM/tool-loop fanout lanes without mutating state.
    LlmLanes {
        /// Include workspace and session ids for each lane.
        verbose: bool,
    },
    /// Set the default nested LLM/tool-loop lane focus.
    LlmFocus {
        /// Lane id, usually the candidate workspace basename.
        lane: String,
    },
    /// Inspect nested LLM/tool-loop debugger checkpoints without mutating state.
    LlmShow {
        /// Specific checkpoint session id. Defaults to selected lane/latest session.
        session_id: Option<String>,
        /// Lane id. Defaults to current focus.
        lane: Option<String>,
        /// Inspect the latest head rather than the read-only cursor.
        head: bool,
        /// Specific provider-response step. Defaults to lane cursor or latest recorded step.
        step: Option<usize>,
    },
    /// Show a compact chronological summary of nested LLM/tool-loop checkpoints.
    LlmTimeline {
        /// Specific checkpoint session id. Defaults to selected lane/latest session.
        session_id: Option<String>,
        /// Lane id. Defaults to current focus.
        lane: Option<String>,
    },
    /// Inspect persisted request messages sent to a nested LLM/tool-loop step.
    LlmPrompt {
        /// Specific checkpoint session id. Defaults to selected lane/latest session.
        session_id: Option<String>,
        /// Lane id. Defaults to current focus.
        lane: Option<String>,
        /// Response step whose request messages should be inspected. Defaults to 0.
        step: Option<usize>,
        /// Optional message-role filter.
        role: Option<String>,
        /// Optional zero-based message index.
        message: Option<usize>,
        /// Show complete message content instead of a bounded preview.
        full: bool,
        /// Print JSON instead of human-readable text.
        json: bool,
    },
    /// Inspect persisted protocol review artifacts for a nested LLM/tool-loop session.
    LlmProtocol {
        /// Specific checkpoint session id. Defaults to selected lane/latest session.
        session_id: Option<String>,
        /// Lane id. Defaults to current focus.
        lane: Option<String>,
        /// Print JSON instead of human-readable text.
        json: bool,
    },
    /// Inspect a nested LLM/tool-loop tool definition and selected call arguments.
    LlmTool {
        /// Specific checkpoint session id. Defaults to selected lane/latest session.
        session_id: Option<String>,
        /// Lane id. Defaults to current focus.
        lane: Option<String>,
        /// Inspect the latest head rather than the read-only cursor.
        head: bool,
        /// Specific provider-response step. Defaults to lane cursor or latest recorded step.
        step: Option<usize>,
        /// One-based tool call index within the selected step.
        call: Option<usize>,
        /// Tool name to inspect.
        name: Option<String>,
        /// Print a JSON payload instead of human-readable text.
        json: bool,
    },
    /// Execute one historical or live provider response step through current tools.
    LlmStep {
        /// Specific checkpoint session id. Defaults to selected lane/latest session.
        session_id: Option<String>,
        /// Lane id. Defaults to current focus.
        lane: Option<String>,
        /// Response step index. Historical mode replays this response; live mode continues after it.
        step: Option<usize>,
        /// Step source.
        source: Prototype1StateWalkLlmStepSource,
        /// Required for live provider calls.
        watch: bool,
        /// Required because current TUI tools may mutate the candidate workspace.
        allow_workspace_mutation: bool,
        /// Optional model override for live steps.
        model_id: Option<String>,
        /// Optional provider override for live steps.
        provider: Option<String>,
        /// Maximum attempts for the one-step headless runtime.
        max_attempts: u32,
        /// Timeout seconds for the one-step headless runtime.
        timeout_secs: u64,
    },
    /// Continue live provider response steps until terminal or max steps.
    LlmFinish {
        /// Specific checkpoint session id. Defaults to selected lane/latest session.
        session_id: Option<String>,
        /// Lane id. Defaults to current focus.
        lane: Option<String>,
        /// Response step to continue after. Defaults to lane cursor/head.
        step: Option<usize>,
        /// Required for live provider calls.
        watch: bool,
        /// Required because current TUI tools may mutate the candidate workspace.
        allow_workspace_mutation: bool,
        /// Optional model override for live steps.
        model_id: Option<String>,
        /// Optional provider override for live steps.
        provider: Option<String>,
        /// Maximum live response steps.
        max_steps: usize,
        /// Maximum attempts for each one-step headless runtime.
        max_attempts: u32,
        /// Timeout seconds for each one-step headless runtime.
        timeout_secs: u64,
    },
    /// Move the nested LLM/tool-loop lane cursor backward without mutating state.
    LlmBack {
        /// Lane id. Defaults to current focus.
        lane: Option<String>,
        /// Number of recorded steps to move.
        steps: usize,
    },
    /// Move the nested LLM/tool-loop lane cursor forward without mutating state.
    LlmForward {
        /// Lane id. Defaults to current focus.
        lane: Option<String>,
        /// Number of recorded steps to move.
        steps: usize,
    },
    /// Move the nested LLM/tool-loop lane cursor to the latest checkpoint head.
    LlmHead {
        /// Lane id. Defaults to current focus.
        lane: Option<String>,
    },
    /// Show or position the read-only historical replay cursor.
    Replay {
        /// Optional absolute journal entry index to select.
        index: Option<usize>,
        /// Number of trailing entries to render.
        tail: usize,
    },
    /// Move the read-only historical replay cursor backward.
    ReplayBack {
        /// Number of entries to move.
        steps: usize,
        /// Number of trailing entries to render.
        tail: usize,
    },
    /// Move the read-only historical replay cursor forward.
    ReplayForward {
        /// Number of entries to move.
        steps: usize,
        /// Number of trailing entries to render.
        tail: usize,
    },
    /// Record explicit provenance for leaving historical replay toward live work.
    BranchLive {
        /// Operator-supplied reason for leaving read-only replay.
        reason: String,
        /// Explicit admission that this request writes a provenance record.
        allow_provenance_record: bool,
    },
    /// Ask the server to reply and then exit.
    Stop,
}

/// Server-side lifecycle for a live walk command submitted as a background job.
///
/// `start` and `step` can take minutes because they may build child binaries,
/// spawn child runtimes, wait for LLM/tool loops, or perform successor handoff.
/// The socket protocol reports that work as a job so status/health requests can
/// remain non-blocking while the controller owns the actual typestate edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WalkJobStatus {
    Running,
    Succeeded,
    Failed,
    CancelRequested,
    Cancelled,
}

impl WalkJobStatus {
    /// True while a duplicate live mutation must not be admitted.
    pub(crate) fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::CancelRequested)
    }
}

/// Observable state for the server's one supervised live walk job.
///
/// The server intentionally permits only one active job at a time because there
/// is one `WalkController`, one active checkout, and one in-memory typestate
/// position. Parallel mutation requests would otherwise race the parent loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WalkJobSnapshot {
    pub(crate) job_id: u64,
    pub(crate) operation_id: OperationId,
    pub(crate) expected: SessionVersion,
    pub(crate) command: String,
    pub(crate) status: WalkJobStatus,
    pub(crate) phase_before: WalkPhase,
    pub(crate) phase_after: Option<WalkPhase>,
    pub(crate) target_phase: Option<WalkPhase>,
    pub(crate) watch: Option<bool>,
    pub(crate) allow_live_api: Option<bool>,
    pub(crate) allow_git_changes: Option<bool>,
    pub(crate) started_at: String,
    pub(crate) updated_at: String,
    pub(crate) finished_at: Option<String>,
    pub(crate) message: Option<String>,
}

/// One framed server-to-client response.
///
/// Every response carries the server epoch so clients and humans can see which
/// binary/source snapshot is holding the in-memory state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum WalkResponse {
    /// Successful request result.
    Ok {
        /// Current phase after the request.
        phase: WalkPhase,
        /// Human-readable summary for table output and debugging.
        message: String,
        /// Server freshness identity.
        epoch: ServerEpoch,
    },
    /// Structured audit result.
    Audit {
        /// Current phase after the request.
        phase: WalkPhase,
        /// Read-only audit payload.
        report: WalkAuditReport,
        /// Server freshness identity.
        epoch: ServerEpoch,
    },
    /// Accepted or duplicate-suppressed background job.
    Job {
        /// Best known phase while the job is active or after it has completed.
        phase: WalkPhase,
        /// Machine-readable live command status.
        job: WalkJobSnapshot,
        /// Human-readable explanation for table output.
        message: String,
        /// Server freshness identity.
        epoch: ServerEpoch,
    },
    /// Non-mutating status/health snapshot.
    Status {
        /// Best known current phase.
        phase: WalkPhase,
        /// Human-readable summary for table output.
        message: String,
        /// Active or most recent live command job, if one exists.
        job: Option<WalkJobSnapshot>,
        /// Exact durable controller-session position read without controller ownership.
        version: SessionVersion,
        /// Server freshness identity.
        epoch: ServerEpoch,
    },
    /// Failed request result.
    Error {
        /// Stable-ish error class for clients.
        code: String,
        /// Human-readable error detail.
        detail: String,
        /// Best known phase when the error was produced.
        phase: Option<WalkPhase>,
        /// Current durable session position for typed admission conflicts.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<SessionVersion>,
        /// Server freshness identity.
        epoch: ServerEpoch,
    },
}

impl WalkResponse {
    /// Build a successful response at `phase`.
    pub(crate) fn ok(phase: WalkPhase, message: impl Into<String>, epoch: ServerEpoch) -> Self {
        Self::Ok {
            phase,
            message: message.into(),
            epoch,
        }
    }

    /// Build a structured audit response at `phase`.
    pub(crate) fn audit(phase: WalkPhase, report: WalkAuditReport, epoch: ServerEpoch) -> Self {
        Self::Audit {
            phase,
            report,
            epoch,
        }
    }

    /// Build a job response at the job's best known phase.
    pub(crate) fn job(
        phase: WalkPhase,
        job: WalkJobSnapshot,
        message: impl Into<String>,
        epoch: ServerEpoch,
    ) -> Self {
        Self::Job {
            phase,
            job,
            message: message.into(),
            epoch,
        }
    }

    /// Build a non-mutating status response.
    pub(crate) fn status(
        phase: WalkPhase,
        message: impl Into<String>,
        job: Option<WalkJobSnapshot>,
        version: SessionVersion,
        epoch: ServerEpoch,
    ) -> Self {
        Self::Status {
            phase,
            message: message.into(),
            job,
            version,
            epoch,
        }
    }

    /// Build an error response with optional current phase.
    pub(crate) fn error(
        code: impl Into<String>,
        detail: impl Into<String>,
        phase: Option<WalkPhase>,
        epoch: ServerEpoch,
    ) -> Self {
        Self::Error {
            code: code.into(),
            detail: detail.into(),
            phase,
            version: None,
            epoch,
        }
    }

    /// Build a typed admission conflict with the actual durable session version.
    pub(crate) fn conflict(
        code: impl Into<String>,
        detail: impl Into<String>,
        version: SessionVersion,
        epoch: ServerEpoch,
    ) -> Self {
        Self::Error {
            code: code.into(),
            detail: detail.into(),
            phase: Some(version.phase()),
            version: Some(version),
            epoch,
        }
    }

    /// Return the response phase, if one was available.
    pub(crate) fn phase(&self) -> Option<WalkPhase> {
        match self {
            WalkResponse::Ok { phase, .. }
            | WalkResponse::Audit { phase, .. }
            | WalkResponse::Job { phase, .. }
            | WalkResponse::Status { phase, .. } => Some(*phase),
            WalkResponse::Error { phase, .. } => *phase,
        }
    }

    /// Return whether this response is `Ok`.
    pub(crate) fn is_ok(&self) -> bool {
        matches!(
            self,
            WalkResponse::Ok { .. }
                | WalkResponse::Audit { .. }
                | WalkResponse::Job { .. }
                | WalkResponse::Status { .. }
        )
    }
}
