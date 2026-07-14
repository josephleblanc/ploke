//! Request and response DTOs for the typestate walk socket protocol.
//!
//! The public application contract uses serde JSON over explicit frame lengths.
//! CLI and native UI clients consume these exact carriers; transition authority
//! remains behind the server-side controller and durable session boundary.

use std::{fmt, path::PathBuf, str::FromStr};

use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cli::{
    Prototype1StateWalkAuditScope, Prototype1StateWalkAuditTransition,
    Prototype1StateWalkLlmStepSource,
    prototype1_state::{
        driver::control::RecoveryDirective,
        edge::ControlEdge,
        session::{Cursor, SessionId},
    },
};

use super::{audit::WalkAuditReport, epoch::ServerEpoch, phase::WalkPhase, query::DbQueryResult};

/// Serializable identity needed to attach to a setup-derived session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkStartConfig {
    pub campaign: Option<CampaignId>,
    pub repo_root: Option<PathBuf>,
}

/// Client-selected identity for one semantic walk mutation.
///
/// This is distinct from the server-local numeric job id and from typestate
/// transition ids. A client may deliberately reuse it to attach to the exact
/// same accepted request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationId(Uuid);

impl OperationId {
    pub fn new() -> Self {
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
pub struct SessionVersion {
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

    pub fn phase(&self) -> WalkPhase {
        self.cursor
            .as_ref()
            .map_or(WalkPhase::Empty, |cursor| cursor.phase)
    }

    pub fn session_id(&self) -> Option<SessionId> {
        self.session_id
    }

    pub fn cursor(&self) -> Option<&Cursor> {
        self.cursor.as_ref()
    }

    pub const fn journal_revision(&self) -> usize {
        self.journal_revision
    }
}

/// Admission envelope for a live socket mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationGuard {
    pub operation: OperationId,
    pub expected: SessionVersion,
}

/// One framed client-to-server request.
///
/// Mutating requests include a freshly captured `client_epoch`; read-only
/// requests may omit it so stale servers remain inspectable. `Stop` also binds
/// shutdown to the caller's selected repository rather than trusting an
/// explicitly supplied socket as repository authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkRequest {
    pub client_epoch: Option<ServerEpoch>,
    pub body: WalkRequestBody,
}

/// Operation requested over the walk socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WalkRequestBody {
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
    /// Explicitly resolve one indeterminate supervised operation.
    ResolveJob {
        /// Target operation identity plus the currently observed session version.
        guard: MutationGuard,
        resolution: WalkJobResolutionKind,
    },
    /// Print tracked output files produced or touched by the current walk.
    Files,
    /// Run an immutable expert query against one exact owner-DB snapshot.
    DbQuery {
        /// Campaign id. Defaults to the selected parent identity.
        campaign: Option<CampaignId>,
        /// Immutable CozoScript query.
        script: String,
    },
    /// Inspect current phase and summary without mutating state.
    Show,
    /// Inspect one exact supervised operation, including terminal history.
    OperationStatus { operation: OperationId },
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
        /// Idempotency and exact durable-session admission guard.
        guard: MutationGuard,
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
        /// Idempotency and exact durable-session admission guard.
        guard: MutationGuard,
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
        /// Idempotency and exact durable-session admission guard.
        guard: MutationGuard,
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
pub enum WalkJobStatus {
    Running,
    Succeeded,
    Failed,
    CancelRequested,
    Cancelled,
    /// Effects may have occurred, so another mutation must not be admitted.
    Indeterminate,
    /// An operator preserved the uncertainty and explicitly abandoned retry.
    Abandoned,
}

impl WalkJobStatus {
    /// True while a duplicate live mutation must not be admitted.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::CancelRequested)
    }

    /// True while admitting another effectful operation would be unsafe.
    pub fn blocks_mutation(self) -> bool {
        self.is_active() || self == Self::Indeterminate
    }
}

/// Explicit operator decision for an indeterminate supervised operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkJobResolutionKind {
    Abandon,
}

/// Durable evidence that an operator resolved an indeterminate job blocker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkJobResolutionReceipt {
    pub kind: WalkJobResolutionKind,
    pub observed: SessionVersion,
    pub resolved_at: String,
}

/// Closed identity of a supervised mutating operation.
///
/// The snake-case wire representation intentionally matches the command
/// strings written by protocol-v5 durable operation records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkJobKind {
    Start,
    Step,
    LlmStep,
    LlmFinish,
    BranchLive,
    Reset,
    Recover,
}

impl WalkJobKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Step => "step",
            Self::LlmStep => "llm_step",
            Self::LlmFinish => "llm_finish",
            Self::BranchLive => "branch_live",
            Self::Reset => "reset",
            Self::Recover => "recover",
        }
    }
}

impl fmt::Display for WalkJobKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Observable state for the server's one supervised live walk job.
///
/// The server intentionally permits only one active job at a time because there
/// is one `WalkController`, one active checkout, and one in-memory typestate
/// position. Parallel mutation requests would otherwise race the parent loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkJobSnapshot {
    pub job_id: u64,
    pub operation_id: OperationId,
    pub expected: SessionVersion,
    pub command: WalkJobKind,
    pub status: WalkJobStatus,
    pub phase_before: WalkPhase,
    pub phase_after: Option<WalkPhase>,
    pub target_phase: Option<WalkPhase>,
    pub watch: Option<bool>,
    pub allow_live_api: Option<bool>,
    pub allow_git_changes: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_source: Option<Prototype1StateWalkLlmStepSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_workspace_mutation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_provenance_record: Option<bool>,
    pub started_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
    pub message: Option<String>,
    /// Exact durable transition result for an outer typestate job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<WalkTransitionReceipt>,
    /// Explicit operator resolution of an earlier indeterminate outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<WalkJobResolutionReceipt>,
}

/// Typed outer-loop transition receipt attached to a terminal supervised job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkTransitionReceipt {
    pub phase_before: WalkPhase,
    pub phase_after: WalkPhase,
    pub edges: Vec<ControlEdge>,
    /// Durable session position observed after the transition committed.
    pub version: SessionVersion,
    /// Typed outcome of projecting this receipt into the owner database.
    #[serde(default)]
    pub event_projection: WalkEventProjection,
}

/// Owner-database projection outcome for one committed walk transition.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum WalkEventProjection {
    /// Compatibility value for receipts written before projection was typed.
    #[default]
    Unknown,
    Recorded,
    NotApplicable {
        detail: String,
    },
    Failed {
        detail: String,
    },
}

/// Current ability of this server to admit a mutating operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkAuthority {
    Active,
    TransferPending,
    JobActive,
    RecoveryRequired,
    Abandoned,
    Stopping,
}

/// Stable blocker classes used by sibling clients instead of message parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkBlockerCode {
    TransferPending,
    JobActive,
    JobIndeterminate,
    JournalDamaged,
    AttemptPending,
    AttemptIndeterminate,
    ControllerBlocked,
    SessionAbandoned,
    ServerStopping,
}

/// Active mutation blocker with operator-facing detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkBlocker {
    pub code: WalkBlockerCode,
    pub detail: String,
}

/// Closed action vocabulary rendered by the CLI and native UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkActionKind {
    Inspect,
    Query,
    Start,
    Step,
    Reset,
    Recover,
    Stop,
}

/// One possible action and the exact gate that currently admits or blocks it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkAction {
    pub kind: WalkActionKind,
    pub edge: Option<ControlEdge>,
    pub target: Option<WalkPhase>,
    pub enabled: bool,
    pub requires_live_api: bool,
    pub requires_git_changes: bool,
    pub blocker: Option<WalkBlockerCode>,
}

/// Nonblocking, structured read model shared by every walk client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkSessionSnapshot {
    pub phase: WalkPhase,
    pub version: SessionVersion,
    /// Whether this server currently carries the reconstructed typed value.
    pub controller_attached: bool,
    pub authority: WalkAuthority,
    pub job: Option<WalkJobSnapshot>,
    pub blocker: Option<WalkBlocker>,
    pub actions: Vec<WalkAction>,
}

/// Complete immutable-query observation returned to every sibling client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkQuerySnapshot {
    pub phase: WalkPhase,
    pub result: DbQueryResult,
    pub version: SessionVersion,
    pub epoch: ServerEpoch,
}

/// Stable application error classes carried by every walk client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkErrorCode {
    BadRequest,
    JobActive,
    OperationConflict,
    OperationRestart,
    RecoveryInProgress,
    RequestFailed,
    ServerStopping,
    StaleVersion,
    TransferPending,
}

impl fmt::Display for WalkErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadRequest => "bad_request",
            Self::JobActive => "job_active",
            Self::OperationConflict => "operation_conflict",
            Self::OperationRestart => "operation_restart",
            Self::RecoveryInProgress => "recovery_in_progress",
            Self::RequestFailed => "request_failed",
            Self::ServerStopping => "server_stopping",
            Self::StaleVersion => "stale_version",
            Self::TransferPending => "transfer_pending",
        })
    }
}

/// Operation identity for successful read-only or local cursor responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkOkKind {
    Files,
    LlmBack,
    LlmFocus,
    LlmForward,
    LlmHead,
    LlmLanes,
    LlmPrompt,
    LlmProtocol,
    LlmShow,
    LlmTimeline,
    LlmTool,
    RecoverInspect,
    Replay,
    ReplayBack,
    ReplayForward,
    Show,
    ShowDelta,
}

impl WalkOkKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::LlmBack => "llm_back",
            Self::LlmFocus => "llm_focus",
            Self::LlmForward => "llm_forward",
            Self::LlmHead => "llm_head",
            Self::LlmLanes => "llm_lanes",
            Self::LlmPrompt => "llm_prompt",
            Self::LlmProtocol => "llm_protocol",
            Self::LlmShow => "llm_show",
            Self::LlmTimeline => "llm_timeline",
            Self::LlmTool => "llm_tool",
            Self::RecoverInspect => "recover_inspect",
            Self::Replay => "replay",
            Self::ReplayBack => "replay_back",
            Self::ReplayForward => "replay_forward",
            Self::Show => "show",
            Self::ShowDelta => "show_delta",
        }
    }
}

/// Operation-specific payload for successful local/read-only walk commands.
///
/// These reports remain human-oriented until the Stage 4 read models replace
/// each report with its normalized carrier. The closed variant set prevents a
/// client from inferring which result it received from prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WalkOkPayload {
    Files { report: String },
    LlmBack { receipt: String },
    LlmFocus { receipt: String },
    LlmForward { receipt: String },
    LlmHead { receipt: String },
    LlmLanes { report: String },
    LlmPrompt { report: String },
    LlmProtocol { report: String },
    LlmShow { report: String },
    LlmTimeline { report: String },
    LlmTool { report: String },
    RecoverInspect { report: String },
    Replay { snapshot: String },
    ReplayBack { snapshot: String },
    ReplayForward { snapshot: String },
    Show { report: String },
    ShowDelta { report: String },
}

impl WalkOkPayload {
    fn from_parts(kind: WalkOkKind, text: String) -> Self {
        match kind {
            WalkOkKind::Files => Self::Files { report: text },
            WalkOkKind::LlmBack => Self::LlmBack { receipt: text },
            WalkOkKind::LlmFocus => Self::LlmFocus { receipt: text },
            WalkOkKind::LlmForward => Self::LlmForward { receipt: text },
            WalkOkKind::LlmHead => Self::LlmHead { receipt: text },
            WalkOkKind::LlmLanes => Self::LlmLanes { report: text },
            WalkOkKind::LlmPrompt => Self::LlmPrompt { report: text },
            WalkOkKind::LlmProtocol => Self::LlmProtocol { report: text },
            WalkOkKind::LlmShow => Self::LlmShow { report: text },
            WalkOkKind::LlmTimeline => Self::LlmTimeline { report: text },
            WalkOkKind::LlmTool => Self::LlmTool { report: text },
            WalkOkKind::RecoverInspect => Self::RecoverInspect { report: text },
            WalkOkKind::Replay => Self::Replay { snapshot: text },
            WalkOkKind::ReplayBack => Self::ReplayBack { snapshot: text },
            WalkOkKind::ReplayForward => Self::ReplayForward { snapshot: text },
            WalkOkKind::Show => Self::Show { report: text },
            WalkOkKind::ShowDelta => Self::ShowDelta { report: text },
        }
    }

    pub const fn kind(&self) -> WalkOkKind {
        match self {
            Self::Files { .. } => WalkOkKind::Files,
            Self::LlmBack { .. } => WalkOkKind::LlmBack,
            Self::LlmFocus { .. } => WalkOkKind::LlmFocus,
            Self::LlmForward { .. } => WalkOkKind::LlmForward,
            Self::LlmHead { .. } => WalkOkKind::LlmHead,
            Self::LlmLanes { .. } => WalkOkKind::LlmLanes,
            Self::LlmPrompt { .. } => WalkOkKind::LlmPrompt,
            Self::LlmProtocol { .. } => WalkOkKind::LlmProtocol,
            Self::LlmShow { .. } => WalkOkKind::LlmShow,
            Self::LlmTimeline { .. } => WalkOkKind::LlmTimeline,
            Self::LlmTool { .. } => WalkOkKind::LlmTool,
            Self::RecoverInspect { .. } => WalkOkKind::RecoverInspect,
            Self::Replay { .. } => WalkOkKind::Replay,
            Self::ReplayBack { .. } => WalkOkKind::ReplayBack,
            Self::ReplayForward { .. } => WalkOkKind::ReplayForward,
            Self::Show { .. } => WalkOkKind::Show,
            Self::ShowDelta { .. } => WalkOkKind::ShowDelta,
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Self::Files { report }
            | Self::LlmLanes { report }
            | Self::LlmPrompt { report }
            | Self::LlmProtocol { report }
            | Self::LlmShow { report }
            | Self::LlmTimeline { report }
            | Self::LlmTool { report }
            | Self::RecoverInspect { report }
            | Self::Show { report }
            | Self::ShowDelta { report } => report,
            Self::LlmBack { receipt }
            | Self::LlmFocus { receipt }
            | Self::LlmForward { receipt }
            | Self::LlmHead { receipt } => receipt,
            Self::Replay { snapshot }
            | Self::ReplayBack { snapshot }
            | Self::ReplayForward { snapshot } => snapshot,
        }
    }
}

/// One framed server-to-client response.
///
/// Every response carries the server epoch so clients and humans can see which
/// binary/source snapshot is holding the in-memory state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WalkResponse {
    /// Successful request result.
    Ok {
        /// Current phase after the request.
        phase: WalkPhase,
        /// Closed operation-specific result; prose is never its discriminator.
        result: WalkOkPayload,
        /// Server freshness identity.
        epoch: ServerEpoch,
    },
    /// Revision-tagged immutable owner database query.
    Query {
        /// Rows plus the controller and server revisions observed with them.
        query: WalkQuerySnapshot,
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
        /// Human-readable summary for table output.
        message: String,
        /// Exact structured session state observed without waiting on the controller.
        snapshot: WalkSessionSnapshot,
        /// Server freshness identity.
        epoch: ServerEpoch,
    },
    /// Failed request result.
    Error {
        /// Stable-ish error class for clients.
        code: WalkErrorCode,
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
    pub(crate) fn ok(
        kind: WalkOkKind,
        phase: WalkPhase,
        message: impl Into<String>,
        epoch: ServerEpoch,
    ) -> Self {
        Self::Ok {
            phase,
            result: WalkOkPayload::from_parts(kind, message.into()),
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

    /// Build a revision-tagged immutable database query response.
    pub(crate) fn query(
        phase: WalkPhase,
        result: DbQueryResult,
        version: SessionVersion,
        epoch: ServerEpoch,
    ) -> Self {
        Self::Query {
            query: WalkQuerySnapshot {
                phase,
                result,
                version,
                epoch,
            },
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
        snapshot: WalkSessionSnapshot,
        message: impl Into<String>,
        epoch: ServerEpoch,
    ) -> Self {
        Self::Status {
            message: message.into(),
            snapshot,
            epoch,
        }
    }

    /// Build an error response with optional current phase.
    pub(crate) fn error(
        code: WalkErrorCode,
        detail: impl Into<String>,
        phase: Option<WalkPhase>,
        epoch: ServerEpoch,
    ) -> Self {
        Self::Error {
            code,
            detail: detail.into(),
            phase,
            version: None,
            epoch,
        }
    }

    /// Build a typed admission conflict with the actual durable session version.
    pub(crate) fn conflict(
        code: WalkErrorCode,
        detail: impl Into<String>,
        version: SessionVersion,
        epoch: ServerEpoch,
    ) -> Self {
        Self::Error {
            code,
            detail: detail.into(),
            phase: Some(version.phase()),
            version: Some(version),
            epoch,
        }
    }

    /// Return the response phase, if one was available.
    pub fn phase(&self) -> Option<WalkPhase> {
        match self {
            WalkResponse::Ok { phase, .. }
            | WalkResponse::Audit { phase, .. }
            | WalkResponse::Job { phase, .. } => Some(*phase),
            WalkResponse::Query { query } => Some(query.phase),
            WalkResponse::Status { snapshot, .. } => Some(snapshot.phase),
            WalkResponse::Error { phase, .. } => *phase,
        }
    }

    /// Exact server/binary identity that produced this response.
    pub fn epoch(&self) -> &ServerEpoch {
        match self {
            WalkResponse::Ok { epoch, .. }
            | WalkResponse::Audit { epoch, .. }
            | WalkResponse::Job { epoch, .. }
            | WalkResponse::Status { epoch, .. }
            | WalkResponse::Error { epoch, .. } => epoch,
            WalkResponse::Query { query } => &query.epoch,
        }
    }

    /// Return whether this response is `Ok`.
    pub fn is_ok(&self) -> bool {
        matches!(
            self,
            WalkResponse::Ok { .. }
                | WalkResponse::Audit { .. }
                | WalkResponse::Query { .. }
                | WalkResponse::Job { .. }
                | WalkResponse::Status { .. }
        )
    }
}
