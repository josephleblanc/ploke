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
    Prototype1StateStopAfter, Prototype1SuccessorSelection, Prototype1TraversalMetrics,
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
    /// Create a fresh in-memory walk and advance until an early target phase.
    Start {
        /// Captured command arguments for the new walk.
        config: WalkStartConfig,
        /// Early phase to stop at after creating `R0`.
        until: WalkPhase,
    },
    /// Advance the existing in-memory walk.
    Step {
        /// If present, advance repeatedly until this phase; otherwise one step.
        until: Option<WalkPhase>,
    },
    /// Inspect current phase and summary without mutating state.
    Show,
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
