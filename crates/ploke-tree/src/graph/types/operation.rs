use ploke_records::history::{
    ActorRefRecord, EntryKindRecord, EvidenceRefRecord, ProcedureRefRecord, SubjectRefRecord,
};
use ploke_records::ids::{BlockHash, BlockId, EntryId, LineageId, RecordedAt};

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
