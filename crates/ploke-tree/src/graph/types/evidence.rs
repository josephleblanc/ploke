use std::collections::BTreeMap;
use std::path::PathBuf;

use ploke_records::history::EvidenceRefRecord;
use ploke_records::ids::{ArtifactId, BlockHash, EntryId, RuntimeId};

/// Typed evidence attachments. These are not lineage authority.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvidenceIndex {
    pub attachments: BTreeMap<EvidenceId, EvidenceAttachment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceId(pub(crate) u64);

#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceAttachment {
    pub id: EvidenceId,
    pub subject: EvidenceSubject,
    pub kind: EvidenceKind,
    pub locators: Vec<EvidenceLocator>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceLocator {
    HistoryRef(EvidenceRefRecord),
    SchedulerState,
    SchedulerNode {
        node_id: String,
    },
    ParentIdentity {
        node_id: String,
        parent_id: String,
    },
    SuccessorReady {
        node_id: String,
        runtime_id: RuntimeId,
    },
    SuccessorCompletion {
        node_id: String,
        runtime_id: RuntimeId,
    },
    TransitionJournalLine {
        line_number: usize,
    },
    LoadedSummary {
        name: &'static str,
    },
    EvaluationArtifact {
        path: PathBuf,
    },
    ProtocolArtifact {
        path: PathBuf,
        procedure_name: String,
        subject_id: String,
        run_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceSubject {
    HistoryBlock(BlockHash),
    HistoryEntry(EntryId),
    SchedulerCampaign(String),
    SchedulerNode(String),
    Runtime(RuntimeId),
    Artifact(ArtifactId),
    Candidate {
        selection_entry_id: EntryId,
        payload_index: usize,
    },
    Branch(String),
    Selection(EntryId),
    TransitionJournalSummary {
        line_count: usize,
        parsed_count: usize,
        parse_error_count: usize,
    },
    TransitionJournalLoadedEntries {
        loaded_entry_count: usize,
    },
    BranchRegistrySummary {
        source_node_count: usize,
        branch_count: usize,
        active_target_count: usize,
        record_count: usize,
        registry_snapshot_count: usize,
        parent_comparison_count: usize,
        latest_campaign_id: Option<String>,
        latest_recorded_at: Option<String>,
    },
    HistoryStorageSummary {
        line_count: usize,
        sealed_block_count: usize,
        admitted_entry_count: usize,
        record_parse_error_count: usize,
        json_parse_error_count: usize,
    },
    ChannelSummary {
        file_count: usize,
        line_count: usize,
        parsed_count: usize,
        parse_error_count: usize,
    },
    EvaluationSummary {
        file_count: usize,
        parsed_count: usize,
        keep_count: usize,
        reject_count: usize,
    },
    ChildPlanSummary {
        file_count: usize,
        parsed_count: usize,
        child_count: usize,
        children_with_surface_count: usize,
        rejected_surface_attempt_count: usize,
    },
    ProtocolArtifactSummary {
        file_count: usize,
        parsed_count: usize,
        intent_segmentation_count: usize,
        review_count: usize,
        segment_review_count: usize,
        typed_payload_count: usize,
    },
    ProtocolArtifact {
        procedure_name: String,
        subject_id: String,
        run_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    HistoryBlockHeader,
    HistoryEntryCustody,
    SchedulerStateSummary,
    SchedulerNodeRecord,
    ParentIdentity,
    SuccessorReady,
    SuccessorCompletion,
    TransitionJournalSummary,
    TransitionJournalLoadedEntries,
    TransitionJournalEntry,
    BranchRegistrySummary,
    HistoryStorageSummary,
    ChannelSummary,
    ChildPlanSummary,
    EvaluationSummary,
    ProtocolArtifactSummary,
    SelectionDecision,
    CandidatePayload,
    CandidateEvaluation,
    ProtocolArtifact,
}
