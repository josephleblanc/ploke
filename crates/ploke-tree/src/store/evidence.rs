use std::collections::BTreeMap;

use ploke_records::child_plan::ChildPlanRecord;
use ploke_records::evaluation::Artifact as EvaluationArtifact;
use ploke_records::journal::JournalEntry;
use ploke_records::protocol::Artifact as ProtocolArtifact;
use ploke_records::run_profile::{RunProfileCommitmentRecord, RunProfileRecord};
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
    pub run_profile: Option<RunProfileEvidence>,
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

/// Read-only run metadata and reproducibility commitment loaded from a run root.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunProfileEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<RunProfileRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment: Option<RunProfileCommitmentRecord>,
}
