use std::collections::BTreeMap;

use ploke_records::history::{
    ActorRefRecord, ArtifactRefRecord, EntryKindRecord, EvidenceRefRecord, ProcedureRefRecord,
    SubjectRefRecord, SurfaceCommitmentRecord,
};
use ploke_records::ids::{BlockHash, BlockId, EntryId, LineageId, RecordedAt};

/// Sealed History index. Block height is lineage-local.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoryIndex {
    pub lineages: BTreeMap<LineageId, LineageNode>,
    pub blocks: BTreeMap<BlockHash, HistoryBlockNode>,
    pub entries: BTreeMap<EntryId, HistoryEntryNode>,
}

/// One History lineage authority coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageNode {
    pub lineage_id: LineageId,
    pub blocks: Vec<BlockHash>,
}

/// One sealed authority epoch in a lineage.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryBlockNode {
    pub block_hash: BlockHash,
    pub block_id: BlockId,
    pub lineage_id: LineageId,
    pub block_height: u64,
    pub parent_block_hashes: Vec<BlockHash>,
    pub opened_from_artifact: ArtifactRefRecord,
    pub active_artifact: ArtifactRefRecord,
    pub selected_successor: SuccessorNode,
    pub opening_authority: OpeningAuthorityNode,
    pub ruling_authority: ActorRefRecord,
    pub policy_ref: ProcedureRefRecord,
    pub surface: SurfaceCommitmentRecord,
    pub opened_at: RecordedAt,
    pub sealed_at: RecordedAt,
    pub entry_count: usize,
    pub entries: Vec<EntryId>,
}

/// Block-opening authority, kept role-aware for graph consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpeningAuthorityNode {
    Genesis {
        bootstrap_policy: ProcedureRefRecord,
        tree_key_hash: String,
        parent_identity_ref: EvidenceRefRecord,
    },
    Predecessor {
        predecessor_block_hash: BlockHash,
    },
}

/// Selected successor named by a sealed block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorNode {
    pub runtime: ActorRefRecord,
    pub artifact: ArtifactRefRecord,
}

/// Authority facts indexed separately from visual/provenance entities.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AuthorityIndex {
    pub epochs_by_lineage: BTreeMap<LineageId, Vec<BlockHash>>,
}

/// One admitted History entry with custody roles preserved.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntryNode {
    pub entry_id: EntryId,
    pub block_hash: BlockHash,
    pub lineage_id: LineageId,
    pub block_id: BlockId,
    pub block_height: u64,
    pub kind: EntryKindRecord,
    pub subject: SubjectRefRecord,
    pub executor: ActorRefRecord,
    pub observer: ActorRefRecord,
    pub recorder: ActorRefRecord,
    pub proposer: ActorRefRecord,
    pub admitting_authority: ActorRefRecord,
    pub ruling_authority: ActorRefRecord,
    pub procedure_or_policy: ProcedureRefRecord,
    pub payload: HistoryPayloadKind,
    pub occurred_at: RecordedAt,
    pub observed_at: RecordedAt,
    pub recorded_at: RecordedAt,
    pub input_refs: Vec<EvidenceRefRecord>,
    pub output_refs: Vec<EvidenceRefRecord>,
    pub payload_ref: EvidenceRefRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryPayloadKind {
    Direct,
    SelectionDecision,
    IngressImport,
}
