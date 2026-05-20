use std::collections::BTreeMap;

use ploke_records::agent_turn::{AgentTurnArtifactRecord, ObservedTurnEventRecord};
use ploke_records::child_plan::ChildPlanRecord;
use ploke_records::evaluation::Artifact as EvaluationArtifact;
use ploke_records::invocation::InvocationRecord;
use ploke_records::journal::JournalEntry;
use ploke_records::protocol::Artifact as ProtocolArtifact;
use ploke_records::run_profile::{RunProfileCommitmentRecord, RunProfileRecord};
use ploke_records::run_record::{RunRecord, ToolResult};
use ploke_records::scheduler::{RunnerRequestRecord, RunnerResultRecord};
use serde::{Deserialize, Serialize};

/// Passive evidence counts loaded beside the scheduler tree.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PassiveEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_registry: Option<BranchRegistryEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_journal: Option<JsonlEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<HistoryEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_envelopes: Option<ChannelEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_plans: Option<ChildPlanEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluations: Option<EvaluationEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_artifacts: Option<ProtocolArtifactsEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_records: Option<RunRecordEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_profile: Option<RunProfileEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_attempts: Option<RunAttemptEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_turns: Option<AgentTurnEvidence>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attempt_runner_results: BTreeMap<String, RunnerResultRecord>,
}

/// Typed transition journal loaded in append order from `transition-journal.jsonl`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TransitionJournal {
    pub entries: Vec<JsonlRecord<JournalEntry>>,
}

impl TransitionJournal {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, JsonlRecord<JournalEntry>> {
        self.entries.iter()
    }
}

/// One parsed JSONL record with its source line preserved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonlRecord<T> {
    pub line_number: usize,
    pub record: T,
}

/// Counts from a passive Prototype 1 branch registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchRegistryEvidence {
    pub source_node_count: usize,
    pub branch_count: usize,
    pub active_target_count: usize,
    #[serde(default)]
    pub record_count: usize,
    #[serde(default)]
    pub registry_snapshot_count: usize,
    #[serde(default)]
    pub parent_comparison_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_campaign_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_recorded_at: Option<String>,
}

/// Counts from a JSONL evidence file parsed as passive records.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonlEvidence {
    pub line_count: usize,
    pub parsed_count: usize,
    pub parse_error_count: usize,
}

/// Counts from passive History block storage.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEvidence {
    pub line_count: usize,
    pub sealed_block_count: usize,
    pub admitted_entry_count: usize,
    pub record_parse_error_count: usize,
    pub json_parse_error_count: usize,
}

/// Counts from passive runtime channel envelope files.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelEvidence {
    pub file_count: usize,
    pub line_count: usize,
    pub parsed_count: usize,
    pub parse_error_count: usize,
}

/// Read-only typed evidence loaded from child-plan message boxes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChildPlanEvidence {
    pub summary: ChildPlanSummary,
    pub index: BTreeMap<String, ChildPlanRecord>,
}

/// Counts from persisted child-plan message boxes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildPlanSummary {
    pub file_count: usize,
    pub parsed_count: usize,
    pub child_count: usize,
    pub children_with_surface_count: usize,
    pub rejected_surface_attempt_count: usize,
}

/// Read-only typed evidence loaded from evaluation artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct EvaluationEvidence {
    pub summary: EvaluationArtifactSummary,
    pub index: BTreeMap<String, EvaluationArtifact>,
}

/// Counts from persisted evaluation artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationArtifactSummary {
    pub file_count: usize,
    pub parsed_count: usize,
    pub keep_count: usize,
    pub reject_count: usize,
}

/// Read-only typed evidence loaded from protocol artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProtocolArtifactsEvidence {
    pub summary: ProtocolArtifactSummary,
    pub index: BTreeMap<String, ProtocolArtifact>,
}

/// Counts from persisted protocol artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolArtifactSummary {
    pub file_count: usize,
    pub parsed_count: usize,
    pub intent_segmentation_count: usize,
    pub review_count: usize,
    pub segment_review_count: usize,
    pub typed_payload_count: usize,
}

/// Read-only typed evidence loaded from compressed `record.json.gz` files.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RunRecordEvidence {
    pub summary: RunRecordSummary,
    pub index: BTreeMap<String, RunRecord>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stats: BTreeMap<String, RunRecordStats>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refs_by_branch: BTreeMap<String, Vec<BranchRunRecordRef>>,
}

/// Counts from persisted compressed run records.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunRecordSummary {
    pub file_count: usize,
    pub parsed_count: usize,
    pub branch_ref_count: usize,
    pub baseline_ref_count: usize,
    pub treatment_ref_count: usize,
    pub records_with_setup_count: usize,
    pub records_with_packaging_count: usize,
    pub total_turn_count: usize,
    pub total_tool_call_count: usize,
    pub failed_tool_call_count: usize,
}

/// Load-time facts derived once from a compressed run record for UI inspection.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunRecordStats {
    pub turn_count: usize,
    pub tool_call_count: usize,
    pub failed_tool_call_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_wall_clock_millis: Option<u64>,
}

impl RunRecordStats {
    pub fn from_record(record: &RunRecord) -> Self {
        let turn_count = record.phases.agent_turns.len();
        let mut tool_call_count = 0;
        let mut failed_tool_call_count = 0;
        for turn in &record.phases.agent_turns {
            tool_call_count += turn.tool_calls.len();
            failed_tool_call_count += turn
                .tool_calls
                .iter()
                .filter(|call| matches!(call.result, ToolResult::Failed(_)))
                .count();
        }
        Self {
            turn_count,
            tool_call_count,
            failed_tool_call_count,
            total_wall_clock_millis: record
                .timing
                .as_ref()
                .map(|timing| (timing.total_wall_clock_secs * 1000.0).round() as u64),
        }
    }
}

/// Branch-scoped comparison arm that points at one loaded compressed run record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchRunRecordRef {
    pub branch_id: String,
    pub instance_id: String,
    pub arm: ComparedRunArm,
    pub record_key: String,
    pub record_path: std::path::PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComparedRunArm {
    Baseline,
    Treatment,
}

/// Read-only run metadata and reproducibility commitment loaded from a run root.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunProfileEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<RunProfileRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment: Option<RunProfileCommitmentRecord>,
}

/// Read-only runner request/result and invocation records loaded from node dirs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RunAttemptEvidence {
    pub summary: RunAttemptSummary,
    pub runner_requests: BTreeMap<String, RunnerRequestRecord>,
    pub runner_results: BTreeMap<String, RunnerResultRecord>,
    pub invocations: BTreeMap<String, InvocationRecord>,
}

/// Counts from persisted runner attempt metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunAttemptSummary {
    pub runner_request_file_count: usize,
    pub runner_request_parsed_count: usize,
    pub runner_result_file_count: usize,
    pub runner_result_parsed_count: usize,
    pub invocation_file_count: usize,
    pub invocation_parsed_count: usize,
    pub child_invocation_count: usize,
    pub successor_invocation_count: usize,
}

/// Read-only typed evidence loaded from agent-turn trace and summary artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentTurnEvidence {
    pub summary: AgentTurnEvidenceSummary,
    pub traces: BTreeMap<String, AgentTurnArtifactEvidence>,
    pub summaries: BTreeMap<String, AgentTurnArtifactEvidence>,
}

/// Counts from persisted agent-turn artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentTurnEvidenceSummary {
    pub trace_file_count: usize,
    pub trace_parsed_count: usize,
    pub summary_file_count: usize,
    pub summary_parsed_count: usize,
    pub artifact_with_terminal_record_count: usize,
    pub artifact_with_final_message_count: usize,
    pub artifact_with_applied_patch_count: usize,
    pub tool_request_event_count: usize,
    pub tool_completed_event_count: usize,
    pub tool_failed_event_count: usize,
}

impl AgentTurnEvidenceSummary {
    pub fn observe_artifact(&mut self, artifact: &AgentTurnArtifactEvidence) {
        if artifact.terminal_outcome.is_some() {
            self.artifact_with_terminal_record_count += 1;
        }
        if artifact.final_assistant_message_id.is_some() {
            self.artifact_with_final_message_count += 1;
        }
        if artifact.patch_applied {
            self.artifact_with_applied_patch_count += 1;
        }
        self.tool_request_event_count += artifact.tool_request_event_count;
        self.tool_completed_event_count += artifact.tool_completed_event_count;
        self.tool_failed_event_count += artifact.tool_failed_event_count;
    }
}

/// Canonical agent-turn records loaded for drilldown playback.
///
/// Summary evidence keeps compact counts for graph attachment. These records
/// preserve the ordered turn events for lower-granularity playback consumers.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AgentTurnRecordSet {
    #[serde(default)]
    pub traces: BTreeMap<String, AgentTurnArtifactRecord>,
    #[serde(default)]
    pub summaries: BTreeMap<String, AgentTurnArtifactRecord>,
}

impl AgentTurnRecordSet {
    pub fn is_empty(&self) -> bool {
        self.traces.is_empty() && self.summaries.is_empty()
    }
}

/// Passive projection over fields already parsed through `ploke_records::agent_turn`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentTurnArtifactEvidence {
    pub task_id: String,
    pub selected_model: String,
    pub user_message_id: String,
    pub event_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_attempts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

impl AgentTurnArtifactEvidence {
    pub fn from_record(record: &AgentTurnArtifactRecord) -> Self {
        let tool_request_event_count = record
            .events
            .iter()
            .filter(|event| matches!(event, ObservedTurnEventRecord::ToolRequested(_)))
            .count();
        let tool_completed_event_count = record
            .events
            .iter()
            .filter(|event| matches!(event, ObservedTurnEventRecord::ToolCompleted(_)))
            .count();
        let tool_failed_event_count = record
            .events
            .iter()
            .filter(|event| matches!(event, ObservedTurnEventRecord::ToolFailed(_)))
            .count();

        Self {
            task_id: record.task_id.clone(),
            selected_model: record.selected_model.clone(),
            user_message_id: record.user_message_id.clone(),
            event_count: record.events.len(),
            terminal_outcome: record
                .terminal_record
                .as_ref()
                .map(|terminal| terminal.outcome.clone()),
            terminal_attempts: record
                .terminal_record
                .as_ref()
                .map(|terminal| terminal.attempts),
            final_assistant_message_id: record
                .final_assistant_message
                .as_ref()
                .map(|message| message.id.clone()),
            patch_applied: record.patch_artifact.applied,
            all_proposals_applied: record.patch_artifact.all_proposals_applied,
            edit_proposal_count: record.patch_artifact.edit_proposals.len(),
            create_proposal_count: record.patch_artifact.create_proposals.len(),
            expected_file_change_count: record.patch_artifact.expected_file_changes.len(),
            llm_prompt_message_count: record.llm_prompt.len(),
            has_llm_response: record.llm_response.is_some(),
            tool_request_event_count,
            tool_completed_event_count,
            tool_failed_event_count,
        }
    }
}
