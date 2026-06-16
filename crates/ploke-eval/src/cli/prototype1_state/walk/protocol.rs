use std::path::PathBuf;

use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};

use crate::cli::{
    InspectOutputFormat, Prototype1CandidateGenerator, Prototype1StateCommand,
    Prototype1StateStopAfter, Prototype1SuccessorSelection, Prototype1TraversalMetrics,
};

use super::{epoch::ServerEpoch, phase::WalkPhase};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WalkRequest {
    pub(crate) client_epoch: Option<ServerEpoch>,
    pub(crate) body: WalkRequestBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum WalkRequestBody {
    Health,
    Start {
        config: WalkStartConfig,
        until: WalkPhase,
    },
    Step {
        until: Option<WalkPhase>,
    },
    Show,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum WalkResponse {
    Ok {
        phase: WalkPhase,
        message: String,
        epoch: ServerEpoch,
    },
    Error {
        code: String,
        detail: String,
        phase: Option<WalkPhase>,
        epoch: ServerEpoch,
    },
}

impl WalkResponse {
    pub(crate) fn ok(phase: WalkPhase, message: impl Into<String>, epoch: ServerEpoch) -> Self {
        Self::Ok {
            phase,
            message: message.into(),
            epoch,
        }
    }

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

    pub(crate) fn phase(&self) -> Option<WalkPhase> {
        match self {
            WalkResponse::Ok { phase, .. } => Some(*phase),
            WalkResponse::Error { phase, .. } => *phase,
        }
    }

    pub(crate) fn is_ok(&self) -> bool {
        matches!(self, WalkResponse::Ok { .. })
    }
}
