//! Request and response DTOs for the typestate walk socket protocol.
//!
//! The protocol is private to `ploke-eval` and intentionally uses serde JSON
//! over explicit frame lengths. It mirrors the current CLI command surface just
//! enough for the server to construct the real `Prototype1StateCommand` before
//! entering `R0`.

use std::path::PathBuf;

use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};

use crate::cli::{
    InspectOutputFormat, Prototype1CandidateGenerator, Prototype1StateCommand,
    Prototype1StateStopAfter, Prototype1StateWalkLlmStepSource, Prototype1SuccessorSelection,
    Prototype1TraversalMetrics,
};

use super::{epoch::ServerEpoch, phase::WalkPhase};

/// Serializable form of the arguments needed to create `typestate::R0`.
///
/// This is not a second source of command semantics: `into_state_command`
/// immediately rebuilds the existing `Prototype1StateCommand`, and all later
/// transitions use `live_edges`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WalkStartConfig {
    pub(crate) campaign: Option<CampaignId>,
    pub(crate) node_id: Option<String>,
    pub(crate) repo_root: Option<PathBuf>,
    pub(crate) init_parent_identity: bool,
    pub(crate) identity_branch: Option<String>,
    pub(crate) identity_instance: Option<String>,
    pub(crate) handoff_invocation: Option<PathBuf>,
    pub(crate) stop_after: Prototype1StateStopAfter,
    pub(crate) successor_selection: Prototype1SuccessorSelection,
    pub(crate) successor_selection_seed: u64,
    pub(crate) successor_selection_metrics: Prototype1TraversalMetrics,
    pub(crate) candidate_generator: Prototype1CandidateGenerator,
    pub(crate) format: InspectOutputFormat,
}

impl WalkStartConfig {
    /// Rehydrate the existing live command type consumed by `typestate::R0`.
    pub(crate) fn into_state_command(self) -> Prototype1StateCommand {
        Prototype1StateCommand {
            campaign: self.campaign,
            node_id: self.node_id,
            repo_root: self.repo_root,
            init_parent_identity: self.init_parent_identity,
            identity_branch: self.identity_branch,
            identity_instance: self.identity_instance,
            handoff_invocation: self.handoff_invocation,
            stop_after: self.stop_after,
            successor_selection: self.successor_selection,
            successor_selection_seed: self.successor_selection_seed,
            successor_selection_metrics: self.successor_selection_metrics,
            candidate_generator: self.candidate_generator,
            format: self.format,
        }
    }
}

/// One framed client-to-server request.
///
/// Mutating requests include `client_epoch`; read-only requests may omit it so
/// stale servers remain inspectable and stoppable.
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
    /// Create a fresh in-memory walk and advance until an admitted target phase.
    Start {
        /// Captured command arguments for the new walk.
        config: WalkStartConfig,
        /// Phase to stop at after creating `R0`.
        until: WalkPhase,
    },
    /// Advance the existing in-memory walk.
    Step {
        /// If present, advance repeatedly until this phase; otherwise one step.
        until: Option<WalkPhase>,
        /// Allow long live edges to run to completion instead of stopping at a safe boundary.
        #[serde(default)]
        watch: bool,
        /// Admit typed edges that mutate the active checkout during handoff.
        #[serde(default)]
        allow_git_changes: bool,
    },
    /// Clear the in-memory walk while keeping the server process alive.
    Reset,
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
    /// Failed request result.
    Error {
        /// Stable-ish error class for clients.
        code: String,
        /// Human-readable error detail.
        detail: String,
        /// Best known phase when the error was produced.
        phase: Option<WalkPhase>,
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
            epoch,
        }
    }

    /// Return the response phase, if one was available.
    pub(crate) fn phase(&self) -> Option<WalkPhase> {
        match self {
            WalkResponse::Ok { phase, .. } => Some(*phase),
            WalkResponse::Error { phase, .. } => *phase,
        }
    }

    /// Return whether this response is `Ok`.
    pub(crate) fn is_ok(&self) -> bool {
        matches!(self, WalkResponse::Ok { .. })
    }
}
