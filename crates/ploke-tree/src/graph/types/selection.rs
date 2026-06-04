use std::{cmp::Ordering, collections::BTreeMap};

use ploke_records::evaluation::RunMetrics;
use ploke_records::history::{
    CandidateSetRootRecord, ProcedureRefRecord, ProtocolMetricsRecord, SelectionScopeRecord,
    SubjectRefRecord, TraversalStrategyRecord,
};
use ploke_records::ids::{
    ArtifactId, CandidateId, CandidateMembershipId, CandidateOccurrenceId, EntryId, HistoryHash,
    PatchId,
};
use ploke_records::selection::Outcome as SelectionOutcome;
use ploke_records::selection::{
    FormulaRecord, ImpAtK, MetricPolicy, ScoreChildPropRecord, ScoreChildPropRowRecord,
};

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
    /// archaeology:selection-protocol-evidence
    /// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
    pub metric_witnesses: BTreeMap<SelectionMetricWitnessKey, SelectionMetricWitness>,
}

/// Traversal policy facts sealed beside a selection decision entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionTraversalSummary {
    pub seed: u64,
    pub strategy: TraversalStrategyRecord,
    pub selected_source: Option<CandidateSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionMetricWitnessKey {
    pub entry_id: EntryId,
    pub payload_index: usize,
    pub branch_id: String,
}

impl Ord for SelectionMetricWitnessKey {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.entry_id, self.payload_index, &self.branch_id).cmp(&(
            &other.entry_id,
            other.payload_index,
            &other.branch_id,
        ))
    }
}

impl PartialOrd for SelectionMetricWitnessKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Selection-time metric witness bundling the sealed header join, metric row,
/// and optional `score_child_prop` replay row for one considered branch.
/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionMetricWitness {
    pub key: SelectionMetricWitnessKey,
    pub selection_entry_id: EntryId,
    pub metric_candidate: MetricCandidateNode,
    pub score_child_prop_row: Option<ScoreChildPropRowRecord>,
}

/// Borrowed selection metric witness resolved from a loaded graph.
#[derive(Debug, Clone, Copy)]
pub struct SelectionMetricWitnessRef<'g> {
    pub key: &'g SelectionMetricWitnessKey,
    pub selection: &'g SelectionNode,
    pub metric_candidate: &'g MetricCandidateNode,
    pub score_child_prop_row: Option<&'g ScoreChildPropRowRecord>,
    pub imp_at_k: Option<&'g ImpAtK>,
}

/// One generation row in the campaign trajectory landing table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrajectoryGenerationRow {
    pub generation: u32,
    pub selection_entry_id: EntryId,
    pub branch_id: Option<String>,
    pub artifact_id: Option<ArtifactId>,
    pub score_child_prop_total: Option<i64>,
    pub decision_outcome: SelectionOutcome,
    pub role_hint: Option<TrajectoryRoleHint>,
}

/// Role hint derived from scheduler/invocation records for the selected node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrajectoryRoleHint {
    Parent,
    Child,
    Successor,
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
    /// archaeology:selection-protocol-evidence
    /// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
    pub traversal: Option<SelectionTraversalSummary>,
    /// Generation label for the selected candidate when coordinate evidence exists.
    pub generation_label: Option<String>,
}

/// Selection-time metrics sealed beside History selection entries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetricIndex {
    pub sets: BTreeMap<HistoryHash, MetricSetNode>,
    pub candidates: BTreeMap<MetricCandidateKey, MetricCandidateNode>,
    /// archaeology:score-child-prop-ui
    /// proof:docs/active/archaeology/ploke-tree-graph/score-child-prop-ui-spec.md
    pub formulas: BTreeMap<SelectionFormulaKey, SelectionFormulaNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricSetNode {
    pub metric_set_id: HistoryHash,
    pub selection_entry_id: EntryId,
    pub considered_order_hash: HistoryHash,
    pub candidate_set_root: Option<HistoryHash>,
    pub candidate_count: usize,
    /// archaeology:selection-protocol-evidence
    /// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
    pub policy: MetricPolicy,
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
    /// archaeology:selection-protocol-evidence
    /// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
    pub compared_runs: Vec<ComparedRunMetricNode>,
}

/// Selection-time run metrics sealed beside a considered candidate.
/// archaeology:selection-protocol-evidence
/// proof:docs/active/archaeology/ploke-tree-graph/selection-protocol-evidence.md
#[derive(Debug, Clone, PartialEq)]
pub struct ComparedRunMetricNode {
    pub instance_id: Option<String>,
    pub status: Option<String>,
    pub baseline_metrics: Option<RunMetrics>,
    pub treatment_metrics: Option<RunMetrics>,
    pub baseline_protocol: Option<ProtocolMetricsRecord>,
    pub treatment_protocol: Option<ProtocolMetricsRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionFormulaKey {
    pub selection_entry_id: EntryId,
    pub metric_set_id: HistoryHash,
}

impl Ord for SelectionFormulaKey {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.selection_entry_id, &self.metric_set_id)
            .cmp(&(&other.selection_entry_id, &other.metric_set_id))
    }
}

impl PartialOrd for SelectionFormulaKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Selection-entry-scoped selector formula values ingested from History.
/// archaeology:score-child-prop-ui
/// proof:docs/active/archaeology/ploke-tree-graph/score-child-prop-ui-spec.md
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionFormulaNode {
    pub selection_entry_id: EntryId,
    pub metric_set_id: HistoryHash,
    pub formula: SelectionFormulaKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectionFormulaKind {
    ScoreChildProp(ScoreChildPropNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoreChildPropNode {
    pub record: ScoreChildPropRecord,
}

impl ScoreChildPropNode {
    pub fn row_for_payload_index(&self, payload_index: usize) -> Option<&ScoreChildPropRowRecord> {
        self.record
            .rows
            .iter()
            .find(|row| row.payload_index == payload_index)
    }
}

impl SelectionMetricWitnessRef<'_> {
    pub fn score_child_prop_total(&self) -> Option<i64> {
        self.score_child_prop_row
            .and_then(score_child_prop_row_total_points)
    }
}

pub fn score_child_prop_row_total_points(row: &ScoreChildPropRowRecord) -> Option<i64> {
    row.performance.or_else(|| {
        Some(
            row.outcome_points
                .saturating_add(row.operational_points)
                .saturating_add(row.protocol_points)
                .saturating_add(row.imp_at_k_delta.unwrap_or(0)),
        )
    })
}

impl From<FormulaRecord> for SelectionFormulaKind {
    fn from(value: FormulaRecord) -> Self {
        match value {
            FormulaRecord::ScoreChildProp(record) => {
                Self::ScoreChildProp(ScoreChildPropNode { record })
            }
        }
    }
}
