//! Request and response DTOs for the typestate walk socket protocol.
//!
//! The public application contract uses serde JSON over explicit frame lengths.
//! CLI and native UI clients consume these exact carriers; transition authority
//! remains behind the server-side controller and durable session boundary.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::PathBuf,
    str::FromStr,
};

use ploke_records::{
    identity::ParentIdentityRecord,
    ids::{CampaignId, RuntimeId},
    invocation::ProcessIncarnation,
    run_profile::RunProfileCommitmentRecord,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cli::{
    Prototype1StateWalkAuditScope, Prototype1StateWalkAuditTransition,
    Prototype1StateWalkLlmStepSource,
    prototype1_state::{
        driver::control::RecoveryDirective,
        edge::ControlEdge,
        event::{ContentHash, TransitionId},
        session::{Cursor, SessionId},
        typestate::RuntimeAxisDelta,
    },
};

use super::{
    audit::WalkAuditReport,
    config::WalkConfigSnapshot,
    epoch::ServerEpoch,
    llm_trace::{LlmTraceCoordinate, LlmTraceIndex, LlmTraceSnapshot},
    phase::WalkPhase,
    query::DbQueryResult,
    trace::{EvaluationRunCoordinate, EvaluationTraceIndex, EvaluationTraceSnapshot},
};

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
#[serde(deny_unknown_fields)]
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
/// requests carry only `client_protocol` for schema negotiation without
/// granting mutation authority. `Stop` also binds shutdown to the caller's
/// selected repository rather than trusting an explicitly supplied socket as
/// repository authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkRequest {
    /// Lightweight schema negotiation for read-only responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_protocol: Option<u32>,
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
    /// Inspect the admitted campaign, run profile, and effective controller configuration.
    Config,
    /// List completed registered evaluation runs scoped to the admitted campaign.
    EvaluationTraceIndex,
    /// Inspect one exact registered evaluation run without reading mutable trace files.
    EvaluationTrace { coordinate: EvaluationRunCoordinate },
    /// List every persisted LLM debugger session without collapsing retries.
    LlmTraceIndex,
    /// Inspect one exact LLM debugger session and optional published step.
    LlmTrace { coordinate: LlmTraceCoordinate },
    /// Run an immutable expert query against one exact owner-DB snapshot.
    DbQuery {
        /// Campaign id. Defaults to the selected parent identity.
        campaign: Option<CampaignId>,
        /// Immutable CozoScript query.
        script: String,
    },
    /// Inspect current phase and summary without mutating state.
    Show,
    /// Inspect the durable controller journal as an ordered typed history.
    SessionHistory,
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

/// Closed authority-bearing position displayed in a session snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "source")]
pub enum WalkPosition {
    /// No durable controller cursor is available for the phase projection.
    NoSession,
    /// A reconstruction observed before durable session authority exists.
    Reconstruction { phase: WalkPhase },
    /// A durable controller session exists but has no committed cursor.
    Unpositioned { version: SessionVersion },
    /// The exact committed durable controller-session position.
    Session { version: SessionVersion },
    /// A status decoded from an older protocol that did not identify phase authority.
    Legacy {
        phase: WalkPhase,
        version: SessionVersion,
    },
}

impl WalkPosition {
    pub fn phase(&self) -> WalkPhase {
        match self {
            Self::NoSession => WalkPhase::Empty,
            Self::Reconstruction { phase } | Self::Legacy { phase, .. } => *phase,
            Self::Unpositioned { version } | Self::Session { version } => version.phase(),
        }
    }

    pub fn version(&self) -> SessionVersion {
        match self {
            Self::NoSession | Self::Reconstruction { .. } => SessionVersion::empty(),
            Self::Unpositioned { version }
            | Self::Session { version }
            | Self::Legacy { version, .. } => version.clone(),
        }
    }

    pub const fn source_label(&self) -> &'static str {
        match self {
            Self::NoSession => "no session",
            Self::Reconstruction { .. } => "pre-session reconstruction",
            Self::Unpositioned { .. } => "durable session without cursor",
            Self::Session { .. } => "durable session cursor",
            Self::Legacy { .. } => "legacy protocol snapshot",
        }
    }

    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::NoSession => Ok(()),
            Self::Reconstruction {
                phase: WalkPhase::Empty,
            } => Err("pre-session reconstruction cannot identify the empty phase"),
            Self::Reconstruction { .. } => Ok(()),
            Self::Unpositioned { version } if version.session_id().is_none() => {
                Err("unpositioned durable session has no session id")
            }
            Self::Unpositioned { version } if version.cursor().is_some() => {
                Err("unpositioned durable session has a committed cursor")
            }
            Self::Unpositioned { version } if version.journal_revision() == 0 => {
                Err("unpositioned durable session has no committed journal revision")
            }
            Self::Unpositioned { .. } => Ok(()),
            Self::Session { version } if version.session_id().is_none() => {
                Err("durable session position has no session id")
            }
            Self::Session { version } if version.cursor().is_none() => {
                Err("durable session position has no committed cursor")
            }
            Self::Session { version } if version.journal_revision() == 0 => {
                Err("durable session position has no committed journal revision")
            }
            Self::Session { version } if version.phase() == WalkPhase::Empty => {
                Err("durable session position has an empty cursor phase")
            }
            Self::Session { version }
                if version
                    .cursor()
                    .is_some_and(|cursor| cursor.validate().is_err()) =>
            {
                Err("durable session position has invalid cursor evidence")
            }
            Self::Session { .. } => Ok(()),
            Self::Legacy { .. } => {
                Err("legacy position is receive-only and requires an omitted position field")
            }
        }
    }
}

/// Nonblocking, structured read model shared by every walk client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkSessionSnapshot {
    pub position: WalkPosition,
    /// Whether this server currently carries the reconstructed typed value.
    pub controller_attached: bool,
    pub authority: WalkAuthority,
    pub job: Option<WalkJobSnapshot>,
    pub blocker: Option<WalkBlocker>,
    pub actions: Vec<WalkAction>,
}

impl WalkSessionSnapshot {
    pub fn phase(&self) -> WalkPhase {
        self.position.phase()
    }

    pub fn version(&self) -> SessionVersion {
        self.position.version()
    }
}

#[derive(Serialize, Deserialize)]
struct WalkSessionSnapshotWire {
    phase: WalkPhase,
    version: SessionVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    position: Option<WalkPosition>,
    controller_attached: bool,
    authority: WalkAuthority,
    job: Option<WalkJobSnapshot>,
    blocker: Option<WalkBlocker>,
    actions: Vec<WalkAction>,
}

impl Serialize for WalkSessionSnapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.position
            .validate()
            .map_err(serde::ser::Error::custom)?;
        WalkSessionSnapshotWire {
            phase: self.phase(),
            version: self.version(),
            position: Some(self.position.clone()),
            controller_attached: self.controller_attached,
            authority: self.authority,
            job: self.job.clone(),
            blocker: self.blocker.clone(),
            actions: self.actions.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for WalkSessionSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = WalkSessionSnapshotWire::deserialize(deserializer)?;
        let position = match wire.position {
            Some(position) => {
                position.validate().map_err(serde::de::Error::custom)?;
                if position.phase() != wire.phase || position.version() != wire.version {
                    return Err(serde::de::Error::custom(
                        "walk position disagrees with its compatibility phase/version fields",
                    ));
                }
                position
            }
            None => WalkPosition::Legacy {
                phase: wire.phase,
                version: wire.version,
            },
        };
        Ok(Self {
            position,
            controller_attached: wire.controller_attached,
            authority: wire.authority,
            job: wire.job,
            blocker: wire.blocker,
            actions: wire.actions,
        })
    }
}

/// Explicit terminal state recorded for an abandoned controller session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkSessionAbandonment {
    pub detail: String,
}

/// Public projection of a controller cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkCursor {
    pub phase: WalkPhase,
    pub evidence: ContentHash,
}

/// Run mode recorded when a controller session was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkRunMode {
    Continuous,
    Step,
}

/// Semantic origin of the controller session without its authority preimage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
pub enum WalkSessionOrigin {
    Admitted { plan_hash: ContentHash },
    Successor { invocation_path: PathBuf },
    Historical { source: ContentHash },
}

/// Read-only classification of a damaged controller journal observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
pub enum WalkSessionDamage {
    Truncated { line: usize, tail: ContentHash },
    Malformed { line: usize, detail: String },
    Sequence { line: usize, detail: String },
}

/// Public, authority-free projection of one durable attempt intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkAttemptIntent {
    pub transition_id: TransitionId,
    pub expected: WalkPhase,
    pub targets: Vec<WalkPhase>,
    pub allow_live_api: bool,
    pub allow_git_changes: bool,
    pub epoch: ServerEpoch,
    pub evidence: ContentHash,
    pub retry: u32,
}

/// Controller epoch observed around one attempted effect boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkEpochReceipt {
    pub before: ServerEpoch,
    pub after: Option<ServerEpoch>,
}

/// Public classification of one durable attempt outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "status")]
pub enum WalkAttemptResult {
    Committed {
        phase: WalkPhase,
        evidence: ContentHash,
    },
    Rejected {
        phase: WalkPhase,
        evidence: ContentHash,
        detail: String,
    },
    Cancelled {
        phase: WalkPhase,
        evidence: ContentHash,
        detail: String,
    },
    Indeterminate {
        phase: Option<WalkPhase>,
        evidence: Option<ContentHash>,
        detail: String,
    },
}

/// Semantic evidence carried by one committed cursor receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkCursorEvidence {
    pub graph_version: String,
    pub edge: ControlEdge,
    pub witness: ContentHash,
}

/// Public projection of one `finished` journal record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkAttemptReceipt {
    pub transition_id: TransitionId,
    pub fence: u64,
    pub result: WalkAttemptResult,
    pub evidence: Option<WalkCursorEvidence>,
    pub epoch: Option<WalkEpochReceipt>,
}

/// Inspectable socket coordinate retained in successor-ready evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkEndpoint {
    pub repo_root: PathBuf,
    pub socket: PathBuf,
    pub pid: u32,
}

/// Exact committed R4c edge named by a successor-ready receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkReadyCommit {
    pub session_id: SessionId,
    pub transition_id: TransitionId,
    pub fence: u64,
    pub cursor: WalkCursor,
    pub mode: WalkRunMode,
}

/// Authority-free successor-ready evidence retained by a session event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkReadyReceipt {
    pub campaign_id: CampaignId,
    pub node_id: String,
    pub runtime_id: RuntimeId,
    pub pid: u32,
    pub incarnation: Option<ProcessIncarnation>,
    pub recorded_at: String,
    pub commit: WalkReadyCommit,
    pub endpoint: Option<WalkEndpoint>,
    pub predecessor: Option<WalkEndpoint>,
}

/// Predecessor attempt bound to one accepted successor handoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkPredecessorAttempt {
    pub session_id: SessionId,
    pub transition_id: TransitionId,
    pub fence: u64,
    pub allow_live_api: bool,
    pub allow_git_changes: bool,
}

/// Public projection of an accepted successor handoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkHandoffAcceptance {
    pub ready: WalkReadyReceipt,
    pub attempt: WalkPredecessorAttempt,
}

/// Explicit recovery action recorded against an older controller fence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
pub enum WalkRecoveryResolution {
    AbandonOwner,
    AbandonSession {
        detail: String,
    },
    ResolveAttempt {
        transition_id: TransitionId,
        result: WalkAttemptResult,
        evidence: Option<WalkCursorEvidence>,
        epoch: Option<WalkEpochReceipt>,
    },
    AcceptHandoff {
        acceptance: WalkHandoffAcceptance,
        result: WalkAttemptResult,
        evidence: Option<WalkCursorEvidence>,
        epoch: WalkEpochReceipt,
    },
    AdmitEpoch {
        prior: ServerEpoch,
        next: ServerEpoch,
    },
}

/// One journal entry in canonical append order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkSessionEvent {
    /// One-based durable journal revision.
    pub revision: usize,
    /// Original millisecond timestamp stored with the entry.
    pub recorded_at_ms: i64,
    pub session_id: SessionId,
    pub kind: WalkSessionEventKind,
}

/// Semantic public projection of every durable controller-session entry kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
pub enum WalkSessionEventKind {
    Created {
        schema_version: String,
        origin: WalkSessionOrigin,
        parent: ParentIdentityRecord,
        profile: RunProfileCommitmentRecord,
        mode: WalkRunMode,
        cursor: Option<WalkCursor>,
    },
    Acquired {
        fence: u64,
        epoch: ServerEpoch,
        runtime_id: Option<RuntimeId>,
        pid: u32,
        incarnation: Option<ProcessIncarnation>,
    },
    AttemptBegan {
        fence: u64,
        intent: WalkAttemptIntent,
    },
    AttemptFinished {
        receipt: WalkAttemptReceipt,
    },
    Recovered {
        fence: u64,
        resolution: WalkRecoveryResolution,
    },
    EpochAdmitted {
        prior: ServerEpoch,
        next: ServerEpoch,
    },
    TailRepaired {
        discarded: ContentHash,
        evidence_path: PathBuf,
    },
    Released {
        fence: u64,
        ready: Option<WalkReadyReceipt>,
    },
}

/// One immutable observation of the durable controller-session journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SessionHistoryWire")]
pub struct WalkSessionHistory {
    pub journal_path: Option<PathBuf>,
    pub version: SessionVersion,
    pub origin: Option<WalkSessionOrigin>,
    pub profile: Option<RunProfileCommitmentRecord>,
    /// Canonical ordered view. Each event corresponds to one validated journal line.
    pub events: Vec<WalkSessionEvent>,
    pub damage: Option<WalkSessionDamage>,
    pub abandonment: Option<WalkSessionAbandonment>,
    pub epoch: ServerEpoch,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionHistoryWire {
    journal_path: Option<PathBuf>,
    version: SessionVersion,
    origin: Option<WalkSessionOrigin>,
    profile: Option<RunProfileCommitmentRecord>,
    events: Vec<WalkSessionEvent>,
    damage: Option<WalkSessionDamage>,
    abandonment: Option<WalkSessionAbandonment>,
    epoch: ServerEpoch,
}

impl TryFrom<SessionHistoryWire> for WalkSessionHistory {
    type Error = String;

    fn try_from(wire: SessionHistoryWire) -> Result<Self, Self::Error> {
        let history = Self {
            journal_path: wire.journal_path,
            version: wire.version,
            origin: wire.origin,
            profile: wire.profile,
            events: wire.events,
            damage: wire.damage,
            abandonment: wire.abandonment,
            epoch: wire.epoch,
        };
        history.validate()?;
        Ok(history)
    }
}

impl WalkSessionHistory {
    pub(crate) fn empty(journal_path: Option<PathBuf>, epoch: ServerEpoch) -> Self {
        Self {
            journal_path,
            version: SessionVersion::empty(),
            origin: None,
            profile: None,
            events: Vec::new(),
            damage: None,
            abandonment: None,
            epoch,
        }
    }

    fn validate(&self) -> Result<(), String> {
        for (index, event) in self.events.iter().enumerate() {
            let expected = index + 1;
            if event.revision != expected {
                return Err(format!(
                    "session history event revision {} is not expected revision {expected}",
                    event.revision
                ));
            }
        }

        let (cursor, abandonment);
        if let Some(created) = self.events.first() {
            let WalkSessionEventKind::Created {
                origin, profile, ..
            } = &created.kind
            else {
                return Err("nonempty session history does not begin with Created".to_string());
            };
            if self.version.session_id != Some(created.session_id) {
                return Err(
                    "session history version does not mirror Created session id".to_string()
                );
            }
            if self.origin.as_ref() != Some(origin) {
                return Err("session history origin does not mirror Created origin".to_string());
            }
            if self.profile.as_ref() != Some(profile) {
                return Err("session history profile does not mirror Created profile".to_string());
            }
            (cursor, abandonment) = replay_history(self)?;
        } else if self.version.session_id.is_some()
            || self.version.cursor.is_some()
            || self.origin.is_some()
            || self.profile.is_some()
            || self.abandonment.is_some()
        {
            return Err(
                "empty session history contains creation-derived summary fields".to_string(),
            );
        } else {
            cursor = None;
            abandonment = None;
        }

        if !cursor_matches(self.version.cursor.as_ref(), cursor.as_ref()) {
            return Err("session history cursor does not match its event prefix".to_string());
        }
        if self.abandonment != abandonment {
            return Err("session history abandonment does not match its event prefix".to_string());
        }

        let count = self.events.len();
        let revision = self.version.journal_revision;
        match &self.damage {
            None if revision != count => Err(format!(
                "undamaged session history has revision {revision}, expected {count}"
            )),
            Some(WalkSessionDamage::Malformed { line, .. })
            | Some(WalkSessionDamage::Truncated { line, .. })
                if revision != count || *line != count + 1 =>
            {
                Err(format!(
                    "tail-damaged session history has revision {revision} and line {line}, expected revision {count} and line {}",
                    count + 1
                ))
            }
            Some(WalkSessionDamage::Sequence { line, .. })
                if *line != count + 1 || revision < *line =>
            {
                Err(format!(
                    "sequence-damaged session history has revision {revision} and line {line}, expected line {} within the observed revision",
                    count + 1
                ))
            }
            _ => Ok(()),
        }
    }
}

fn replay_history(
    history: &WalkSessionHistory,
) -> Result<(Option<WalkCursor>, Option<WalkSessionAbandonment>), String> {
    let created = history
        .events
        .first()
        .expect("nonempty history was checked before replay");
    let WalkSessionEventKind::Created {
        schema_version,
        origin,
        parent,
        mode,
        cursor: created_cursor,
        ..
    } = &created.kind
    else {
        return Err("nonempty session history does not begin with Created".to_string());
    };
    crate::cli::prototype1_state::session::validate_walk_creation(
        schema_version,
        created_cursor.as_ref(),
    )?;

    let mut cursor = created_cursor.clone();
    let mut abandonment = None;
    let mut active: Option<(
        u64,
        ServerEpoch,
        Option<RuntimeId>,
        u32,
        Option<ProcessIncarnation>,
    )> = None;
    let mut last_epoch = None;
    let mut max_fence = 0_u64;
    let mut attempts = BTreeMap::<TransitionId, (u64, WalkAttemptIntent)>::new();
    let mut results = BTreeMap::<TransitionId, WalkAttemptResult>::new();
    let mut finished = BTreeMap::<TransitionId, WalkAttemptReceipt>::new();
    let mut recovered = BTreeSet::<TransitionId>::new();
    let mut recoveries = BTreeSet::<u64>::new();
    let mut ready_seen = false;

    for event in &history.events {
        if event.session_id != created.session_id {
            return Err(format!(
                "session history event revision {} changed session id",
                event.revision
            ));
        }
        match &event.kind {
            WalkSessionEventKind::Created { .. } if event.revision == 1 => {}
            WalkSessionEventKind::Created { .. } => {
                return Err("session history contains more than one Created event".to_string());
            }
            WalkSessionEventKind::Acquired {
                fence,
                epoch,
                runtime_id,
                pid,
                incarnation,
            } => {
                crate::cli::prototype1_state::session::validate_walk_acquisition(
                    schema_version,
                    incarnation.as_ref(),
                )?;
                if active.is_some() {
                    return Err(format!(
                        "session history revision {} acquired a lease while another owner was active",
                        event.revision
                    ));
                }
                let expected = max_fence
                    .checked_add(1)
                    .ok_or_else(|| "session history fence counter overflow".to_string())?;
                if *fence != expected {
                    return Err(format!(
                        "session history revision {} used fence {fence}, expected {expected}",
                        event.revision
                    ));
                }
                if last_epoch.as_ref().is_some_and(|prior| prior != epoch) {
                    return Err(format!(
                        "session history revision {} changed epoch without admission",
                        event.revision
                    ));
                }
                max_fence = *fence;
                last_epoch.get_or_insert_with(|| epoch.clone());
                active = Some((
                    *fence,
                    epoch.clone(),
                    runtime_id.clone(),
                    *pid,
                    incarnation.clone(),
                ));
            }
            WalkSessionEventKind::AttemptBegan { fence, intent } => {
                let Some((owner_fence, owner_epoch, ..)) = active.as_ref() else {
                    return Err(format!(
                        "session history revision {} began an attempt without an active owner",
                        event.revision
                    ));
                };
                if owner_fence != fence {
                    return Err(format!(
                        "session history revision {} used fence {fence}, active fence is {owner_fence}",
                        event.revision
                    ));
                }
                if owner_epoch != &intent.epoch {
                    return Err(format!(
                        "session history revision {} began transition {} under a different epoch",
                        event.revision, intent.transition_id
                    ));
                }
                crate::cli::prototype1_state::session::validate_walk_intent(
                    schema_version,
                    event.session_id,
                    intent,
                )?;
                let unresolved = attempts.iter().any(|(transition_id, (attempt_fence, _))| {
                    *attempt_fence == *fence
                        && !recovered.contains(transition_id)
                        && match results.get(transition_id) {
                            None => true,
                            Some(WalkAttemptResult::Indeterminate { .. }) => true,
                            Some(_) => false,
                        }
                });
                if unresolved {
                    return Err(format!(
                        "session history revision {} began a second unresolved transition attempt",
                        event.revision
                    ));
                }
                match cursor.as_ref() {
                    Some(active)
                        if active.phase != intent.expected
                            || active.evidence != intent.evidence =>
                    {
                        return Err(format!(
                            "session history transition {} does not start at the active cursor",
                            intent.transition_id
                        ));
                    }
                    None => {
                        cursor = Some(WalkCursor {
                            phase: intent.expected,
                            evidence: intent.evidence.clone(),
                        });
                    }
                    Some(_) => {}
                }
                if attempts
                    .insert(intent.transition_id, (*fence, intent.clone()))
                    .is_some()
                {
                    return Err(format!(
                        "session history transition {} began more than once",
                        intent.transition_id
                    ));
                }
            }
            WalkSessionEventKind::AttemptFinished { receipt } => {
                let Some((owner_fence, ..)) = active.as_ref() else {
                    return Err(format!(
                        "session history revision {} finished an attempt without an active owner",
                        event.revision
                    ));
                };
                if *owner_fence != receipt.fence {
                    return Err(format!(
                        "session history revision {} used fence {}, active fence is {owner_fence}",
                        event.revision, receipt.fence
                    ));
                }
                let Some((began_fence, intent)) = attempts.get(&receipt.transition_id) else {
                    return Err(format!(
                        "session history transition {} finished without a pending attempt",
                        receipt.transition_id
                    ));
                };
                if *began_fence != receipt.fence {
                    return Err(format!(
                        "session history transition {} changed fence from {began_fence} to {}",
                        receipt.transition_id, receipt.fence
                    ));
                }
                if results.contains_key(&receipt.transition_id) {
                    return Err(format!(
                        "session history transition {} finished more than once",
                        receipt.transition_id
                    ));
                }
                crate::cli::prototype1_state::session::validate_walk_receipt(
                    schema_version,
                    intent,
                    receipt,
                )?;
                update_cursor(&mut cursor, &receipt.result);
                finished.insert(receipt.transition_id, receipt.clone());
                results.insert(receipt.transition_id, receipt.result.clone());
                if let Some(epoch) = receipt.epoch.as_ref()
                    && let Some(after) = epoch.after.as_ref()
                    && after != &epoch.before
                    && crate::cli::prototype1_state::session::walk_admits_epoch(
                        intent,
                        &receipt.result,
                        &epoch.before,
                        after,
                    )
                {
                    last_epoch = Some(after.clone());
                    active.as_mut().expect("owner was checked").1 = after.clone();
                }
            }
            WalkSessionEventKind::Recovered { fence, resolution } => {
                crate::cli::prototype1_state::session::validate_walk_resolution(
                    schema_version,
                    resolution,
                )?;
                let Some((owner_fence, ..)) = active.as_ref() else {
                    return Err(format!(
                        "session history revision {} recovered without an active owner",
                        event.revision
                    ));
                };
                if owner_fence != fence {
                    return Err(format!(
                        "session history revision {} used recovery fence {fence}, active fence is {owner_fence}",
                        event.revision
                    ));
                }
                if recoveries.contains(fence) {
                    return Err(format!(
                        "session history fence {fence} recovered more than once"
                    ));
                }
                let unresolved = attempts
                    .iter()
                    .find_map(|(transition_id, (attempt_fence, _))| {
                        (*attempt_fence == *fence
                            && !recovered.contains(transition_id)
                            && match results.get(transition_id) {
                                None => true,
                                Some(WalkAttemptResult::Indeterminate { .. }) => true,
                                Some(_) => false,
                            })
                        .then_some(*transition_id)
                    });
                let mut admitted_epoch = None;
                match resolution {
                    WalkRecoveryResolution::AbandonOwner => {
                        if unresolved.is_some() {
                            return Err(
                                "session history abandoned an owner with an unresolved attempt"
                                    .to_string(),
                            );
                        }
                    }
                    WalkRecoveryResolution::AbandonSession { detail } => {
                        if unresolved.is_none() {
                            return Err(
                                "session history abandoned a session without an unresolved attempt"
                                    .to_string(),
                            );
                        }
                        if detail.trim().is_empty() {
                            return Err("session abandonment requires a detail".to_string());
                        }
                        if abandonment
                            .replace(WalkSessionAbandonment {
                                detail: detail.clone(),
                            })
                            .is_some()
                        {
                            return Err(
                                "session history contains duplicate abandonment events".to_string()
                            );
                        }
                    }
                    WalkRecoveryResolution::ResolveAttempt {
                        transition_id,
                        result,
                        evidence,
                        epoch,
                    } => {
                        let Some((_, intent)) = attempts.get(transition_id) else {
                            return Err(format!(
                                "session history recovery names unknown transition {transition_id}"
                            ));
                        };
                        if unresolved != Some(*transition_id) {
                            return Err(format!(
                                "session history recovery does not name the unresolved transition {transition_id}"
                            ));
                        }
                        crate::cli::prototype1_state::session::validate_walk_recovery(
                            schema_version,
                            intent,
                            result,
                            evidence.as_ref(),
                            epoch.as_ref(),
                        )?;
                        validate_recorded_epoch(finished.get(transition_id), epoch.as_ref())?;
                        update_cursor(&mut cursor, result);
                        results.insert(*transition_id, result.clone());
                        recovered.insert(*transition_id);
                        admitted_epoch = epoch.as_ref().and_then(|epoch| {
                            epoch.after.as_ref().and_then(|after| {
                                crate::cli::prototype1_state::session::walk_admits_epoch(
                                    intent,
                                    result,
                                    &epoch.before,
                                    after,
                                )
                                .then(|| after.clone())
                            })
                        });
                    }
                    WalkRecoveryResolution::AcceptHandoff {
                        acceptance,
                        result,
                        evidence,
                        epoch,
                    } => {
                        let transition_id = acceptance.attempt.transition_id;
                        let Some((began_fence, intent)) = attempts.get(&transition_id) else {
                            return Err(format!(
                                "session history handoff names unknown transition {transition_id}"
                            ));
                        };
                        let resolves = unresolved == Some(transition_id);
                        let recertifies = unresolved.is_none()
                            && finished.get(&transition_id).is_some_and(|receipt| {
                                receipt.fence == *fence
                                    && &receipt.result == result
                                    && receipt.evidence.as_ref() == evidence.as_ref()
                                    && receipt.epoch.as_ref() == Some(epoch)
                            });
                        if (!resolves && !recertifies)
                            || *began_fence != *fence
                            || acceptance.attempt.session_id != event.session_id
                            || acceptance.attempt.fence != *began_fence
                            || acceptance.attempt.allow_live_api != intent.allow_live_api
                            || acceptance.attempt.allow_git_changes != intent.allow_git_changes
                            || intent.expected != WalkPhase::R12
                            || !intent.targets.contains(&WalkPhase::R13b)
                            || !intent.allow_git_changes
                            || !matches!(
                                result,
                                WalkAttemptResult::Committed {
                                    phase: WalkPhase::R13b,
                                    ..
                                }
                            )
                        {
                            return Err(format!(
                                "session history handoff does not match transition {transition_id}"
                            ));
                        }
                        validate_ready(&acceptance.ready)?;
                        crate::cli::prototype1_state::session::validate_walk_recovery(
                            schema_version,
                            intent,
                            result,
                            evidence.as_ref(),
                            Some(epoch),
                        )?;
                        validate_recorded_epoch(finished.get(&transition_id), Some(epoch))?;
                        if !recertifies {
                            update_cursor(&mut cursor, result);
                        }
                        results.insert(transition_id, result.clone());
                        recovered.insert(transition_id);
                        admitted_epoch = epoch.after.as_ref().and_then(|after| {
                            crate::cli::prototype1_state::session::walk_admits_epoch(
                                intent,
                                result,
                                &epoch.before,
                                after,
                            )
                            .then(|| after.clone())
                        });
                    }
                    WalkRecoveryResolution::AdmitEpoch { .. } => {
                        return Err(
                            "session epoch admission used an owner-recovery event".to_string()
                        );
                    }
                }
                if let Some(epoch) = admitted_epoch {
                    last_epoch = Some(epoch);
                }
                recoveries.insert(*fence);
                active = None;
            }
            WalkSessionEventKind::EpochAdmitted { prior, next } => {
                if active.is_some() {
                    return Err(
                        "session history admitted an epoch while an owner was active".to_string(),
                    );
                }
                if last_epoch.as_ref() != Some(prior) || prior == next {
                    return Err(
                        "session history epoch admission does not match the established epoch"
                            .to_string(),
                    );
                }
                last_epoch = Some(next.clone());
            }
            WalkSessionEventKind::TailRepaired { .. } => {}
            WalkSessionEventKind::Released { fence, ready } => {
                let Some((owner_fence, owner_epoch, runtime_id, pid, incarnation)) =
                    active.as_ref()
                else {
                    return Err(format!(
                        "session history revision {} released without an active owner",
                        event.revision
                    ));
                };
                if owner_fence != fence {
                    return Err(format!(
                        "session history revision {} released fence {fence}, active fence is {owner_fence}",
                        event.revision
                    ));
                }
                let unresolved = attempts.iter().any(|(transition_id, (attempt_fence, _))| {
                    *attempt_fence == *fence
                        && !recovered.contains(transition_id)
                        && match results.get(transition_id) {
                            None => true,
                            Some(WalkAttemptResult::Indeterminate { .. }) => true,
                            Some(_) => false,
                        }
                });
                if unresolved {
                    return Err(format!(
                        "session history revision {} released with an unresolved attempt",
                        event.revision
                    ));
                }
                if let Some(ready) = ready {
                    if ready_seen || !matches!(origin, WalkSessionOrigin::Successor { .. }) {
                        return Err("session history contains an invalid Ready release".to_string());
                    }
                    validate_ready(ready)?;
                    let commit = &ready.commit;
                    let attempt_matches = attempts
                        .get(&commit.transition_id)
                        .is_some_and(|(attempt_fence, _)| *attempt_fence == commit.fence);
                    let committed = results.get(&commit.transition_id).is_some_and(|result| {
                        matches!(
                            result,
                            WalkAttemptResult::Committed { phase, evidence }
                                if *phase == WalkPhase::R4c
                                    && commit.cursor.phase == WalkPhase::R4c
                                    && evidence == &commit.cursor.evidence
                        )
                    });
                    let endpoint_matches = ready
                        .endpoint
                        .as_ref()
                        .is_none_or(|endpoint| endpoint.repo_root == owner_epoch.repo_root);
                    if !committed
                        || !attempt_matches
                        || commit.session_id != event.session_id
                        || commit.fence != *fence
                        || cursor.as_ref() != Some(&commit.cursor)
                        || commit.mode != *mode
                        || ready.campaign_id != parent.campaign_id
                        || ready.node_id != parent.node_id
                        || runtime_id.as_ref() != Some(&ready.runtime_id)
                        || ready.pid != *pid
                        || ready.incarnation.as_ref() != incarnation.as_ref()
                        || !endpoint_matches
                    {
                        return Err(
                            "session Ready release does not match its owner and R4c transition"
                                .to_string(),
                        );
                    }
                    ready_seen = true;
                }
                active = None;
            }
        }
    }

    Ok((cursor, abandonment))
}

fn validate_recorded_epoch(
    receipt: Option<&WalkAttemptReceipt>,
    observed: Option<&WalkEpochReceipt>,
) -> Result<(), String> {
    let Some(recorded) = receipt
        .and_then(|receipt| receipt.epoch.as_ref())
        .and_then(|epoch| epoch.after.as_ref())
    else {
        return Ok(());
    };
    if observed.and_then(|epoch| epoch.after.as_ref()) != Some(recorded) {
        return Err(
            "attempt recovery contradicted the previously observed after epoch".to_string(),
        );
    }
    Ok(())
}

fn validate_ready(ready: &WalkReadyReceipt) -> Result<(), String> {
    if ready.campaign_id.0.trim().is_empty()
        || ready.node_id.trim().is_empty()
        || ready.runtime_id.0.trim().is_empty()
        || ready.pid == 0
        || ready.recorded_at.trim().is_empty()
    {
        return Err("session Ready receipt has incomplete identity".to_string());
    }
    if let Some(incarnation) = ready.incarnation.as_ref()
        && (incarnation.boot_id.is_nil() || incarnation.start_ticks == 0)
    {
        return Err("session Ready receipt has incomplete process incarnation".to_string());
    }
    if ready.commit.cursor.phase != WalkPhase::R4c {
        return Err("session Ready receipt cursor is not R4c".to_string());
    }
    crate::cli::prototype1_state::session::validate_walk_cursor(&ready.commit.cursor)?;
    match (
        ready.commit.mode,
        ready.endpoint.as_ref(),
        ready.predecessor.as_ref(),
    ) {
        (WalkRunMode::Step, Some(endpoint), Some(predecessor))
            if endpoint.pid == ready.pid
                && predecessor.repo_root == endpoint.repo_root
                && predecessor.socket != endpoint.socket
                && endpoint.pid != 0
                && predecessor.pid != 0 =>
        {
            Ok(())
        }
        (WalkRunMode::Step, _, _) => {
            Err("Step-mode session Ready requires successor and predecessor endpoints".to_string())
        }
        (WalkRunMode::Continuous, None, None) => Ok(()),
        (WalkRunMode::Continuous, _, _) => {
            Err("Continuous-mode session Ready cannot contain endpoints".to_string())
        }
    }
}

fn update_cursor(cursor: &mut Option<WalkCursor>, result: &WalkAttemptResult) {
    if let WalkAttemptResult::Committed { phase, evidence } = result {
        *cursor = Some(WalkCursor {
            phase: *phase,
            evidence: evidence.clone(),
        });
    }
}

fn cursor_matches(cursor: Option<&Cursor>, projected: Option<&WalkCursor>) -> bool {
    match (cursor, projected) {
        (None, None) => true,
        (Some(cursor), Some(projected)) => {
            cursor.phase() == projected.phase && cursor.evidence() == projected.evidence.as_str()
        }
        _ => false,
    }
}

/// Exact typestate-axis changes produced by one admitted control edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkEdgeDelta {
    pub edge: ControlEdge,
    pub axes: Vec<RuntimeAxisDelta>,
}

impl WalkEdgeDelta {
    pub const fn from(&self) -> WalkPhase {
        self.edge.from()
    }

    pub const fn to(&self) -> WalkPhase {
        self.edge.to()
    }
}

/// Availability and content of the most recent in-process walk advance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "status")]
pub enum WalkDeltaState {
    NotRecorded,
    Recorded {
        from: WalkPhase,
        edges: Vec<WalkEdgeDelta>,
    },
}

/// Revision-tagged typed result of inspecting the most recent walk advance.
///
/// A custom decode check rejects disconnected edge chains, mismatched axis
/// deltas, and results whose final phase disagrees with the observed durable
/// session version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "WalkDeltaWire")]
pub struct WalkDeltaSnapshot {
    pub version: SessionVersion,
    pub state: WalkDeltaState,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WalkDeltaWire {
    version: SessionVersion,
    state: WalkDeltaState,
}

impl TryFrom<WalkDeltaWire> for WalkDeltaSnapshot {
    type Error = String;

    fn try_from(wire: WalkDeltaWire) -> Result<Self, Self::Error> {
        if let WalkDeltaState::Recorded { from, edges } = &wire.state {
            let mut phase = *from;
            for delta in edges {
                if delta.from() != phase {
                    return Err(format!(
                        "walk delta edge {} starts at {}, expected {}",
                        delta.edge,
                        delta.from(),
                        phase
                    ));
                }
                let axes = delta.to().axis_deltas_from(delta.from());
                if delta.axes != axes {
                    return Err(format!(
                        "walk delta edge {} carries typestate axes that do not match {} -> {}",
                        delta.edge,
                        delta.from(),
                        delta.to()
                    ));
                }
                phase = delta.to();
            }
            if phase != wire.version.phase() {
                return Err(format!(
                    "walk delta ends at {phase}, but durable session version is at {}",
                    wire.version.phase()
                ));
            }
        }
        Ok(Self {
            version: wire.version,
            state: wire.state,
        })
    }
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
    /// Read-only projection of the configuration admitted for this checkout.
    Config {
        /// Current durable phase observed with the configuration projection.
        phase: WalkPhase,
        /// Passive identity, campaign, profile, and effective-control records.
        config: WalkConfigSnapshot,
        /// Server freshness identity.
        epoch: ServerEpoch,
    },
    /// Completed-run inventory from the canonical run registry.
    EvaluationTraceIndex { index: EvaluationTraceIndex },
    /// Lifecycle-sensitive exact evaluation trace observation.
    EvaluationTrace { snapshot: EvaluationTraceSnapshot },
    /// Typed inventory of mutable LLM debugger evidence.
    LlmTraceIndex { index: LlmTraceIndex },
    /// Exact mutable LLM debugger observation.
    LlmTrace { snapshot: LlmTraceSnapshot },
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
    /// Ordered typed projection of one durable session-journal observation.
    History { history: WalkSessionHistory },
    /// Typed most-recent transition delta plus its human rendering.
    Delta {
        phase: WalkPhase,
        report: String,
        snapshot: WalkDeltaSnapshot,
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

    /// Build the structured most-recent-transition response.
    pub(crate) fn delta(
        phase: WalkPhase,
        report: String,
        snapshot: WalkDeltaSnapshot,
        epoch: ServerEpoch,
    ) -> Self {
        Self::Delta {
            phase,
            report,
            snapshot,
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

    /// Build a typed read-only configuration response.
    pub(crate) fn config(phase: WalkPhase, config: WalkConfigSnapshot, epoch: ServerEpoch) -> Self {
        Self::Config {
            phase,
            config,
            epoch,
        }
    }

    /// Build a typed completed-run inventory response.
    pub(crate) fn evaluation_trace_index(index: EvaluationTraceIndex) -> Self {
        Self::EvaluationTraceIndex { index }
    }

    /// Build one lifecycle-sensitive exact-run trace response.
    pub(crate) fn evaluation_trace(snapshot: EvaluationTraceSnapshot) -> Self {
        Self::EvaluationTrace { snapshot }
    }

    pub(crate) fn llm_trace_index(index: LlmTraceIndex) -> Self {
        Self::LlmTraceIndex { index }
    }

    pub(crate) fn llm_trace(snapshot: LlmTraceSnapshot) -> Self {
        Self::LlmTrace { snapshot }
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

    /// Build a typed durable session-history response.
    pub(crate) fn history(history: WalkSessionHistory) -> Self {
        Self::History { history }
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
            | WalkResponse::Config { phase, .. }
            | WalkResponse::Job { phase, .. }
            | WalkResponse::Delta { phase, .. } => Some(*phase),
            WalkResponse::Query { query } => Some(query.phase),
            WalkResponse::Status { snapshot, .. } => Some(snapshot.phase()),
            WalkResponse::History { history } => Some(history.version.phase()),
            WalkResponse::EvaluationTraceIndex { index } => Some(index.version.phase()),
            WalkResponse::EvaluationTrace { snapshot } => Some(snapshot.version.phase()),
            WalkResponse::LlmTraceIndex { index } => Some(index.version.phase()),
            WalkResponse::LlmTrace { snapshot } => Some(snapshot.version.phase()),
            WalkResponse::Error { phase, .. } => *phase,
        }
    }

    /// Exact server/binary identity that produced this response.
    pub fn epoch(&self) -> &ServerEpoch {
        match self {
            WalkResponse::Ok { epoch, .. }
            | WalkResponse::Audit { epoch, .. }
            | WalkResponse::Config { epoch, .. }
            | WalkResponse::Job { epoch, .. }
            | WalkResponse::Status { epoch, .. }
            | WalkResponse::Delta { epoch, .. }
            | WalkResponse::Error { epoch, .. } => epoch,
            WalkResponse::Query { query } => &query.epoch,
            WalkResponse::History { history } => &history.epoch,
            WalkResponse::EvaluationTraceIndex { index } => &index.epoch,
            WalkResponse::EvaluationTrace { snapshot } => &snapshot.epoch,
            WalkResponse::LlmTraceIndex { index } => &index.epoch,
            WalkResponse::LlmTrace { snapshot } => &snapshot.epoch,
        }
    }

    /// Return whether this response is `Ok`.
    pub fn is_ok(&self) -> bool {
        matches!(
            self,
            WalkResponse::Ok { .. }
                | WalkResponse::Audit { .. }
                | WalkResponse::Config { .. }
                | WalkResponse::Query { .. }
                | WalkResponse::Job { .. }
                | WalkResponse::Status { .. }
                | WalkResponse::History { .. }
                | WalkResponse::Delta { .. }
                | WalkResponse::EvaluationTraceIndex { .. }
                | WalkResponse::EvaluationTrace { .. }
                | WalkResponse::LlmTraceIndex { .. }
                | WalkResponse::LlmTrace { .. }
        )
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;

    fn reconstructed() -> WalkSessionSnapshot {
        WalkSessionSnapshot {
            position: WalkPosition::Reconstruction {
                phase: WalkPhase::R4c,
            },
            controller_attached: true,
            authority: WalkAuthority::Active,
            job: None,
            blocker: None,
            actions: Vec::new(),
        }
    }

    #[test]
    fn snapshot_round_trip_preserves_closed_position_and_compatibility_fields() {
        let snapshot = reconstructed();

        let encoded = serde_json::to_value(&snapshot).expect("serialize snapshot");
        let decoded: WalkSessionSnapshot =
            serde_json::from_value(encoded.clone()).expect("deserialize snapshot");

        assert_eq!(decoded, snapshot);
        assert_eq!(encoded["phase"], serde_json::json!("r4c"));
        assert_eq!(
            encoded["version"],
            serde_json::to_value(SessionVersion::empty()).expect("serialize empty version")
        );
        assert_eq!(encoded["position"]["source"], "reconstruction");
    }

    #[test]
    fn snapshot_rejects_position_that_disagrees_with_compatibility_fields() {
        let mut encoded = serde_json::to_value(reconstructed()).expect("serialize snapshot");
        encoded["phase"] = serde_json::json!("r4a");

        let error = serde_json::from_value::<WalkSessionSnapshot>(encoded)
            .expect_err("mismatched position must be rejected")
            .to_string();

        assert!(error.contains("walk position disagrees"), "{error}");
    }

    #[test]
    fn snapshot_without_position_decodes_as_legacy_authority() {
        let mut encoded = serde_json::to_value(reconstructed()).expect("serialize snapshot");
        encoded
            .as_object_mut()
            .expect("snapshot object")
            .remove("position");

        let decoded: WalkSessionSnapshot =
            serde_json::from_value(encoded).expect("decode v8 snapshot");

        assert!(matches!(
            decoded.position,
            WalkPosition::Legacy {
                phase: WalkPhase::R4c,
                ref version,
            } if version == &SessionVersion::empty()
        ));
    }

    #[test]
    fn snapshot_rejects_session_position_without_session_identity() {
        let mut encoded = serde_json::to_value(WalkSessionSnapshot {
            position: WalkPosition::NoSession,
            controller_attached: false,
            authority: WalkAuthority::Active,
            job: None,
            blocker: None,
            actions: Vec::new(),
        })
        .expect("serialize no-session snapshot");
        encoded["position"] = serde_json::json!({
            "source": "session",
            "version": SessionVersion::empty(),
        });

        let error = serde_json::from_value::<WalkSessionSnapshot>(encoded)
            .expect_err("session without identity must be rejected")
            .to_string();

        assert!(
            error.contains("durable session position has no session id"),
            "{error}"
        );
    }

    #[test]
    fn snapshot_rejects_session_position_without_committed_cursor() {
        let snapshot = WalkSessionSnapshot {
            position: WalkPosition::Session {
                version: SessionVersion {
                    session_id: Some(SessionId::for_test(7)),
                    cursor: None,
                    journal_revision: 1,
                },
            },
            controller_attached: true,
            authority: WalkAuthority::Active,
            job: None,
            blocker: None,
            actions: Vec::new(),
        };

        let error = serde_json::to_value(snapshot)
            .expect_err("session without cursor must be rejected")
            .to_string();

        assert!(
            error.contains("durable session position has no committed cursor"),
            "{error}"
        );
    }

    #[test]
    fn snapshot_rejects_session_position_with_empty_cursor_phase() {
        let snapshot = WalkSessionSnapshot {
            position: WalkPosition::Session {
                version: SessionVersion {
                    session_id: Some(SessionId::for_test(7)),
                    cursor: Some(Cursor {
                        phase: WalkPhase::Empty,
                        evidence: ContentHash::of("empty session cursor"),
                    }),
                    journal_revision: 1,
                },
            },
            controller_attached: true,
            authority: WalkAuthority::Active,
            job: None,
            blocker: None,
            actions: Vec::new(),
        };

        let error = serde_json::to_value(snapshot)
            .expect_err("session with an empty cursor phase must be rejected")
            .to_string();

        assert!(error.contains("empty cursor phase"), "{error}");
    }

    #[test]
    fn snapshot_rejects_session_position_with_invalid_cursor_evidence() {
        let mut encoded = serde_json::to_value(WalkSessionSnapshot {
            position: WalkPosition::Session {
                version: SessionVersion {
                    session_id: Some(SessionId::for_test(7)),
                    cursor: Some(
                        Cursor::new(WalkPhase::R3, ContentHash::of("valid session cursor"))
                            .expect("valid cursor"),
                    ),
                    journal_revision: 1,
                },
            },
            controller_attached: true,
            authority: WalkAuthority::Active,
            job: None,
            blocker: None,
            actions: Vec::new(),
        })
        .expect("serialize valid session snapshot");
        encoded["position"]["version"]["cursor"]["evidence"] = serde_json::json!("not-a-digest");

        let error = serde_json::from_value::<WalkSessionSnapshot>(encoded)
            .expect_err("session with invalid cursor evidence must be rejected")
            .to_string();

        assert!(error.contains("invalid cursor evidence"), "{error}");
    }

    #[test]
    fn snapshot_round_trip_accepts_unpositioned_session() {
        let version = SessionVersion {
            session_id: Some(SessionId::for_test(9)),
            cursor: None,
            journal_revision: 2,
        };
        let snapshot = WalkSessionSnapshot {
            position: WalkPosition::Unpositioned {
                version: version.clone(),
            },
            controller_attached: false,
            authority: WalkAuthority::RecoveryRequired,
            job: None,
            blocker: Some(WalkBlocker {
                code: WalkBlockerCode::ControllerBlocked,
                detail: "cursorless v1 session".to_string(),
            }),
            actions: Vec::new(),
        };

        let encoded = serde_json::to_value(&snapshot).expect("serialize cursorless session");
        let decoded: WalkSessionSnapshot =
            serde_json::from_value(encoded.clone()).expect("deserialize cursorless session");

        assert_eq!(decoded, snapshot);
        assert_eq!(decoded.phase(), WalkPhase::Empty);
        assert_eq!(decoded.version(), version);
        assert_eq!(encoded["position"]["source"], "unpositioned");
    }

    #[test]
    fn snapshot_rejects_explicit_legacy_position_on_current_wire() {
        let snapshot = WalkSessionSnapshot {
            position: WalkPosition::Legacy {
                phase: WalkPhase::R4c,
                version: SessionVersion::empty(),
            },
            controller_attached: true,
            authority: WalkAuthority::Active,
            job: None,
            blocker: None,
            actions: Vec::new(),
        };

        let error = serde_json::to_value(snapshot)
            .expect_err("legacy authority must be receive-only")
            .to_string();

        assert!(error.contains("legacy position is receive-only"), "{error}");
    }

    #[test]
    fn snapshot_round_trip_accepts_complete_session_position() {
        let version = SessionVersion {
            session_id: Some(SessionId::for_test(8)),
            cursor: Some(
                Cursor::new(WalkPhase::R3, ContentHash::of("r3 session position"))
                    .expect("valid cursor"),
            ),
            journal_revision: 1,
        };
        let snapshot = WalkSessionSnapshot {
            position: WalkPosition::Session {
                version: version.clone(),
            },
            controller_attached: true,
            authority: WalkAuthority::Active,
            job: None,
            blocker: None,
            actions: Vec::new(),
        };

        let encoded = serde_json::to_value(&snapshot).expect("serialize session snapshot");
        let decoded: WalkSessionSnapshot =
            serde_json::from_value(encoded).expect("deserialize session snapshot");

        assert_eq!(decoded, snapshot);
        assert_eq!(decoded.version(), version);
    }
}
