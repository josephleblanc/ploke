use std::collections::BTreeMap;
use std::path::PathBuf;

use ploke_records::history::EvidenceRefRecord;
use ploke_records::ids::{ArtifactId, BlockHash, EntryId, RuntimeId};
use ploke_records::run_profile::{
    ExecutionStopAfter, GenerationSource, GenerationSurface, RunProfileCommitmentRecord,
    RunProfileRecord, SelectionEvidence, SelectionStrategy, TraceJsonl,
};
use ploke_records::scheduler::{ChildBudgetRecord, ChildScheduleModeRecord};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTurnArtifactKind {
    Trace,
    Summary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnArtifactMetadata {
    pub task_id: String,
    pub selected_model: String,
    pub user_message_id: String,
    pub event_count: usize,
    pub terminal_outcome: Option<String>,
    pub terminal_attempts: Option<u32>,
    pub final_assistant_message_id: Option<String>,
    pub patch_applied: bool,
    pub all_proposals_applied: bool,
    pub edit_proposal_count: usize,
    pub create_proposal_count: usize,
    pub expected_file_change_count: usize,
    pub llm_prompt_message_count: usize,
    pub has_llm_response: bool,
    pub tool_request_event_count: usize,
    pub tool_completed_event_count: usize,
    pub tool_failed_event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunProfileMetadata {
    pub schema_version: String,
    pub name: String,
    pub worktree_root: PathBuf,
    pub target_dataset_key: Option<String>,
    pub target_instance: Option<String>,
    pub target_instances: Vec<String>,
    pub max_generations: u32,
    pub max_total_nodes: u32,
    pub child_budget: ChildBudgetRecord,
    pub child_schedule_mode: ChildScheduleModeRecord,
    pub stop_on_first_keep: bool,
    pub require_keep_for_continuation: bool,
    pub explore_from_rejected: bool,
    pub generation_source: GenerationSource,
    pub generation_surface: Option<GenerationSurface>,
    pub selection_strategy: SelectionStrategy,
    pub selection_evidence: SelectionEvidence,
    pub selection_seed: u64,
    pub execution_stop_after: ExecutionStopAfter,
    pub trace_jsonl: TraceJsonl,
    pub debug_tools: bool,
}

impl From<&RunProfileRecord> for RunProfileMetadata {
    fn from(profile: &RunProfileRecord) -> Self {
        Self {
            schema_version: profile.schema_version.clone(),
            name: profile.name.clone(),
            worktree_root: profile.storage.worktree_root.clone(),
            target_dataset_key: profile.target.dataset_key.clone(),
            target_instance: profile.target.instance.clone(),
            target_instances: profile.target.instances.clone(),
            max_generations: profile.search.max_generations,
            max_total_nodes: profile.search.max_total_nodes,
            child_budget: profile.search.children,
            child_schedule_mode: profile.search.schedule,
            stop_on_first_keep: profile.search.stop_on_first_keep,
            require_keep_for_continuation: profile.search.require_keep_for_continuation,
            explore_from_rejected: profile.search.explore_from_rejected,
            generation_source: profile.generation.source,
            generation_surface: profile.generation.surface,
            selection_strategy: profile.selection.strategy,
            selection_evidence: profile.selection.evidence,
            selection_seed: profile.selection.seed,
            execution_stop_after: profile.execution.stop_after,
            trace_jsonl: profile.execution.trace_jsonl,
            debug_tools: profile.execution.debug_tools,
        }
    }
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
    AgentTurnArtifact {
        path: PathBuf,
        kind: AgentTurnArtifactKind,
        task_id: String,
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
    RunProfileSummary(RunProfileMetadata),
    RunProfileCommitment(RunProfileCommitmentRecord),
    AgentTurnEvidenceSummary {
        trace_file_count: usize,
        trace_parsed_count: usize,
        summary_file_count: usize,
        summary_parsed_count: usize,
        artifact_with_terminal_record_count: usize,
        artifact_with_final_message_count: usize,
        artifact_with_applied_patch_count: usize,
        tool_request_event_count: usize,
        tool_completed_event_count: usize,
        tool_failed_event_count: usize,
    },
    AgentTurnArtifact(AgentTurnArtifactMetadata),
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
    RunProfileSummary,
    RunProfileCommitment,
    AgentTurnEvidenceSummary,
    AgentTurnTraceArtifact,
    AgentTurnSummaryArtifact,
}
