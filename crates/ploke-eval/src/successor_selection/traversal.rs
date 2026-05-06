use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    HISTORY_TRAVERSAL_PROCEDURE_ID, PROCEDURE_ID, SelectionInput, SuccessorDecision, decide,
    decision::SuccessorOutcome,
};
use crate::{
    BranchDisposition, OperationalRunMetrics,
    cli::prototype1_state::history::{
        CandidateSetCommitment, CandidateSetMembership, CandidateSetRoot, EvaluationPayload,
        HistoryCandidate, HistoryCandidateSource, HistoryCandidates, HistoryError,
        SelectionProjectionFailure, SelectionProjectionFailureKind, SelectionScope,
    },
    intervention::Prototype1SelectionPolicyOutcome,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HistoryTraversalConfig {
    pub(crate) seed: u64,
    pub(crate) normalize_frontier: bool,
}

impl Default for HistoryTraversalConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            normalize_frontier: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryTraversalSelection {
    pub(crate) decision: SuccessorDecision,
    pub(crate) selected_payload: EvaluationPayload,
    pub(crate) considered: Vec<EvaluationPayload>,
    pub(crate) projection_failures: Vec<SelectionProjectionFailure>,
    pub(crate) selected_from_current_generation: bool,
}

#[cfg(test)]
pub(crate) fn decide_history_traversal(
    history: HistoryCandidates,
    config: HistoryTraversalConfig,
) -> Result<Option<HistoryTraversalSelection>, HistoryError> {
    decide_traversal(TraversalCandidates::from_history(history), config)
}

pub(crate) fn decide_traversal(
    candidates: TraversalCandidates,
    config: HistoryTraversalConfig,
) -> Result<Option<HistoryTraversalSelection>, HistoryError> {
    let mut considered = Vec::new();
    let mut failures = Vec::new();
    let mut sources = Vec::new();

    for candidate in candidates.candidates {
        let source = candidate.source.clone();
        match decision_grade(candidate)? {
            CandidateGrade::Eligible(payload) => {
                considered.push(payload);
                sources.push(source);
            }
            CandidateGrade::Excluded(failure) => failures.push(failure),
        }
    }

    let child_counts = successful_child_counts(&considered);
    let max_performance = if config.normalize_frontier {
        considered
            .iter()
            .filter_map(|payload| payload.selection_input.as_ref())
            .map(performance_score)
            .max()
    } else {
        None
    };

    let mut best = None::<ScoredPayload>;
    for (index, payload) in considered.iter().cloned().enumerate() {
        let Some(input) = payload.selection_input.as_ref() else {
            continue;
        };
        let selected = selectable_decision(input);
        if selected
            .selection_policy_outcome()
            .is_none_or(|outcome| !outcome.allows_successor())
        {
            continue;
        }
        let score = TraversalScore::for_input(input, &child_counts, max_performance);
        let tie = tie_break_key(config.seed, index, &payload)?;
        let scored = ScoredPayload {
            payload,
            score,
            tie,
            decision: selected,
            source: sources[index].clone(),
        };
        if best
            .as_ref()
            .is_none_or(|current| scored.orders_after(current))
        {
            best = Some(scored);
        }
    }

    let Some(best) = best else {
        return Ok(None);
    };

    let mut decision = best.decision;
    decision.procedure_id = HISTORY_TRAVERSAL_PROCEDURE_ID.to_string();
    decision.rationale.push(format!(
        "history traversal selected from {} decision-grade candidates with seed={}",
        considered.len(),
        config.seed
    ));
    if let Some(max_performance) = max_performance {
        decision.rationale.push(format!(
            "frontier_normalization=max_performance_score={}",
            max_performance.0
        ));
    }

    Ok(Some(HistoryTraversalSelection {
        decision,
        selected_payload: best.payload,
        considered,
        projection_failures: failures,
        selected_from_current_generation: best.source.is_current_generation(),
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraversalCandidates {
    pub(crate) scope: SelectionScope,
    pub(crate) candidates: Vec<TraversalCandidate>,
}

impl TraversalCandidates {
    pub(crate) fn from_history(history: HistoryCandidates) -> Self {
        Self {
            scope: history.scope,
            candidates: history
                .candidates
                .into_iter()
                .map(TraversalCandidate::from)
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
            self.candidates.push(TraversalCandidate {
                source: TraversalCandidateSource::CurrentGeneration {
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
pub(crate) struct TraversalCandidate {
    pub(crate) source: TraversalCandidateSource,
    pub(crate) decision_scope: SelectionScope,
    pub(crate) selected_by_decision: bool,
    pub(crate) payload: EvaluationPayload,
    pub(crate) payload_hash: crate::cli::prototype1_state::history::HistoryHash,
    pub(crate) candidate_set_root: Option<CandidateSetRoot>,
    pub(crate) candidate_set_membership: Option<CandidateSetMembership>,
}

impl From<HistoryCandidate> for TraversalCandidate {
    fn from(candidate: HistoryCandidate) -> Self {
        Self {
            source: TraversalCandidateSource::History {
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
pub(crate) enum TraversalCandidateSource {
    History { source: HistoryCandidateSource },
    CurrentGeneration { scope: SelectionScope },
}

impl TraversalCandidateSource {
    fn is_current_generation(&self) -> bool {
        matches!(self, Self::CurrentGeneration { .. })
    }
}

enum CandidateGrade {
    Eligible(EvaluationPayload),
    Excluded(SelectionProjectionFailure),
}

fn decision_grade(candidate: TraversalCandidate) -> Result<CandidateGrade, HistoryError> {
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
            "history_traversal: missing SelectionInput".to_string(),
        );
    }
    if !candidate.payload.verify_selection_input_binding()? {
        return exclude(
            SelectionProjectionFailureKind::SelectionInputBindingInvalid,
            "history_traversal: SelectionInput hash binding invalid".to_string(),
        );
    }

    let Some(root) = candidate.candidate_set_root.as_ref() else {
        return exclude(
            SelectionProjectionFailureKind::CandidateSetMembershipMissing,
            "history_traversal: missing candidate-set root".to_string(),
        );
    };
    let Some(membership) = candidate.candidate_set_membership.as_ref() else {
        return exclude(
            SelectionProjectionFailureKind::CandidateSetMembershipMissing,
            "history_traversal: missing candidate-set membership proof".to_string(),
        );
    };
    if membership.candidate != candidate.payload.candidate
        || membership.payload_hash != candidate.payload_hash
    {
        return exclude(
            SelectionProjectionFailureKind::CandidateSetPayloadHashMismatch,
            "history_traversal: candidate-set membership does not bind this payload hash"
                .to_string(),
        );
    }
    if !membership.proof.verify(root)? {
        return exclude(
            SelectionProjectionFailureKind::CandidateSetProofInvalid,
            "history_traversal: candidate-set proof verification failed".to_string(),
        );
    }

    let grade = candidate.payload.decision_grade_eligibility();
    if !grade.eligible {
        return exclude(
            SelectionProjectionFailureKind::DecisionGradeIneligible,
            format!("history_traversal: {}", grade.identity_gaps.join(",")),
        );
    }

    Ok(CandidateGrade::Eligible(candidate.payload))
}

fn selectable_decision(input: &SelectionInput) -> SuccessorDecision {
    let mut decision = decide(input.clone());
    if decision.outcome == SuccessorOutcome::Stop
        && input.branch_disposition == BranchDisposition::Reject
    {
        decision.outcome = SuccessorOutcome::ExploreFrom;
        decision.selected_branch_id = Some(input.candidate.branch_id.clone());
        decision.rationale.push(format!(
            "history traversal selected rejected branch as exploration coordinate from generation {}",
            input.candidate.generation
        ));
    }
    decision
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PerformanceScore(i64);

fn performance_score(input: &SelectionInput) -> PerformanceScore {
    let decision = decide(input.clone());
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
        score += operational_points(metrics);
    }

    PerformanceScore(score)
}

fn operational_points(metrics: &OperationalRunMetrics) -> i64 {
    let mut score = 0;
    score += i64::from(metrics.oracle_eligible) * 400;
    score += i64::from(metrics.convergence) * 300;
    score += i64::from(metrics.nonempty_valid_patch) * 200;
    score += i64::from(metrics.patch_attempted) * 50;
    score -= (metrics.tool_calls_failed as i64) * 25;
    score -= (metrics.partial_patch_failures as i64) * 10;
    score -= i64::from(metrics.aborted) * 500;
    score -= i64::from(metrics.aborted_repair_loop) * 250;
    score
}

fn successful_child_counts(considered: &[EvaluationPayload]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for payload in considered {
        let Some(input) = payload.selection_input.as_ref() else {
            continue;
        };
        if selectable_decision(input)
            .selection_policy_outcome()
            .is_none_or(|outcome| !outcome.allows_successor())
        {
            continue;
        }
        let Some(parent_node_id) = payload
            .sealed_evidence
            .as_ref()
            .and_then(|sealed| sealed.coordinate.parent_node_id.as_ref())
        else {
            continue;
        };
        *counts.entry(parent_node_id.clone()).or_default() += 1;
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
    fn for_input(
        input: &SelectionInput,
        child_counts: &BTreeMap<String, usize>,
        max_performance: Option<PerformanceScore>,
    ) -> Self {
        let performance = performance_score(input);
        let frontier_delta = max_performance
            .map(|max| performance.0 - max.0)
            .unwrap_or_default();
        let child_count = child_counts
            .get(&input.candidate.node_id)
            .copied()
            .unwrap_or_default();
        Self {
            performance,
            frontier_delta,
            exploration_pressure: usize::MAX.saturating_sub(child_count),
            generation: input.candidate.generation,
        }
    }
}

struct ScoredPayload {
    payload: EvaluationPayload,
    score: TraversalScore,
    tie: [u8; 32],
    decision: SuccessorDecision,
    source: TraversalCandidateSource,
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

trait SelectionOutcomeExt {
    fn allows_successor(self) -> bool;
}

impl SelectionOutcomeExt for Prototype1SelectionPolicyOutcome {
    fn allows_successor(self) -> bool {
        matches!(
            self,
            Prototype1SelectionPolicyOutcome::Accepted
                | Prototype1SelectionPolicyOutcome::ExploreFromRejected
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::record::SubmissionArtifactState;
    use crate::{
        PatchApplyState,
        cli::prototype1_state::{
            evidence::PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID,
            history::{
                CandidateCoordinate, CandidateLifecycle, HistoryCandidate, HistoryCandidateSource,
                HistoryCandidates, HistoryHash, LineageId, ProcedureRef, SealedCandidateEvidence,
                SealedEvaluationEvidence, SealedEvidenceCitation, SelectionDecisionEntry,
                SelectionScope, SubjectRef,
            },
        },
        successor_selection::{CandidateRef, RunComparison},
    };

    use super::*;

    #[test]
    fn history_traversal_excludes_missing_selection_input() {
        let candidates = HistoryCandidates {
            scope: SelectionScope::all_admitted_candidates(),
            candidates: vec![candidate_from_payload(payload_without_selection_input(
                "missing-input",
                "branch-missing",
                0,
            ))],
        };

        let selection = decide_history_traversal(candidates, HistoryTraversalConfig::default())
            .expect("traversal");

        assert!(selection.is_none());
    }

    #[test]
    fn history_traversal_excludes_invalid_candidate_set_membership() {
        let mut candidate = candidate_from_payload(decision_grade_payload(
            "node-a",
            "branch-a",
            None,
            0,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        ));
        candidate.candidate_set_membership = None;

        let selection = decide_history_traversal(
            HistoryCandidates {
                scope: SelectionScope::all_admitted_candidates(),
                candidates: vec![candidate],
            },
            HistoryTraversalConfig::default(),
        )
        .expect("traversal");

        assert!(selection.is_none());
    }

    #[test]
    fn history_traversal_excludes_non_decision_grade_evidence() {
        let candidate = candidate_from_payload(payload_without_sealed_evaluation(
            "node-a",
            "branch-a",
            0,
            BranchDisposition::Keep,
            metrics(true, true, 0),
        ));

        let selection = decide_history_traversal(
            HistoryCandidates {
                scope: SelectionScope::all_admitted_candidates(),
                candidates: vec![candidate],
            },
            HistoryTraversalConfig::default(),
        )
        .expect("traversal");

        assert!(selection.is_none());
    }

    #[test]
    fn history_traversal_scores_high_performing_candidates_above_weak_candidates() {
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

        let selection = decide_history_traversal(
            HistoryCandidates {
                scope: SelectionScope::all_admitted_candidates(),
                candidates: vec![weak, strong],
            },
            HistoryTraversalConfig::default(),
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

        let candidates = TraversalCandidates::from_history(history)
            .with_current_generation(
                SelectionScope::new("generation_local:current"),
                vec![current],
            )
            .expect("current generation candidates");
        let selection = decide_traversal(candidates, HistoryTraversalConfig::default())
            .expect("traversal")
            .expect("selection");

        assert_eq!(selection.decision.candidate_node_id, "current-strong");
        assert!(selection.selected_from_current_generation);
    }

    #[test]
    fn history_traversal_downweights_over_expanded_candidates() {
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

        let selection = decide_history_traversal(
            HistoryCandidates {
                scope: SelectionScope::all_admitted_candidates(),
                candidates: vec![expanded, child_of_expanded, frontier],
            },
            HistoryTraversalConfig::default(),
        )
        .expect("traversal")
        .expect("selection");

        assert_eq!(selection.decision.candidate_node_id, "frontier");
    }

    #[test]
    fn history_traversal_seed_is_replayable_for_same_candidate_set() {
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

        let first = decide_history_traversal(
            candidates.clone(),
            HistoryTraversalConfig {
                seed: 42,
                normalize_frontier: true,
            },
        )
        .expect("first")
        .expect("first selection");
        let second = decide_history_traversal(
            candidates,
            HistoryTraversalConfig {
                seed: 42,
                normalize_frontier: true,
            },
        )
        .expect("second")
        .expect("second selection");

        assert_eq!(
            first.decision.candidate_node_id,
            second.decision.candidate_node_id
        );
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
