use std::{cmp::Ordering, collections::BTreeMap};

use ploke_records::history::{
    CandidateSetRootRecord, ProcedureRefRecord, SelectionScopeRecord, SubjectRefRecord,
};
use ploke_records::ids::{
    ArtifactId, CandidateId, CandidateMembershipId, CandidateOccurrenceId, EntryId, HistoryHash,
    PatchId,
};
use ploke_records::selection::ImpAtK;
use ploke_records::selection::Outcome as SelectionOutcome;

use super::evidence::EvidenceId;

/// Candidate and candidate-set facts admitted by History selection entries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CandidateIndex {
    pub candidates: Vec<CandidateNode>,
    pub branches: Vec<CandidateBranchNode>,
    pub memberships: BTreeMap<CandidateMembershipKey, CandidateMembershipNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateNode {
    pub selection_entry_id: EntryId,
    pub payload_index: usize,
    pub subject: SubjectRefRecord,
    pub source: Option<CandidateSource>,
    pub occurrence_id: Option<CandidateOccurrenceId>,
    pub membership_id: Option<CandidateMembershipId>,
    pub membership_key: Option<CandidateMembershipKey>,
    pub node_id: Option<String>,
    pub branch_id: Option<String>,
    pub generation: Option<u32>,
    pub primary_runtime_id: Option<String>,
    pub artifact_after: Option<ArtifactId>,
    pub patch_id: Option<PatchId>,
    pub evidence: Vec<EvidenceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateBranchNode {
    pub selection_entry_id: EntryId,
    pub payload_index: usize,
    pub branch_id: String,
    pub candidate_id: Option<CandidateId>,
    pub source_state_id: Option<String>,
    pub parent_branch_id: Option<String>,
    pub base_artifact_id: Option<ArtifactId>,
    pub derived_artifact_id: Option<ArtifactId>,
    pub patch_id: Option<PatchId>,
    pub evidence: Vec<EvidenceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateSource {
    History,
    CurrentGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateMembershipKey {
    pub candidate_set_root: CandidateSetRootRecord,
    pub membership_id: CandidateMembershipId,
}

impl Ord for CandidateMembershipKey {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.candidate_set_root.0, &self.membership_id)
            .cmp(&(&other.candidate_set_root.0, &other.membership_id))
    }
}

impl PartialOrd for CandidateMembershipKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateMembershipNode {
    pub membership_id: CandidateMembershipId,
    pub candidate_set_root: CandidateSetRootRecord,
    pub occurrence_id: Option<CandidateOccurrenceId>,
    pub candidate_subject: SubjectRefRecord,
    pub selection_entry_id: EntryId,
    pub payload_hash: String,
}

/// Selection decisions sealed into History entries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectionIndex {
    pub selections: BTreeMap<EntryId, SelectionNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionNode {
    pub entry_id: EntryId,
    pub procedure_or_policy: ProcedureRefRecord,
    pub scope: SelectionScopeRecord,
    pub selected_candidate: Option<SubjectRefRecord>,
    pub selected_occurrence_id: Option<CandidateOccurrenceId>,
    pub selected_membership_id: Option<CandidateMembershipId>,
    pub candidate_set_root: Option<CandidateSetRootRecord>,
    pub considered_count: usize,
    pub projection_failure_count: usize,
    pub metric_set_id: HistoryHash,
    pub decision_outcome: SelectionOutcome,
}

/// Selection-time metrics sealed beside History selection entries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetricIndex {
    pub sets: BTreeMap<HistoryHash, MetricSetNode>,
    pub candidates: BTreeMap<MetricCandidateKey, MetricCandidateNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricSetNode {
    pub metric_set_id: HistoryHash,
    pub selection_entry_id: EntryId,
    pub considered_order_hash: HistoryHash,
    pub candidate_set_root: Option<HistoryHash>,
    pub candidate_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricCandidateKey {
    pub metric_set_id: HistoryHash,
    pub payload_index: usize,
}

impl Ord for MetricCandidateKey {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.metric_set_id, self.payload_index).cmp(&(&other.metric_set_id, other.payload_index))
    }
}

impl PartialOrd for MetricCandidateKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricCandidateNode {
    pub metric_set_id: HistoryHash,
    pub selection_entry_id: EntryId,
    pub payload_index: usize,
    pub payload_hash: HistoryHash,
    pub candidate: String,
    pub occurrence_id: Option<CandidateOccurrenceId>,
    pub membership_id: Option<CandidateMembershipId>,
    pub imp_at_k: Option<ImpAtK>,
}
