use ploke_records::history::SealedBlockRecord;
use ploke_records::identity::ParentIdentityRecord;
use ploke_records::invocation::{SuccessorCompletionRecord, SuccessorReadyRecord};
use ploke_records::scheduler::{NodeRecord, SchedulerStateRecord};
use serde::{Deserialize, Serialize};

use super::{AgentTurnRecordSet, PassiveEvidence, TransitionJournal};

/// In-memory inputs for one run-forest projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunForestInput {
    pub scheduler: SchedulerStateRecord,
    #[serde(default)]
    pub node_records: Vec<NodeRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_identity: Option<ParentIdentityRecord>,
    #[serde(default)]
    pub successor_ready: Vec<SuccessorReadyRecord>,
    #[serde(default)]
    pub successor_completion: Vec<SuccessorCompletionRecord>,
    #[serde(default)]
    pub passive_evidence: PassiveEvidence,
}

/// Typed records loaded for one Prototype 1 run root.
///
/// This is a read-only record carrier for projection crates. It does not make
/// scheduler sidecars authoritative and it does not interpret sealed History;
/// callers choose the projection they need from the same loaded record set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunRecordSet {
    pub forest_input: RunForestInput,
    #[serde(default)]
    pub history_blocks: Vec<SealedBlockRecord>,
    #[serde(default)]
    pub transition_journal: TransitionJournal,
    #[serde(default, skip_serializing_if = "AgentTurnRecordSet::is_empty")]
    pub agent_turn_records: AgentTurnRecordSet,
}

/// Lightweight typed counts for choosing a run root before full graph import.
///
/// This deliberately excludes passive evidence such as compressed child
/// `record.json.gz` payloads. It is suitable for picker labels, not for graph
/// authority or detailed inspectors.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunRootSummary {
    pub scheduler_node_count: usize,
    pub artifact_count: usize,
    pub history_block_count: usize,
    pub candidate_count: usize,
}
