// TODO(prototype1-history-traversal): split this file by responsibility before
// adding more traversal behavior. The current root carries strategy definitions,
// History candidate projection/filtering, candidate accessors, score assembly,
// sealed decision material, and tests. Move toward modules such as `strategy`,
// `candidate`, `score`, and `history` so new scoring sources do not turn this
// file into another flattened catch-all.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    HISTORY_TRAVERSAL_PROCEDURE_ID, PROCEDURE_ID, SelectionInput, SuccessorDecision,
    decide as decide_candidate, decision::SuccessorOutcome, disposition_as_str,
};
use crate::{
    BranchDisposition,
    cli::prototype1_state::history::{
        CandidateArtifact, CandidateSetCommitment, CandidateSetMembership, CandidateSetRoot,
        EvaluationPayload, HistoryCandidate, HistoryCandidateSource, HistoryCandidates,
        HistoryError, SealedCandidateEvidence, SealedComparedRunEvidence, SelectionDecisionEntry,
        SelectionProjectionFailure, SelectionProjectionFailureKind, SelectionScope, SubjectRef,
        TraversalCandidateSource,
    },
    metric::{self, Summary},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum StrategyKind {
    FrontierMax {
        normalize_frontier: bool,
        #[serde(default)]
        metrics: metric::Inputs,
    },
    ScoreChildProp {
        top_m: usize,
        lambda_millis: u32,
        #[serde(default)]
        metrics: metric::Inputs,
    },
}

impl StrategyKind {
    pub(crate) fn score_child_prop() -> Self {
        ScoreChildProp::default().evidence()
    }

    pub(crate) fn with_metrics(self, metrics: metric::Inputs) -> Self {
        match self {
            Self::FrontierMax {
                normalize_frontier, ..
            } => Self::FrontierMax {
                normalize_frontier,
                metrics,
            },
            Self::ScoreChildProp {
                top_m,
                lambda_millis,
                ..
            } => Self::ScoreChildProp {
                top_m,
                lambda_millis,
                metrics,
            },
        }
    }
}

impl Default for StrategyKind {
    fn default() -> Self {
        Self::FrontierMax {
            normalize_frontier: true,
            metrics: metric::Inputs::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrontierMax {
    normalize_frontier: bool,
    metrics: metric::Inputs,
}

impl Default for FrontierMax {
    fn default() -> Self {
        Self {
            normalize_frontier: true,
            metrics: metric::Inputs::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScoreChildProp {
    top_m: usize,
    lambda_millis: u32,
    metrics: metric::Inputs,
}

impl Default for ScoreChildProp {
    fn default() -> Self {
        Self {
            top_m: 3,
            lambda_millis: 10_000,
            metrics: metric::Inputs::default(),
        }
    }
}

pub(crate) trait Strategy: Copy {
    type Item;

    fn evidence(self) -> StrategyKind;

    fn select(
        self,
        items: &[Self::Item],
        child_counts: &BTreeMap<String, usize>,
        seed: u64,
    ) -> Result<Option<StrategySelection>, HistoryError>;
}

impl Strategy for FrontierMax {
    type Item = Item;

    fn evidence(self) -> StrategyKind {
        StrategyKind::FrontierMax {
            normalize_frontier: self.normalize_frontier,
            metrics: self.metrics,
        }
    }

    fn select(
        self,
        items: &[Self::Item],
        child_counts: &BTreeMap<String, usize>,
        seed: u64,
    ) -> Result<Option<StrategySelection>, HistoryError> {
        select_frontier_max(
            items,
            child_counts,
            seed,
            self.normalize_frontier,
            self.metrics,
        )
    }
}

impl Strategy for ScoreChildProp {
    type Item = Item;

    fn evidence(self) -> StrategyKind {
        StrategyKind::ScoreChildProp {
            top_m: self.top_m,
            lambda_millis: self.lambda_millis,
            metrics: self.metrics,
        }
    }

    fn select(
        self,
        items: &[Self::Item],
        child_counts: &BTreeMap<String, usize>,
        seed: u64,
    ) -> Result<Option<StrategySelection>, HistoryError> {
        select_score_child_prop(
            items,
            child_counts,
            seed,
            self.top_m,
            self.lambda_millis,
            self.metrics,
        )
    }
}

pub(crate) trait Traversal<S>
where
    S: Strategy<Item = Self::Item>,
{
    type Candidate;
    type Item;
    type Selection;

    fn traverse(self, seed: u64, strategy: S) -> Result<Option<Self::Selection>, HistoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Selection {
    pub(crate) decision: SuccessorDecision,
    pub(crate) selected_payload: EvaluationPayload,
    pub(crate) considered: Vec<EvaluationPayload>,
    pub(crate) considered_sources: Vec<TraversalCandidateSource>,
    pub(crate) projection_failures: Vec<SelectionProjectionFailure>,
    pub(crate) selected_from_current_generation: bool,
}

#[cfg(test)]
pub(crate) fn select_from_history(
    history: HistoryCandidates,
    seed: u64,
    strategy: StrategyKind,
) -> Result<Option<Selection>, HistoryError> {
    select(Candidates::from_history(history), seed, strategy)
}

pub(crate) fn select(
    candidates: Candidates,
    seed: u64,
    strategy: StrategyKind,
) -> Result<Option<Selection>, HistoryError> {
    match strategy {
        StrategyKind::FrontierMax {
            normalize_frontier,
            metrics,
        } => candidates.traverse(
            seed,
            FrontierMax {
                normalize_frontier,
                metrics,
            },
        ),
        StrategyKind::ScoreChildProp {
            top_m,
            lambda_millis,
            metrics,
        } => candidates.traverse(
            seed,
            ScoreChildProp {
                top_m,
                lambda_millis,
                metrics,
            },
        ),
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ScoreChildPropReplay {
    pub(crate) seed: u64,
    pub(crate) top_m: usize,
    pub(crate) lambda: f64,
    pub(crate) metric_inputs: &'static str,
    pub(crate) total_weight: f64,
    pub(crate) sample: f64,
    pub(crate) selected_index: Option<usize>,
    pub(crate) selected_candidate: Option<String>,
    pub(crate) rows: Vec<ScoreChildPropReplayRow>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ScoreChildPropReplayRow {
    pub(crate) index: usize,
    pub(crate) candidate: String,
    pub(crate) node_id: Option<String>,
    pub(crate) branch_id: Option<String>,
    pub(crate) branch_disposition: Option<String>,
    pub(crate) base_outcome: SuccessorOutcome,
    pub(crate) performance: i64,
    pub(crate) child_count: usize,
    pub(crate) alpha: f64,
    pub(crate) exploitation: f64,
    pub(crate) exploration: f64,
    pub(crate) weight: f64,
    pub(crate) selected: bool,
}

pub(crate) fn replay_score_child_prop(
    entry: &SelectionDecisionEntry,
) -> Result<Option<ScoreChildPropReplay>, HistoryError> {
    let Some(traversal) = entry.traversal.as_ref() else {
        return Ok(None);
    };
    let StrategyKind::ScoreChildProp {
        top_m,
        lambda_millis,
        metrics,
    } = traversal.strategy
    else {
        return Ok(None);
    };
    let items = entry
        .considered
        .iter()
        .cloned()
        .map(|payload| Item {
            payload,
            source: Source::SealedDecisionReplay,
        })
        .collect::<Vec<_>>();
    let child_counts = successful_child_counts(&entry.considered);
    let weights = score_child_prop_weights(&items, &child_counts, top_m, lambda_millis, metrics);
    let total_weight = weights.iter().map(|weight| weight.weight).sum::<f64>();
    let sample = sample_unit(traversal.seed, &items)?;
    let selected_index = if weights.is_empty() {
        None
    } else {
        sample_weighted_index(&weights, total_weight, sample)
    };
    let selected_candidate = selected_index
        .and_then(|index| items.get(index))
        .map(|item| item.payload.candidate.as_str().to_string());
    let rows = weights
        .into_iter()
        .filter_map(|weight| {
            let item = items.get(weight.index)?;
            let case = CandidateCase::from_payload(&item.payload);
            let input = case.selection_input();
            Some(ScoreChildPropReplayRow {
                index: weight.index,
                candidate: item.payload.candidate.as_str().to_string(),
                node_id: input.map(|value| value.candidate.node_id.clone()),
                branch_id: input.map(|value| value.candidate.branch_id.clone()),
                branch_disposition: input
                    .map(|value| disposition_as_str(value.branch_disposition.clone()).to_string()),
                base_outcome: weight.decision.outcome,
                performance: weight.performance.0,
                child_count: weight.child_count,
                alpha: weight.alpha,
                exploitation: weight.exploitation,
                exploration: weight.exploration,
                weight: weight.weight,
                selected: selected_index == Some(weight.index),
            })
        })
        .collect();
    Ok(Some(ScoreChildPropReplay {
        seed: traversal.seed,
        top_m,
        lambda: lambda_millis as f64 / 1_000.0,
        metric_inputs: metric_inputs_name(metrics),
        total_weight,
        sample,
        selected_index,
        selected_candidate,
        rows,
    }))
}

impl<S> Traversal<S> for Candidates
where
    S: Strategy<Item = Item>,
{
    type Candidate = Candidate;
    type Item = Item;
    type Selection = Selection;

    fn traverse(self, seed: u64, strategy: S) -> Result<Option<Selection>, HistoryError> {
        let mut items = Vec::new();
        let mut failures = Vec::new();

        for candidate in self.candidates {
            let source = candidate.source.clone();
            match decision_grade(candidate)? {
                CandidateGrade::Eligible(payload) => {
                    items.push(Item { payload, source });
                }
                CandidateGrade::Excluded(failure) => failures.push(failure),
            }
        }

        let considered = items
            .iter()
            .map(|item| item.payload.clone())
            .collect::<Vec<_>>();
        let considered_sources = items
            .iter()
            .map(|item| item.source.traversal_candidate_source())
            .collect::<Vec<_>>();
        let child_counts = successful_child_counts(&considered);
        let evidence_summary = CandidateCaseEvidenceSummary::from_considered(&considered);
        let Some(selection) = strategy.select(&items, &child_counts, seed)? else {
            return Ok(None);
        };

        let mut decision = selection.chosen.decision;
        decision.procedure_id = HISTORY_TRAVERSAL_PROCEDURE_ID.to_string();
        decision.rationale.push(format!(
            "history traversal selected from {} decision-grade candidates with seed={}",
            considered.len(),
            seed
        ));
        decision.rationale.push(evidence_summary.rationale());
        decision.rationale.extend(selection.rationale);

        Ok(Some(Selection {
            decision,
            selected_payload: selection.chosen.payload,
            considered,
            considered_sources,
            projection_failures: failures,
            selected_from_current_generation: selection.chosen.source.is_current_generation(),
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidates {
    pub(crate) scope: SelectionScope,
    pub(crate) candidates: Vec<Candidate>,
}

impl Candidates {
    pub(crate) fn from_history(history: HistoryCandidates) -> Self {
        Self {
            scope: history.scope,
            candidates: history
                .candidates
                .into_iter()
                .map(Candidate::from)
                .collect(),
        }
    }

    pub(crate) fn with_current_generation(
        mut self,
        scope: SelectionScope,
        payloads: Vec<EvaluationPayload>,
    ) -> Result<Self, HistoryError> {
        let candidate_set = CandidateSetCommitment::from_payloads(&payloads)?;
        let root = candidate_set.root.clone();
        for payload in payloads {
            let payload_hash = payload.payload_hash()?;
            let membership = candidate_set.membership(&payload.candidate).cloned();
            self.candidates.push(Candidate {
                source: Source::CurrentGeneration {
                    scope: scope.clone(),
                },
                decision_scope: scope.clone(),
                selected_by_decision: false,
                payload,
                payload_hash,
                candidate_set_root: Some(root.clone()),
                candidate_set_membership: membership,
            });
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) source: Source,
    pub(crate) decision_scope: SelectionScope,
    pub(crate) selected_by_decision: bool,
    pub(crate) payload: EvaluationPayload,
    pub(crate) payload_hash: crate::cli::prototype1_state::history::HistoryHash,
    pub(crate) candidate_set_root: Option<CandidateSetRoot>,
    pub(crate) candidate_set_membership: Option<CandidateSetMembership>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    pub(crate) payload: EvaluationPayload,
    pub(crate) source: Source,
}

impl From<HistoryCandidate> for Candidate {
    fn from(candidate: HistoryCandidate) -> Self {
        Self {
            source: Source::History {
                source: candidate.source,
            },
            decision_scope: candidate.decision_scope,
            selected_by_decision: candidate.selected_by_decision,
            payload: candidate.payload,
            payload_hash: candidate.payload_hash,
            candidate_set_root: candidate.candidate_set_root,
            candidate_set_membership: candidate.candidate_set_membership,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    History { source: HistoryCandidateSource },
    CurrentGeneration { scope: SelectionScope },
    SealedDecisionReplay,
}

impl Source {
    fn is_current_generation(&self) -> bool {
        matches!(self, Self::CurrentGeneration { .. })
    }

    fn traversal_candidate_source(&self) -> TraversalCandidateSource {
        match self {
            Self::CurrentGeneration { .. } => TraversalCandidateSource::CurrentGeneration,
            Self::History { .. } | Self::SealedDecisionReplay => TraversalCandidateSource::History,
        }
    }
}

/// Borrowed selection-time view over a sealed candidate payload.
///
/// This keeps the candidate universe richer than the current scorer. Policies
/// can opt into optional sections by reading them from the case, while the
/// sealed History payload remains the replayable source of facts.
///
/// Scoring inventory, grouped by signal:
///
/// Identity and lineage:
/// `candidate`
/// `selection_input.candidate.node_id`
/// `selection_input.candidate.branch_id`
/// `selection_input.candidate.generation`
/// `sealed_evidence.coordinate.node_id`
/// `sealed_evidence.coordinate.parent_node_id`
/// `sealed_evidence.coordinate.branch_id`
/// `sealed_evidence.coordinate.generation`
/// `sealed_evidence.coordinate.plan_index`
/// `sealed_evidence.coordinate.primary_runtime_id`
/// `sealed_evidence.evaluations[*].branch_id`
/// `sealed_evidence.runtimes[*].runtime_id`
/// `sealed_evidence.branches[*].branch_id`
/// `sealed_evidence.branches[*].candidate_id`
/// `sealed_evidence.branches[*].source_state_id`
///
/// Policy, lifecycle, and evaluation outcome:
/// `selection_input.branch_disposition`
/// `selection_input.evaluation_artifact_path`
/// `selection_input.comparisons[*].instance_id`
/// `selection_input.comparisons[*].status`
/// `sealed_evidence.lifecycle.planner_outcome`
/// `sealed_evidence.lifecycle.node_status`
/// `sealed_evidence.evaluations[*].evaluation_procedure_id`
/// `sealed_evidence.evaluations[*].evaluator_identity`
/// `sealed_evidence.evaluations[*].eval_set_identity`
/// `sealed_evidence.evaluations[*].overall_disposition`
/// `sealed_evidence.evaluations[*].compared_runs[*].instance_id`
/// `sealed_evidence.evaluations[*].compared_runs[*].status`
///
/// Metric-bearing procedure states:
/// `selection_input.comparisons[*].parent_metrics`
/// `selection_input.comparisons[*].child_metrics`
/// `sealed_evidence.evaluations[*].compared_runs[*].baseline_metrics`
/// `sealed_evidence.evaluations[*].compared_runs[*].treatment_metrics`
/// `metric::Operational.tool_calls_total`
/// `metric::Operational.tool_calls_failed`
/// `metric::Operational.patch_attempted`
/// `metric::Operational.patch_apply_state`
/// `metric::Operational.submission_artifact_state`
/// `metric::Operational.partial_patch_failures`
/// `metric::Operational.same_file_patch_retry_count`
/// `metric::Operational.same_file_patch_max_streak`
/// `metric::Operational.aborted`
/// `metric::Operational.aborted_repair_loop`
/// `metric::Operational.nonempty_valid_patch`
/// `metric::Operational.convergence`
/// `metric::Operational.oracle_eligible`
/// `sealed_evidence.evaluations[*].compared_runs[*].baseline_protocol`
/// `sealed_evidence.evaluations[*].compared_runs[*].treatment_protocol`
/// `metric::Protocol.reviewed_call_count`
/// `metric::Protocol.reviewed_segment_count`
/// `metric::Protocol.missing_call_count`
/// `metric::Protocol.missing_segment_count`
/// `metric::Protocol.skipped_segment_review_count`
/// `metric::Protocol.segment_anchor_mismatch_count`
/// `metric::Protocol.calls_with_segment_crosswalk`
/// `metric::Protocol.calls_without_segment_crosswalk`
/// `metric::Protocol.call_review_overall_counts`
/// `metric::Protocol.segment_review_overall_counts`
/// `metric::Protocol.review_signal_totals`
///
/// Rich run snapshots:
/// `sealed_evidence.evaluations[*].compared_runs[*].baseline_run`
/// `sealed_evidence.evaluations[*].compared_runs[*].treatment_run`
///
/// Citations, diagnostics, and provenance:
/// `sealed_evidence.evaluations[*].evaluation_artifact_citation`
/// `sealed_evidence.evaluations[*].primary_report_citation`
/// `sealed_evidence.evaluations[*].compared_runs[*].baseline_citation`
/// `sealed_evidence.evaluations[*].compared_runs[*].treatment_citation`
/// `sealed_evidence.evaluations[*].compared_runs[*].diagnostics`
/// `sealed_evidence.runtimes[*].document_citations`
/// `sealed_evidence.runtimes[*].journal_citations`
/// `sealed_evidence.branches[*].branch_evidence_citations`
/// `sealed_evidence.extra_document_citations`
/// `sealed_evidence.extra_journal_citations`
/// `sealed_evidence.child_diagnostics`
/// `payload.source_refs`
/// `payload.source_hashes`
/// `payload.projection_failures`
///
/// Artifact handoff:
/// `artifact.node`
/// `artifact.resolved`
///
/// Traversal provenance:
/// `Item::source`
///
/// Candidate-set root and membership proofs are validated before a payload
/// becomes an `Item`; those proof objects are not currently exposed to strategy
/// code.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CandidateCase<'a> {
    candidate: &'a SubjectRef,
    selection_input: Option<&'a SelectionInput>,
    sealed_evidence: Option<&'a SealedCandidateEvidence>,
    artifact: Option<&'a CandidateArtifact>,
}

impl<'a> CandidateCase<'a> {
    pub(crate) fn builder(payload: &'a EvaluationPayload) -> CandidateCaseBuilder<'a> {
        CandidateCaseBuilder {
            payload,
            selection_input: None,
            sealed_evidence: None,
            artifact: None,
        }
    }

    pub(crate) fn from_payload(payload: &'a EvaluationPayload) -> Self {
        Self::builder(payload)
            .selection_input(payload.selection_input.as_ref())
            .sealed_evidence(payload.sealed_evidence.as_ref())
            .artifact(payload.artifact.as_ref())
            .build()
    }

    pub(crate) fn candidate(&self) -> &'a SubjectRef {
        self.candidate
    }

    pub(crate) fn selection_input(&self) -> Option<&'a SelectionInput> {
        self.selection_input
    }

    pub(crate) fn sealed_evidence(&self) -> Option<&'a SealedCandidateEvidence> {
        self.sealed_evidence
    }

    pub(crate) fn artifact(&self) -> Option<&'a CandidateArtifact> {
        self.artifact
    }

    fn parent_node_id(&self) -> Option<&'a str> {
        self.sealed_evidence
            .and_then(|sealed| sealed.coordinate.parent_node_id.as_deref())
    }

    pub(crate) fn compared_runs(&self) -> impl Iterator<Item = &'a SealedComparedRunEvidence> + 'a {
        self.sealed_evidence
            .into_iter()
            .flat_map(|sealed| sealed.evaluations.iter())
            .flat_map(|evaluation| evaluation.compared_runs.iter())
    }

    pub(crate) fn protocol_run_snapshot_count(&self) -> usize {
        self.compared_runs()
            .flat_map(|run| [run.baseline_run.as_ref(), run.treatment_run.as_ref()])
            .flatten()
            .filter(|snapshot| run_snapshot_has_protocol(snapshot))
            .count()
    }
}

pub(crate) struct CandidateCaseBuilder<'a> {
    payload: &'a EvaluationPayload,
    selection_input: Option<&'a SelectionInput>,
    sealed_evidence: Option<&'a SealedCandidateEvidence>,
    artifact: Option<&'a CandidateArtifact>,
}

impl<'a> CandidateCaseBuilder<'a> {
    pub(crate) fn selection_input(mut self, value: Option<&'a SelectionInput>) -> Self {
        self.selection_input = value;
        self
    }

    pub(crate) fn sealed_evidence(mut self, value: Option<&'a SealedCandidateEvidence>) -> Self {
        self.sealed_evidence = value;
        self
    }

    pub(crate) fn artifact(mut self, value: Option<&'a CandidateArtifact>) -> Self {
        self.artifact = value;
        self
    }

    pub(crate) fn build(self) -> CandidateCase<'a> {
        CandidateCase {
            candidate: &self.payload.candidate,
            selection_input: self.selection_input,
            sealed_evidence: self.sealed_evidence,
            artifact: self.artifact,
        }
    }
}

enum CandidateGrade {
    Eligible(EvaluationPayload),
    Excluded(SelectionProjectionFailure),
}

fn decision_grade(candidate: Candidate) -> Result<CandidateGrade, HistoryError> {
    let subject = candidate.payload.candidate.clone();
    let exclude = |kind, detail: String| {
        SelectionProjectionFailure::committed(kind, Some(subject.clone()), Some(detail))
            .map(CandidateGrade::Excluded)
    };

    if candidate.payload.procedure.as_str() != PROCEDURE_ID {
        return exclude(
            SelectionProjectionFailureKind::SelectionProcedureMismatch,
            format!(
                "selection_procedure_mismatch:want={PROCEDURE_ID},got={}",
                candidate.payload.procedure.as_str()
            ),
        );
    }

    if candidate.payload.selection_input.is_none() {
        return exclude(
            SelectionProjectionFailureKind::MissingSelectionInput,
            "traversal: missing SelectionInput".to_string(),
        );
    }
    if !candidate.payload.verify_selection_input_binding()? {
        return exclude(
            SelectionProjectionFailureKind::SelectionInputBindingInvalid,
            "traversal: SelectionInput hash binding invalid".to_string(),
        );
    }

    let Some(root) = candidate.candidate_set_root.as_ref() else {
        return exclude(
            SelectionProjectionFailureKind::CandidateSetMembershipMissing,
            "traversal: missing candidate-set root".to_string(),
        );
    };
    let Some(membership) = candidate.candidate_set_membership.as_ref() else {
        return exclude(
            SelectionProjectionFailureKind::CandidateSetMembershipMissing,
            "traversal: missing candidate-set membership proof".to_string(),
        );
    };
    if membership.candidate != candidate.payload.candidate
        || membership.payload_hash != candidate.payload_hash
    {
        return exclude(
            SelectionProjectionFailureKind::CandidateSetPayloadHashMismatch,
            "traversal: candidate-set membership does not bind this payload hash".to_string(),
        );
    }
    if !membership.proof.verify(root)? {
        return exclude(
            SelectionProjectionFailureKind::CandidateSetProofInvalid,
            "traversal: candidate-set proof verification failed".to_string(),
        );
    }

    let grade = candidate.payload.decision_grade_eligibility();
    if !grade.eligible {
        return exclude(
            SelectionProjectionFailureKind::DecisionGradeIneligible,
            format!("traversal: {}", grade.identity_gaps.join(",")),
        );
    }

    Ok(CandidateGrade::Eligible(candidate.payload))
}

fn traversal_decision(case: &CandidateCase<'_>) -> Option<SuccessorDecision> {
    let input = case.selection_input()?;
    let mut decision = decide_candidate(input.clone());
    if decision.selected_branch_id.is_none() {
        decision.selected_branch_id = Some(input.candidate.branch_id.clone());
        decision.rationale.push(format!(
            "history traversal selected candidate as successor coordinate from generation {} with base_outcome={:?}",
            input.candidate.generation,
            decision.outcome
        ));
    }
    Some(decision)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PerformanceScore(i64);

fn performance_score(case: CandidateCase<'_>, metrics: metric::Inputs) -> Option<PerformanceScore> {
    let input = case.selection_input()?;
    let decision = decide_candidate(input.clone());
    let mut score = match decision.outcome {
        SuccessorOutcome::Accepted => 10_000,
        SuccessorOutcome::ExploreFrom => 5_000,
        SuccessorOutcome::Stop if input.branch_disposition == BranchDisposition::Reject => 2_500,
        SuccessorOutcome::Stop => 0,
    };

    for metrics in input
        .comparisons
        .iter()
        .filter_map(|comparison| comparison.child_metrics.as_ref())
    {
        score += metrics.selection_points();
    }
    if metrics.includes_protocol() {
        for run in case.compared_runs() {
            if let Some(treatment) = run.treatment_protocol.as_ref() {
                score += treatment.selection_delta_points(run.baseline_protocol.as_ref());
            }
        }
    }

    Some(PerformanceScore(score))
}

fn successful_child_counts(considered: &[EvaluationPayload]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for payload in considered {
        let case = CandidateCase::from_payload(payload);
        if traversal_decision(&case).is_none() {
            continue;
        }
        let Some(parent_node_id) = case.parent_node_id() else {
            continue;
        };
        *counts.entry(parent_node_id.to_string()).or_default() += 1;
    }
    counts
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TraversalScore {
    performance: PerformanceScore,
    frontier_delta: i64,
    exploration_pressure: usize,
    generation: u32,
}

impl TraversalScore {
    fn for_case(
        case: &CandidateCase<'_>,
        child_counts: &BTreeMap<String, usize>,
        max_performance: Option<PerformanceScore>,
        metrics: metric::Inputs,
    ) -> Option<Self> {
        let input = case.selection_input()?;
        let performance = performance_score(*case, metrics)?;
        let frontier_delta = max_performance
            .map(|max| performance.0 - max.0)
            .unwrap_or_default();
        let own_child_count = child_counts
            .get(&input.candidate.node_id)
            .copied()
            .unwrap_or_default();
        let parent_child_count = case
            .parent_node_id()
            .and_then(|parent_node_id| child_counts.get(parent_node_id).copied())
            .unwrap_or_default();
        let child_count = own_child_count.saturating_add(parent_child_count);
        Some(Self {
            performance,
            frontier_delta,
            exploration_pressure: usize::MAX.saturating_sub(child_count),
            generation: input.candidate.generation,
        })
    }
}

pub(crate) struct StrategySelection {
    chosen: ChosenPayload,
    rationale: Vec<String>,
}

struct ChosenPayload {
    payload: EvaluationPayload,
    decision: SuccessorDecision,
    source: Source,
}

fn select_frontier_max(
    items: &[Item],
    child_counts: &BTreeMap<String, usize>,
    seed: u64,
    normalize_frontier: bool,
    metrics: metric::Inputs,
) -> Result<Option<StrategySelection>, HistoryError> {
    let max_performance = if normalize_frontier {
        items
            .iter()
            .map(|item| CandidateCase::from_payload(&item.payload))
            .filter_map(|case| performance_score(case, metrics))
            .max()
    } else {
        None
    };

    let mut best = None::<ScoredPayload>;
    for (index, item) in items.iter().enumerate() {
        let payload = item.payload.clone();
        let case = CandidateCase::from_payload(&payload);
        let Some(selected) = traversal_decision(&case) else {
            continue;
        };
        let Some(score) = TraversalScore::for_case(&case, child_counts, max_performance, metrics)
        else {
            continue;
        };
        let tie = tie_break_key(seed, index, &payload)?;
        let scored = ScoredPayload {
            payload,
            score,
            tie,
            decision: selected,
            source: item.source.clone(),
        };
        if best
            .as_ref()
            .is_none_or(|current| scored.orders_after(current))
        {
            best = Some(scored);
        }
    }

    Ok(best.map(|best| {
        let mut rationale = vec![
            "traversal_strategy=frontier_max".to_string(),
            format!("frontier_max_normalize_frontier={normalize_frontier}"),
            format!("metric_inputs={}", metric_inputs_name(metrics)),
        ];
        if let Some(max_performance) = max_performance {
            rationale.push(format!(
                "frontier_normalization=max_performance_score={}",
                max_performance.0
            ));
        }
        StrategySelection {
            chosen: ChosenPayload {
                payload: best.payload,
                decision: best.decision,
                source: best.source,
            },
            rationale,
        }
    }))
}

#[derive(Debug, Clone)]
struct ScoreChildPropWeight {
    index: usize,
    performance: PerformanceScore,
    child_count: usize,
    alpha: f64,
    exploitation: f64,
    exploration: f64,
    weight: f64,
    decision: SuccessorDecision,
}

fn select_score_child_prop(
    items: &[Item],
    child_counts: &BTreeMap<String, usize>,
    seed: u64,
    top_m: usize,
    lambda_millis: u32,
    metrics: metric::Inputs,
) -> Result<Option<StrategySelection>, HistoryError> {
    let weights = score_child_prop_weights(items, child_counts, top_m, lambda_millis, metrics);
    if weights.is_empty() {
        return Ok(None);
    }

    let total_weight: f64 = weights.iter().map(|weight| weight.weight).sum();
    let sample = sample_unit(seed, items)?;
    let selected = sample_weighted_index(&weights, total_weight, sample)
        .unwrap_or_else(|| weights.last().expect("nonempty weights").index);
    let weight = weights
        .iter()
        .find(|weight| weight.index == selected)
        .expect("sampled weight");
    let payload = items[selected].payload.clone();
    let chosen = ChosenPayload {
        payload,
        decision: weight.decision.clone(),
        source: items[selected].source.clone(),
    };
    let lambda = lambda_millis as f64 / 1_000.0;
    let rationale = vec![
        "traversal_strategy=score_child_prop".to_string(),
        format!("score_child_prop_top_m={top_m}"),
        format!("score_child_prop_lambda={lambda:.3}"),
        format!("metric_inputs={}", metric_inputs_name(metrics)),
        format!("score_child_prop_total_weight={total_weight:.9}"),
        format!("score_child_prop_sample={sample:.9}"),
        format!("score_child_prop_selected_weight={:.9}", weight.weight),
        format!(
            "score_child_prop_selected_components=performance={},child_count={},alpha={:.9},exploitation={:.9},exploration={:.9}",
            weight.performance.0,
            weight.child_count,
            weight.alpha,
            weight.exploitation,
            weight.exploration
        ),
    ];

    Ok(Some(StrategySelection { chosen, rationale }))
}

fn score_child_prop_weights(
    items: &[Item],
    child_counts: &BTreeMap<String, usize>,
    top_m: usize,
    lambda_millis: u32,
    metrics: metric::Inputs,
) -> Vec<ScoreChildPropWeight> {
    let mut selectable = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let case = CandidateCase::from_payload(&item.payload);
        let Some(decision) = traversal_decision(&case) else {
            continue;
        };
        let Some(performance) = performance_score(case, metrics) else {
            continue;
        };
        let Some(input) = case.selection_input() else {
            continue;
        };
        let child_count = child_counts
            .get(&input.candidate.node_id)
            .copied()
            .unwrap_or_default();
        selectable.push((index, performance, child_count, decision));
    }

    if selectable.is_empty() {
        return Vec::new();
    }

    let min = selectable
        .iter()
        .map(|(_, performance, _, _)| performance.0)
        .min()
        .expect("nonempty selectable");
    let max = selectable
        .iter()
        .map(|(_, performance, _, _)| performance.0)
        .max()
        .expect("nonempty selectable");
    let span = max.saturating_sub(min);
    let alpha = |performance: PerformanceScore| {
        if span == 0 {
            0.5
        } else {
            (performance.0 - min) as f64 / span as f64
        }
    };

    let mut ranked_alpha: Vec<f64> = selectable
        .iter()
        .map(|(_, performance, _, _)| alpha(*performance))
        .collect();
    ranked_alpha.sort_by(|a, b| b.total_cmp(a));
    let frontier_count = top_m.max(1).min(ranked_alpha.len());
    let alpha_mid = ranked_alpha
        .iter()
        .take(frontier_count)
        .copied()
        .sum::<f64>()
        / frontier_count as f64;
    let lambda = lambda_millis as f64 / 1_000.0;

    selectable
        .into_iter()
        .map(|(index, performance, child_count, decision)| {
            let alpha = alpha(performance);
            let exploitation = sigmoid(lambda * (alpha - alpha_mid));
            let exploration = 1.0 / (1.0 + child_count as f64);
            let weight = exploitation * exploration;
            ScoreChildPropWeight {
                index,
                performance,
                child_count,
                alpha,
                exploitation,
                exploration,
                weight,
                decision,
            }
        })
        .collect()
}

fn metric_inputs_name(inputs: metric::Inputs) -> &'static str {
    match inputs {
        metric::Inputs::Operational => "operational",
        metric::Inputs::OperationalAndProtocol => "operational_and_protocol",
    }
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn sample_unit(seed: u64, items: &[Item]) -> Result<f64, HistoryError> {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(b"successor-selection:history-traversal:score-child-prop:v1");
    for item in items {
        let payload = &item.payload;
        hasher.update(payload.payload_hash()?.as_str().as_bytes());
        hasher.update(payload.candidate.as_str().as_bytes());
    }
    let digest: [u8; 32] = hasher.finalize().into();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let value = u64::from_le_bytes(bytes);
    Ok(value as f64 / (u64::MAX as f64 + 1.0))
}

fn sample_weighted_index(
    weights: &[ScoreChildPropWeight],
    total_weight: f64,
    sample: f64,
) -> Option<usize> {
    if total_weight <= 0.0 || !total_weight.is_finite() {
        let slot = (sample * weights.len() as f64).floor() as usize;
        return weights
            .get(slot.min(weights.len().saturating_sub(1)))
            .map(|weight| weight.index);
    }

    let threshold = sample * total_weight;
    let mut cumulative = 0.0;
    for weight in weights {
        cumulative += weight.weight;
        if threshold <= cumulative {
            return Some(weight.index);
        }
    }
    weights.last().map(|weight| weight.index)
}

fn run_snapshot_has_protocol(snapshot: &serde_json::Value) -> bool {
    let Some(protocol) = snapshot.get("protocol") else {
        return false;
    };
    protocol
        .get("anchor_path")
        .is_some_and(|anchor| !anchor.is_null())
        || protocol
            .get("artifacts")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|artifacts| !artifacts.is_empty())
}

#[derive(Debug, Default)]
struct CandidateCaseEvidenceSummary {
    candidates: usize,
    sealed_candidates: usize,
    artifacts: usize,
    compared_runs: usize,
    protocol_run_snapshots: usize,
}

impl CandidateCaseEvidenceSummary {
    fn from_considered(considered: &[EvaluationPayload]) -> Self {
        let mut summary = Self::default();
        for payload in considered {
            let case = CandidateCase::from_payload(payload);
            if !case.candidate().as_str().is_empty() {
                summary.candidates += 1;
            }
            if case.sealed_evidence().is_some() {
                summary.sealed_candidates += 1;
            }
            if case.artifact().is_some() {
                summary.artifacts += 1;
            }
            summary.compared_runs += case.compared_runs().count();
            summary.protocol_run_snapshots += case.protocol_run_snapshot_count();
        }
        summary
    }

    fn rationale(&self) -> String {
        format!(
            "candidate_case_evidence=candidates={},sealed_candidates={},artifacts={},compared_runs={},protocol_run_snapshots={}",
            self.candidates,
            self.sealed_candidates,
            self.artifacts,
            self.compared_runs,
            self.protocol_run_snapshots
        )
    }
}

struct ScoredPayload {
    payload: EvaluationPayload,
    score: TraversalScore,
    tie: [u8; 32],
    decision: SuccessorDecision,
    source: Source,
}

impl ScoredPayload {
    fn orders_after(&self, other: &Self) -> bool {
        self.score > other.score || (self.score == other.score && self.tie > other.tie)
    }
}

fn tie_break_key(
    seed: u64,
    index: usize,
    payload: &EvaluationPayload,
) -> Result<[u8; 32], HistoryError> {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(index.to_le_bytes());
    hasher.update(payload.payload_hash()?.as_str().as_bytes());
    hasher.update(payload.candidate.as_str().as_bytes());
    Ok(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::record::SubmissionArtifactState;
    use crate::{
        OperationalRunMetrics, PatchApplyState,
        cli::prototype1_state::{
            evidence::PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID,
            history::{
                CandidateArtifact, CandidateCoordinate, CandidateLifecycle, HistoryCandidate,
                HistoryCandidateSource, HistoryCandidates, HistoryHash, LineageId, ProcedureRef,
                SealedCandidateEvidence, SealedComparedRunEvidence, SealedEvaluationEvidence,
                SealedEvidenceCitation, SelectionDecisionEntry, SelectionScope, SubjectRef,
            },
        },
        intervention::{
            Prototype1NodeRecord, Prototype1NodeStatus, ResolvedTreatmentBranch,
            TreatmentBranchNode, TreatmentBranchStatus,
        },
        successor_selection::{CandidateRef, RunComparison},
    };

    use super::*;

    #[test]
    fn traversal_excludes_missing_selection_input() {
        let candidates = HistoryCandidates {
            scope: SelectionScope::all_admitted_candidates(),
            candidates: vec![candidate_from_payload(payload_without_selection_input(
                "missing-input",
                "branch-missing",
                0,
            ))],
        };

        let selection =
            select_from_history(candidates, 0, StrategyKind::default()).expect("traversal");

        assert!(selection.is_none());
    }

    #[test]
    fn traversal_excludes_invalid_candidate_set_membership() {
        let mut candidate = candidate_from_payload(decision_grade_payload(
            "node-a",
            "branch-a",
            None,
            0,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        ));
        candidate.candidate_set_membership = None;

        let selection = select_from_history(
            HistoryCandidates {
                scope: SelectionScope::all_admitted_candidates(),
                candidates: vec![candidate],
            },
            0,
            StrategyKind::default(),
        )
        .expect("traversal");

        assert!(selection.is_none());
    }

    #[test]
    fn traversal_excludes_missing_candidate_artifact_before_selection() {
        let mut payload = decision_grade_payload(
            "node-a",
            "branch-a",
            None,
            0,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        );
        payload.artifact = None;
        let candidate = Candidate::from(candidate_from_payload(payload));

        let grade = decision_grade(candidate).expect("candidate grade");

        match grade {
            CandidateGrade::Excluded(failure) => {
                assert_eq!(
                    failure.kind,
                    SelectionProjectionFailureKind::DecisionGradeIneligible
                );
                assert!(
                    failure
                        .committed_message
                        .as_deref()
                        .is_some_and(|message| message.contains("missing_candidate_artifact")),
                    "unexpected failure message: {:?}",
                    failure.committed_message
                );
            }
            CandidateGrade::Eligible(_) => {
                panic!("candidate without Artifact payload must not be decision-grade")
            }
        }
    }

    #[test]
    fn traversal_excludes_non_decision_grade_evidence() {
        let candidate = candidate_from_payload(payload_without_sealed_evaluation(
            "node-a",
            "branch-a",
            0,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        ));

        let selection = select_from_history(
            HistoryCandidates {
                scope: SelectionScope::all_admitted_candidates(),
                candidates: vec![candidate],
            },
            0,
            StrategyKind::default(),
        )
        .expect("traversal");

        assert!(selection.is_none());
    }

    #[test]
    fn traversal_scores_high_performing_candidates_above_weak_candidates() {
        let weak = candidate_from_payload(decision_grade_payload(
            "node-weak",
            "branch-weak",
            None,
            0,
            BranchDisposition::Reject,
            metrics(false, false, 5),
        ));
        let strong = candidate_from_payload(decision_grade_payload(
            "node-strong",
            "branch-strong",
            None,
            1,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        ));

        let selection = select_from_history(
            HistoryCandidates {
                scope: SelectionScope::all_admitted_candidates(),
                candidates: vec![weak, strong],
            },
            0,
            StrategyKind::default(),
        )
        .expect("traversal")
        .expect("selection");

        assert_eq!(selection.decision.candidate_node_id, "node-strong");
        assert_eq!(
            selection.decision.procedure_id,
            HISTORY_TRAVERSAL_PROCEDURE_ID
        );
    }

    #[test]
    fn traversal_scores_current_generation_candidates_with_history_candidates() {
        let history = HistoryCandidates {
            scope: SelectionScope::all_admitted_candidates(),
            candidates: vec![candidate_from_payload(decision_grade_payload(
                "history-weak",
                "branch-history-weak",
                None,
                0,
                BranchDisposition::Reject,
                metrics(false, false, 5),
            ))],
        };
        let current = decision_grade_payload(
            "current-strong",
            "branch-current-strong",
            None,
            1,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        );

        let candidates = Candidates::from_history(history)
            .with_current_generation(
                SelectionScope::new("generation_local:current"),
                vec![current],
            )
            .expect("current generation candidates");
        let selection = select(candidates, 0, StrategyKind::default())
            .expect("traversal")
            .expect("selection");

        assert_eq!(selection.decision.candidate_node_id, "current-strong");
        assert!(selection.selected_from_current_generation);
    }

    #[test]
    fn candidate_case_exposes_optional_non_mechanized_sections() {
        let mut payload = decision_grade_payload(
            "node-rich",
            "branch-rich",
            None,
            0,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        );
        payload
            .sealed_evidence
            .as_mut()
            .expect("sealed evidence")
            .evaluations
            .first_mut()
            .expect("evaluation")
            .compared_runs
            .push(SealedComparedRunEvidence {
                instance_id: Some("instance-a".to_string()),
                status: Some("compared".to_string()),
                baseline_citation: None,
                treatment_citation: None,
                baseline_metrics: None,
                treatment_metrics: None,
                baseline_protocol: None,
                treatment_protocol: None,
                diagnostics: Vec::new(),
                baseline_run: Some(serde_json::json!({
                    "run_id": "baseline",
                    "protocol": {
                        "anchor_path": null,
                        "artifacts": []
                    }
                })),
                treatment_run: Some(serde_json::json!({
                    "run_id": "treatment",
                    "protocol": {
                        "anchor_path": "/tmp/protocol-anchor.json",
                        "artifacts": [
                            {"procedure_name": "tool-call-review"}
                        ]
                    }
                })),
            });

        let case = CandidateCase::from_payload(&payload);

        assert_eq!(payload.candidate, *case.candidate());
        assert!(case.selection_input().is_some());
        assert!(case.sealed_evidence().is_some());
        assert!(case.artifact().is_some());
        assert_eq!(case.compared_runs().count(), 1);
        assert_eq!(case.protocol_run_snapshot_count(), 1);
    }

    #[test]
    fn traversal_metric_inputs_gate_protocol_scoring() {
        let low = traversal_item(with_protocol(
            decision_grade_payload(
                "low-protocol",
                "branch-low-protocol",
                None,
                0,
                BranchDisposition::Keep,
                metrics(true, true, 0),
            ),
            protocol(1, 1, 2, 2, 0),
        ));
        let high = traversal_item(with_protocol(
            decision_grade_payload(
                "high-protocol",
                "branch-high-protocol",
                None,
                1,
                BranchDisposition::Keep,
                metrics(true, true, 0),
            ),
            protocol(3, 3, 0, 0, 2),
        ));
        let items = vec![low, high];
        let child_counts = BTreeMap::new();

        let operational = score_child_prop_weights(
            &items,
            &child_counts,
            3,
            10_000,
            metric::Inputs::Operational,
        );
        let with_protocol = score_child_prop_weights(
            &items,
            &child_counts,
            3,
            10_000,
            metric::Inputs::OperationalAndProtocol,
        );

        assert_eq!(operational[0].performance, operational[1].performance);
        assert!(with_protocol[1].performance > with_protocol[0].performance);
    }

    #[test]
    fn traversal_downweights_over_expanded_candidates() {
        let expanded = candidate_from_payload(decision_grade_payload(
            "expanded",
            "branch-expanded",
            None,
            0,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        ));
        let child_of_expanded = candidate_from_payload(decision_grade_payload(
            "expanded-child",
            "branch-expanded-child",
            Some("expanded"),
            1,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        ));
        let frontier = candidate_from_payload(decision_grade_payload(
            "frontier",
            "branch-frontier",
            None,
            2,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        ));

        let selection = select_from_history(
            HistoryCandidates {
                scope: SelectionScope::all_admitted_candidates(),
                candidates: vec![expanded, child_of_expanded, frontier],
            },
            0,
            StrategyKind::default(),
        )
        .expect("traversal")
        .expect("selection");

        assert_eq!(selection.decision.candidate_node_id, "frontier");
    }

    #[test]
    fn traversal_seed_is_replayable_for_same_candidate_set() {
        let candidates = HistoryCandidates {
            scope: SelectionScope::all_admitted_candidates(),
            candidates: vec![
                candidate_from_payload(decision_grade_payload(
                    "node-a",
                    "branch-a",
                    None,
                    0,
                    BranchDisposition::Keep,
                    metrics(true, true, 0),
                )),
                candidate_from_payload(decision_grade_payload(
                    "node-b",
                    "branch-b",
                    None,
                    1,
                    BranchDisposition::Keep,
                    metrics(true, true, 0),
                )),
            ],
        };

        let first = select_from_history(candidates.clone(), 42, StrategyKind::default())
            .expect("first")
            .expect("first selection");
        let second = select_from_history(candidates, 42, StrategyKind::default())
            .expect("second")
            .expect("second selection");

        assert_eq!(
            first.decision.candidate_node_id,
            second.decision.candidate_node_id
        );
    }

    #[test]
    fn score_child_prop_weights_follow_sigmoid_and_child_penalty() {
        let expanded = decision_grade_payload(
            "expanded",
            "branch-expanded",
            None,
            0,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        );
        let frontier = decision_grade_payload(
            "frontier",
            "branch-frontier",
            None,
            1,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        );
        let items = vec![traversal_item(expanded), traversal_item(frontier)];
        let mut child_counts = BTreeMap::new();
        child_counts.insert("expanded".to_string(), 1);

        let weights =
            score_child_prop_weights(&items, &child_counts, 3, 10_000, metric::Inputs::default());

        assert_eq!(weights.len(), 2);
        let expanded = weights.iter().find(|weight| weight.index == 0).unwrap();
        let frontier = weights.iter().find(|weight| weight.index == 1).unwrap();
        assert!((expanded.exploitation - frontier.exploitation).abs() < f64::EPSILON);
        assert_eq!(expanded.exploration, 0.5);
        assert_eq!(frontier.exploration, 1.0);
        assert!(frontier.weight > expanded.weight);
    }

    #[test]
    fn score_child_prop_sampling_is_replayable_for_same_candidate_set() {
        let candidates = HistoryCandidates {
            scope: SelectionScope::all_admitted_candidates(),
            candidates: vec![
                candidate_from_payload(decision_grade_payload(
                    "node-a",
                    "branch-a",
                    None,
                    0,
                    BranchDisposition::Keep,
                    metrics(true, true, 0),
                )),
                candidate_from_payload(decision_grade_payload(
                    "node-b",
                    "branch-b",
                    None,
                    1,
                    BranchDisposition::Keep,
                    metrics(false, true, 0),
                )),
            ],
        };
        let first = select_from_history(candidates.clone(), 99, StrategyKind::score_child_prop())
            .expect("first")
            .expect("first selection");
        let second = select_from_history(candidates, 99, StrategyKind::score_child_prop())
            .expect("second")
            .expect("second selection");

        assert_eq!(
            first.decision.candidate_node_id,
            second.decision.candidate_node_id
        );
        assert!(
            first
                .decision
                .rationale
                .iter()
                .any(|line| line == "traversal_strategy=score_child_prop")
        );
    }

    #[test]
    fn traversal_trait_accepts_concrete_strategy_carrier() {
        let candidates = HistoryCandidates {
            scope: SelectionScope::all_admitted_candidates(),
            candidates: vec![candidate_from_payload(decision_grade_payload(
                "node-a",
                "branch-a",
                None,
                0,
                BranchDisposition::Keep,
                metrics(true, true, 0),
            ))],
        };

        let selection = Candidates::from_history(candidates)
            .traverse(7, ScoreChildProp::default())
            .expect("traversal")
            .expect("selection");

        assert_eq!(selection.decision.candidate_node_id, "node-a");
    }

    fn candidate_from_payload(payload: EvaluationPayload) -> HistoryCandidate {
        let selected_candidate = payload
            .selection_input
            .as_ref()
            .map(|_| payload.candidate.clone());
        let entry = SelectionDecisionEntry::new(
            ProcedureRef::new(PROCEDURE_ID),
            SelectionScope::new("generation_local:test"),
            selected_candidate,
            vec![payload.clone()],
            Vec::new(),
            SuccessorDecision {
                procedure_id: PROCEDURE_ID.to_string(),
                candidate_node_id: payload
                    .selection_input
                    .as_ref()
                    .map(|input| input.candidate.node_id.clone())
                    .unwrap_or_else(|| "missing-input".to_string()),
                selected_branch_id: payload
                    .selection_input
                    .as_ref()
                    .map(|input| input.candidate.branch_id.clone()),
                branch_disposition: "keep".to_string(),
                outcome: if payload.selection_input.is_some() {
                    SuccessorOutcome::Accepted
                } else {
                    SuccessorOutcome::Stop
                },
                findings: Vec::new(),
                rationale: Vec::new(),
            },
        )
        .expect("selection entry");
        let payload_hash = payload.payload_hash().expect("payload hash");
        HistoryCandidate {
            source: HistoryCandidateSource {
                block_hash: HistoryHash::of_bytes(b"block").into(),
                block_height: 0,
                lineage_id: LineageId::new("lineage:test"),
                entry_id: crate::cli::prototype1_state::history::EntryId::new(),
            },
            decision_scope: SelectionScope::new("generation_local:test"),
            selected_by_decision: true,
            payload,
            payload_hash,
            candidate_set_root: entry
                .candidate_set
                .as_ref()
                .map(|commitment| commitment.root.clone()),
            candidate_set_membership: entry
                .candidate_set_membership(&entry.considered[0].candidate)
                .cloned(),
        }
    }

    fn traversal_item(payload: EvaluationPayload) -> Item {
        Item {
            payload,
            source: Source::CurrentGeneration {
                scope: SelectionScope::new("generation_local:test"),
            },
        }
    }

    fn with_protocol(
        mut payload: EvaluationPayload,
        treatment: metric::Protocol,
    ) -> EvaluationPayload {
        payload
            .sealed_evidence
            .as_mut()
            .expect("sealed evidence")
            .evaluations
            .first_mut()
            .expect("evaluation")
            .compared_runs
            .push(SealedComparedRunEvidence {
                instance_id: Some("instance-a".to_string()),
                status: Some("compared".to_string()),
                baseline_citation: None,
                treatment_citation: None,
                baseline_metrics: None,
                treatment_metrics: None,
                baseline_protocol: None,
                treatment_protocol: Some(treatment),
                diagnostics: Vec::new(),
                baseline_run: None,
                treatment_run: None,
            });
        payload
    }

    fn protocol(
        reviewed_call_count: usize,
        reviewed_segment_count: usize,
        missing_call_count: usize,
        missing_segment_count: usize,
        focused_progress: usize,
    ) -> metric::Protocol {
        metric::Protocol {
            scanned_artifact_count: 0,
            artifact_counts: BTreeMap::new(),
            total_calls_in_run: reviewed_call_count + missing_call_count,
            total_segments_in_anchor: reviewed_segment_count + missing_segment_count,
            reviewed_call_count,
            reviewed_segment_count,
            missing_call_count,
            missing_segment_count,
            skipped_segment_review_count: 0,
            segment_anchor_mismatch_count: 0,
            call_review_overall_counts: focused_progress_count(focused_progress),
            segment_review_overall_counts: focused_progress_count(focused_progress),
            call_review_confidence_counts: BTreeMap::new(),
            segment_review_confidence_counts: BTreeMap::new(),
            calls_with_segment_crosswalk: reviewed_call_count,
            calls_without_segment_crosswalk: 0,
            average_calls_per_anchor_segment_x1000: 0,
            review_signal_totals: BTreeMap::new(),
        }
    }

    fn focused_progress_count(value: usize) -> BTreeMap<String, usize> {
        BTreeMap::from([("focused_progress".to_string(), value)])
    }

    fn decision_grade_payload(
        node_id: &str,
        branch_id: &str,
        parent_node_id: Option<&str>,
        plan_index: u32,
        disposition: BranchDisposition,
        child: OperationalRunMetrics,
    ) -> EvaluationPayload {
        let input = SelectionInput::new(
            CandidateRef {
                node_id: node_id.to_string(),
                branch_id: branch_id.to_string(),
                generation: 1,
            },
            disposition,
            PathBuf::from(format!("evaluations/{branch_id}.json")),
            vec![RunComparison {
                instance_id: "instance-a".to_string(),
                parent_metrics: Some(metrics(false, false, 0)),
                child_metrics: Some(child),
                status: "compared".to_string(),
            }],
        );
        EvaluationPayload::builder(
            SubjectRef::new(format!("candidate:{node_id}:plan_index={plan_index}")),
            ProcedureRef::new(PROCEDURE_ID),
        )
        .selection_input(input)
        .expect("selection input")
        .sealed_candidate_evidence(sealed_evidence(
            node_id,
            branch_id,
            parent_node_id,
            plan_index,
            true,
        ))
        .candidate_artifact(candidate_artifact(node_id, branch_id))
        .build()
    }

    fn payload_without_selection_input(
        node_id: &str,
        branch_id: &str,
        plan_index: u32,
    ) -> EvaluationPayload {
        EvaluationPayload::builder(
            SubjectRef::new(format!("candidate:{node_id}:plan_index={plan_index}")),
            ProcedureRef::new(PROCEDURE_ID),
        )
        .sealed_candidate_evidence(sealed_evidence(node_id, branch_id, None, plan_index, true))
        .candidate_artifact(candidate_artifact(node_id, branch_id))
        .build()
    }

    fn payload_without_sealed_evaluation(
        node_id: &str,
        branch_id: &str,
        plan_index: u32,
        disposition: BranchDisposition,
        child: OperationalRunMetrics,
    ) -> EvaluationPayload {
        let input = SelectionInput::new(
            CandidateRef {
                node_id: node_id.to_string(),
                branch_id: branch_id.to_string(),
                generation: 1,
            },
            disposition,
            PathBuf::from(format!("evaluations/{branch_id}.json")),
            vec![RunComparison {
                instance_id: "instance-a".to_string(),
                parent_metrics: Some(metrics(false, false, 0)),
                child_metrics: Some(child),
                status: "compared".to_string(),
            }],
        );
        EvaluationPayload::builder(
            SubjectRef::new(format!("candidate:{node_id}:plan_index={plan_index}")),
            ProcedureRef::new(PROCEDURE_ID),
        )
        .selection_input(input)
        .expect("selection input")
        .sealed_candidate_evidence(sealed_evidence(node_id, branch_id, None, plan_index, false))
        .candidate_artifact(candidate_artifact(node_id, branch_id))
        .build()
    }

    fn sealed_evidence(
        node_id: &str,
        branch_id: &str,
        parent_node_id: Option<&str>,
        plan_index: u32,
        include_evaluation: bool,
    ) -> SealedCandidateEvidence {
        SealedCandidateEvidence {
            schema_version: 2,
            coordinate: CandidateCoordinate {
                node_id: node_id.to_string(),
                parent_node_id: parent_node_id.map(str::to_string),
                branch_id: Some(branch_id.to_string()),
                generation: Some(1),
                plan_index: Some(plan_index),
                primary_runtime_id: Some(format!("runtime:{node_id}")),
            },
            lifecycle: CandidateLifecycle {
                planner_outcome: "completed".to_string(),
                node_status: "completed".to_string(),
            },
            evaluations: include_evaluation
                .then(|| SealedEvaluationEvidence {
                    branch_id: branch_id.to_string(),
                    evaluation_procedure_id: Some(
                        PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID.to_string(),
                    ),
                    evaluator_identity: Some(serde_json::json!({"id":"test","version":"1"})),
                    eval_set_identity: Some(serde_json::json!({"id":"eval-set"})),
                    evaluation_artifact_citation: None,
                    overall_disposition: Some("keep".to_string()),
                    primary_report_citation: SealedEvidenceCitation {
                        ref_id: format!("report:{branch_id}"),
                        content_hash: None,
                        record_name: None,
                    },
                    compared_runs: Vec::new(),
                })
                .into_iter()
                .collect(),
            runtimes: Vec::new(),
            branches: Vec::new(),
            extra_document_citations: Vec::new(),
            extra_journal_citations: Vec::new(),
            child_diagnostics: Vec::new(),
        }
    }

    fn candidate_artifact(node_id: &str, branch_id: &str) -> CandidateArtifact {
        let candidate_id = format!("candidate-{node_id}");
        let target_relpath = PathBuf::from("crates/ploke-core/tool_text/read_file.md");
        let node = Prototype1NodeRecord {
            schema_version: "test-node.v1".to_string(),
            node_id: node_id.to_string(),
            parent_node_id: None,
            generation: 1,
            instance_id: "instance-a".to_string(),
            source_state_id: "source-a".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: None,
            branch_id: branch_id.to_string(),
            candidate_id: candidate_id.clone(),
            target_relpath: target_relpath.clone(),
            node_dir: PathBuf::from(format!("/tmp/{node_id}")),
            workspace_root: PathBuf::from(format!("/tmp/{node_id}/worktree")),
            binary_path: PathBuf::from(format!("/tmp/{node_id}/target/debug/ploke-eval")),
            runner_request_path: PathBuf::from(format!("/tmp/{node_id}/runner-request.json")),
            runner_result_path: PathBuf::from(format!("/tmp/{node_id}/runner-result.json")),
            status: Prototype1NodeStatus::Succeeded,
            created_at: "2026-05-06T00:00:00Z".to_string(),
            updated_at: "2026-05-06T00:00:00Z".to_string(),
        };
        let resolved = ResolvedTreatmentBranch {
            instance_id: node.instance_id.clone(),
            source_state_id: node.source_state_id.clone(),
            parent_branch_id: node.parent_branch_id.clone(),
            target_relpath,
            source_content: "old".to_string(),
            source_content_hash: "old-hash".to_string(),
            selected_branch_id: Some(branch_id.to_string()),
            branch: TreatmentBranchNode {
                branch_id: branch_id.to_string(),
                candidate_id,
                patch_id: None,
                branch_label: "test".to_string(),
                synthesized_spec_id: "spec".to_string(),
                proposed_content: "new".to_string(),
                proposed_content_hash: "new-hash".to_string(),
                generation_target: None,
                generation_coordinate: None,
                status: TreatmentBranchStatus::Selected,
                apply_id: None,
                applied_content_hash: None,
                derived_artifact_id: None,
                latest_evaluation: None,
            },
        };
        CandidateArtifact::new(node, resolved)
    }

    fn metrics(
        oracle_eligible: bool,
        convergence: bool,
        failed_tool_calls: usize,
    ) -> OperationalRunMetrics {
        OperationalRunMetrics {
            tool_calls_total: 5,
            tool_calls_failed: failed_tool_calls,
            patch_attempted: true,
            patch_apply_state: if convergence {
                PatchApplyState::Applied
            } else {
                PatchApplyState::No
            },
            submission_artifact_state: if oracle_eligible {
                SubmissionArtifactState::Nonempty
            } else {
                SubmissionArtifactState::Missing
            },
            partial_patch_failures: 0,
            same_file_patch_retry_count: 0,
            same_file_patch_max_streak: 0,
            aborted: false,
            aborted_repair_loop: false,
            nonempty_valid_patch: convergence,
            convergence,
            oracle_eligible,
        }
    }
}
