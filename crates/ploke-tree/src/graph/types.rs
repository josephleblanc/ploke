mod artifact;
mod child_plan;
mod evidence;
mod history;
mod operation;
mod parent_create;
mod runtime;
mod selection;
mod warning;

pub use artifact::*;
pub use child_plan::*;
pub use evidence::*;
pub use history::*;
pub use operation::*;
pub use parent_create::*;
pub use runtime::*;
pub use selection::*;
pub use warning::*;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use ploke_records::ids::ArtifactId;
use ploke_records::invocation::{InvocationRecord, Role};
use ploke_records::protocol::ArtifactBody;
use ploke_records::run_record::SubmissionArtifactState;
use serde::Serialize;

use crate::{
    AgentTurnRecordSet, BranchRunRecordRef, ClosureEvidence, ProtocolArtifactsEvidence,
    RunAttemptEvidence, RunRecordEvidence,
};

/// Immutable read-side graph assembled from one loaded Prototype 1 run.
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    /// Scheduler/prototype node forest assembled from typed run records.
    ///
    /// This remains available for process/schedule drilldown and future
    /// step-through projections. The default `ploke-egui` canvas is now the
    /// borrowed artifact-first `artifact_tree()` projection instead.
    pub forest: Option<crate::RunForest>,
    /// Sealed History is the primary ordering and authority spine.
    pub history: HistoryIndex,
    pub authority: AuthorityIndex,
    /// Recoverable checkout identities observed through History and evidence.
    pub artifacts: ArtifactIndex,
    /// Concrete hydrated executions observed through actors and passive records.
    pub runtimes: RuntimeIndex,
    /// Generative or compositional actions. Evidence may mention these before
    /// they are promoted to core History facts.
    pub operations: OperationIndex,
    /// Selection candidate universes and their set-scoped memberships.
    pub candidates: CandidateIndex,
    pub selections: SelectionIndex,
    pub metrics: MetricIndex,
    /// Parent-published child plans that carry patch/surface details.
    pub child_plans: ChildPlanIndex,
    /// Typed attachments that explain graph objects without replacing History.
    pub evidence: EvidenceIndex,
    /// Canonical agent-turn records available for event-level playback drilldown.
    pub agent_turn_records: AgentTurnRecordSet,
    pub warnings: Vec<GraphWarning>,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            forest: None,
            history: HistoryIndex::default(),
            authority: AuthorityIndex::default(),
            artifacts: ArtifactIndex::default(),
            runtimes: RuntimeIndex::default(),
            operations: OperationIndex::default(),
            candidates: CandidateIndex::default(),
            selections: SelectionIndex::default(),
            metrics: MetricIndex::default(),
            child_plans: ChildPlanIndex::default(),
            evidence: EvidenceIndex::default(),
            agent_turn_records: AgentTurnRecordSet::default(),
            warnings: Vec::new(),
        }
    }
}

impl Graph {
    /// Loaded agent-turn records carried through the graph boundary.
    pub fn agent_turn_records(&self) -> &AgentTurnRecordSet {
        &self.agent_turn_records
    }

    /// Loaded run-attempt records carried through the graph boundary.
    pub fn run_attempts(&self) -> Option<&RunAttemptEvidence> {
        self.forest
            .as_ref()
            .and_then(|forest| forest.passive_evidence.run_attempts.as_ref())
    }

    /// Protocol artifacts loaded from run-attempt directories referenced by this graph.
    pub fn protocol_artifacts(&self) -> Option<&ProtocolArtifactsEvidence> {
        self.forest
            .as_ref()
            .and_then(|forest| forest.passive_evidence.protocol_artifacts.as_ref())
    }

    /// Compressed run records loaded from evaluation baseline/treatment paths.
    pub fn run_records(&self) -> Option<&RunRecordEvidence> {
        self.forest
            .as_ref()
            .and_then(|forest| forest.passive_evidence.run_records.as_ref())
    }

    /// Campaign closure state loaded beside the run root, when present.
    pub fn closure(&self) -> Option<&ClosureEvidence> {
        self.forest
            .as_ref()
            .and_then(|forest| forest.passive_evidence.closure.as_ref())
    }

    /// Baseline eval/protocol evidence view for run-review UI surfaces.
    pub fn run_review_evidence(&self) -> RunReviewEvidence<'_> {
        RunReviewEvidence {
            closure: self.closure(),
            run_records: self.run_records(),
            protocol_artifacts: self.protocol_artifacts(),
        }
    }

    /// Baseline/treatment compressed run-record refs for one evaluation branch.
    pub fn run_record_refs_for_branch<'a>(
        &'a self,
        branch_id: &str,
    ) -> impl Iterator<Item = &'a BranchRunRecordRef> + 'a {
        self.run_records()
            .and_then(|records| records.refs_by_branch.get(branch_id))
            .into_iter()
            .flatten()
    }

    /// Candidate protocol artifact directories derived from typed evaluation record paths.
    pub fn protocol_artifact_dirs(&self) -> BTreeSet<PathBuf> {
        let mut dirs = BTreeSet::new();
        let Some(evaluations) = self
            .forest
            .as_ref()
            .and_then(|forest| forest.passive_evidence.evaluations.as_ref())
        else {
            return dirs;
        };

        for evaluation in evaluations.index.values() {
            for compared in &evaluation.compared_instances {
                for record_path in [
                    compared.baseline_record_path.as_ref(),
                    compared.treatment_record_path.as_ref(),
                ]
                .into_iter()
                .flatten()
                {
                    if let Some(run_dir) = record_path.parent() {
                        dirs.insert(run_dir.join("protocol-artifacts"));
                    }
                }
            }
        }

        dirs
    }

    /// Invocation records loaded from `nodes/<node>/invocations/<runtime>.json`.
    pub fn invocations(&self) -> impl Iterator<Item = (&str, &InvocationRecord)> {
        self.run_attempts().into_iter().flat_map(|attempts| {
            attempts
                .invocations
                .iter()
                .map(|(path, invocation)| (path.as_str(), invocation))
        })
    }

    /// Invocation records whose persisted role is `child`.
    pub fn child_invocations(&self) -> impl Iterator<Item = (&str, &InvocationRecord)> {
        self.invocations()
            .filter(|(_, invocation)| invocation.role == Role::Child)
    }

    /// Child invocation records whose node or request produced `artifact_id`.
    pub fn child_invocations_for_artifact<'a>(
        &'a self,
        artifact_id: &'a ArtifactId,
    ) -> impl Iterator<Item = (&'a str, &'a InvocationRecord)> + 'a {
        self.child_invocations().filter(move |(_, invocation)| {
            invocation
                .node
                .as_ref()
                .and_then(|node| node.derived_artifact_id.as_ref())
                .is_some_and(|derived| derived == artifact_id)
                || invocation
                    .request
                    .as_ref()
                    .and_then(|request| request.derived_artifact_id.as_ref())
                    .is_some_and(|derived| derived == artifact_id)
        })
    }
}

/// Borrowed graph-level witness for run-review evidence.
#[derive(Debug, Clone, Copy)]
pub struct RunReviewEvidence<'g> {
    pub closure: Option<&'g ClosureEvidence>,
    pub run_records: Option<&'g RunRecordEvidence>,
    pub protocol_artifacts: Option<&'g ProtocolArtifactsEvidence>,
}

impl<'g> RunReviewEvidence<'g> {
    pub fn is_available(&self) -> bool {
        self.closure.is_some() || self.run_records.is_some() || self.protocol_artifacts.is_some()
    }

    pub fn protocol_review_stats(&self) -> ProtocolReviewStats {
        let mut stats = ProtocolReviewStats::default();
        let Some(protocol_artifacts) = self.protocol_artifacts else {
            return stats;
        };

        for artifact in protocol_artifacts.index.values() {
            match &artifact.body {
                ArtifactBody::ToolCallReview(payload) => {
                    stats.call_review_count += 1;
                    observe_protocol_labels(
                        &mut stats,
                        &payload.output.overall,
                        &payload.output.redundancy.verdict,
                        &payload.output.recoverability.verdict,
                    );
                }
                ArtifactBody::ToolCallSegmentReview(payload) => {
                    stats.segment_review_count += 1;
                    observe_protocol_labels(
                        &mut stats,
                        &payload.output.overall,
                        &payload.output.redundancy.verdict,
                        &payload.output.recoverability.verdict,
                    );
                }
                ArtifactBody::InterventionIssueDetection(payload) => {
                    stats.issue_detection_count += 1;
                    stats.issue_detection_case_count += payload.output.cases.len();
                }
                ArtifactBody::InterventionSynthesis(payload) => {
                    stats.intervention_synthesis_count += 1;
                    stats.intervention_candidate_count +=
                        payload.output.candidate_set.candidates.len();
                }
                ArtifactBody::InterventionApply(_) => {
                    stats.intervention_apply_count += 1;
                }
                ArtifactBody::ToolCallIntentSegmentation(_) => {}
            }
        }

        stats
    }

    pub fn patch_stats(&self) -> RunReviewPatchStats {
        let mut stats = RunReviewPatchStats::default();
        let Some(run_records) = self.run_records else {
            return stats;
        };

        for record in run_records.index.values() {
            if record.phases.patch.is_some() {
                stats.patch_phase_count += 1;
            }
            if let Some(packaging) = record.phases.packaging.as_ref() {
                match packaging.submission_artifact_state {
                    SubmissionArtifactState::Empty => stats.empty_submission_count += 1,
                    SubmissionArtifactState::Nonempty => stats.nonempty_submission_count += 1,
                    _ => {}
                }
                let label = serde_label(&packaging.patch_projection_check_state);
                *stats.patch_projection_states.entry(label).or_default() += 1;
            }

            for turn in &record.phases.agent_turns {
                let Some(artifact) = turn.agent_turn_artifact.as_ref() else {
                    continue;
                };
                stats.agent_turn_patch_artifact_count += 1;
                stats.edit_proposal_count += artifact.patch_artifact.edit_proposals.len();
                stats.create_proposal_count += artifact.patch_artifact.create_proposals.len();
                stats.expected_file_change_count +=
                    artifact.patch_artifact.expected_file_changes.len();
                if artifact.patch_artifact.applied {
                    stats.applied_patch_artifact_count += 1;
                }
            }
        }

        stats
    }
}

/// Aggregate protocol-review labels derived from persisted protocol artifacts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProtocolReviewStats {
    pub call_review_count: usize,
    pub segment_review_count: usize,
    pub issue_detection_count: usize,
    pub issue_detection_case_count: usize,
    pub intervention_synthesis_count: usize,
    pub intervention_candidate_count: usize,
    pub intervention_apply_count: usize,
    pub overall: BTreeMap<String, usize>,
    pub redundancy: BTreeMap<String, usize>,
    pub recoverability: BTreeMap<String, usize>,
}

/// Aggregate patch/submission facts derived from compressed run records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunReviewPatchStats {
    pub patch_phase_count: usize,
    pub empty_submission_count: usize,
    pub nonempty_submission_count: usize,
    pub agent_turn_patch_artifact_count: usize,
    pub edit_proposal_count: usize,
    pub create_proposal_count: usize,
    pub expected_file_change_count: usize,
    pub applied_patch_artifact_count: usize,
    pub patch_projection_states: BTreeMap<String, usize>,
}

fn observe_protocol_labels(
    stats: &mut ProtocolReviewStats,
    overall: &(impl Serialize + std::fmt::Debug),
    redundancy: &(impl Serialize + std::fmt::Debug),
    recoverability: &(impl Serialize + std::fmt::Debug),
) {
    *stats.overall.entry(serde_label(overall)).or_default() += 1;
    *stats.redundancy.entry(serde_label(redundancy)).or_default() += 1;
    *stats
        .recoverability
        .entry(serde_label(recoverability))
        .or_default() += 1;
}

fn serde_label<T>(value: &T) -> String
where
    T: Serialize + std::fmt::Debug,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{value:?}"))
}
