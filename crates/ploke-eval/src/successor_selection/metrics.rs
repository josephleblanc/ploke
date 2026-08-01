use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::cli::prototype1_state::history::{
    CandidateMembershipId, CandidateOccurrenceId, CandidateSetCommitment, CandidateSetRoot,
    EvaluationPayload, HistoryError, HistoryHash, SealedEvalSetIdentity, SealedEvaluatorIdentity,
    TraversalCandidateSource,
};
use crate::metric::{self, Summary};

const SET_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Set {
    pub(crate) schema_version: u32,
    pub(crate) id: HistoryHash,
    pub(crate) considered_order_hash: HistoryHash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) candidate_set_root: Option<CandidateSetRoot>,
    pub(crate) policy: Policy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) candidates: Vec<Candidate>,
}

impl Set {
    pub(crate) fn from_considered(
        policy: Policy,
        considered: &[EvaluationPayload],
        considered_sources: &[TraversalCandidateSource],
    ) -> Result<Self, HistoryError> {
        let considered_order_hash = considered_order_hash(considered)?;
        let candidate_set = if considered.is_empty() {
            None
        } else if considered_sources.is_empty() {
            Some(CandidateSetCommitment::from_payloads(considered)?)
        } else {
            Some(CandidateSetCommitment::from_payloads_with_sources(
                considered,
                considered_sources,
            )?)
        };
        let candidate_set_root = candidate_set.as_ref().map(|set| set.root.clone());
        let candidates = if policy.persist {
            candidates(
                policy,
                considered,
                considered_sources,
                candidate_set.as_ref(),
            )?
        } else {
            Vec::new()
        };
        let preimage = SetPreimage {
            schema_version: SET_SCHEMA_VERSION,
            considered_order_hash: &considered_order_hash,
            candidate_set_root: candidate_set_root.as_ref(),
            policy: &policy,
            candidates: &candidates,
        };
        let id = HistoryHash::of_domain_json("prototype1.selection.metrics.set.v1", &preimage)?;
        Ok(Self {
            schema_version: SET_SCHEMA_VERSION,
            id,
            considered_order_hash,
            candidate_set_root,
            policy,
            candidates,
        })
    }

    pub(crate) fn score_delta(&self, payload_index: usize) -> Result<i64, ScoreExclusion> {
        let imp = &self.policy.imp_at_k;
        if !imp.enabled || imp.score_points_per_imp_point == 0 {
            return Ok(0);
        }
        let Some(candidate) = self
            .candidates
            .iter()
            .find(|candidate| candidate.payload_index == payload_index)
        else {
            return if imp.require_for_score {
                Err(ScoreExclusion)
            } else {
                Ok(0)
            };
        };
        let Some(row) = candidate.imp_at_k.as_ref() else {
            return if imp.require_for_score {
                Err(ScoreExclusion)
            } else {
                Ok(0)
            };
        };
        let Some(improvement) = row
            .improvement
            .filter(|_| row.incomplete_reasons.is_empty())
        else {
            return if imp.require_for_score {
                Err(ScoreExclusion)
            } else {
                Ok(0)
            };
        };
        Ok(improvement.saturating_mul(imp.score_points_per_imp_point))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScoreExclusion;

#[derive(Serialize)]
struct SetPreimage<'a> {
    schema_version: u32,
    considered_order_hash: &'a HistoryHash,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate_set_root: Option<&'a CandidateSetRoot>,
    policy: &'a Policy,
    candidates: &'a [Candidate],
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Policy {
    pub(crate) persist: bool,
    pub(crate) score_profile: ScoreProfile,
    pub(crate) imp_at_k: ImpAtKPolicy,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            persist: true,
            score_profile: ScoreProfile::OperationalQualityV1,
            imp_at_k: ImpAtKPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ScoreProfile {
    OperationalQualityV1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ImpAtKPolicy {
    pub(crate) enabled: bool,
    pub(crate) budget_k: usize,
    pub(crate) archive_scope: ArchiveScope,
    pub(crate) score_points_per_imp_point: i64,
    pub(crate) require_for_score: bool,
}

impl Default for ImpAtKPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            budget_k: 50,
            archive_scope: ArchiveScope::SelectionScope,
            score_points_per_imp_point: 0,
            require_for_score: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ArchiveScope {
    SelectionScope,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) payload_index: usize,
    pub(crate) payload_hash: HistoryHash,
    pub(crate) candidate: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) occurrence_id: Option<CandidateOccurrenceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) membership_id: Option<CandidateMembershipId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) imp_at_k: Option<ImpAtK>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ImpAtK {
    pub(crate) start_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) start_branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) start_generation: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parent_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) primary_runtime_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) evaluator: Option<Evaluator>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) eval_set: Option<EvalSet>,
    pub(crate) budget_k: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) baseline_score: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) best_descendant_score: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) improvement: Option<i64>,
    pub(crate) descendant_count: usize,
    pub(crate) scored_descendant_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) incomplete_reasons: Vec<ImpAtKReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Evaluator {
    pub(crate) id: String,
    pub(crate) version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EvalSet {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) authority: String,
    pub(crate) explicit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) benchmark_family: Option<String>,
    #[serde(default)]
    pub(crate) dataset_source_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) instance_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) missing_treatment_instance_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) note: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ImpAtKReason {
    Disabled,
    MissingStartIdentity,
    MissingBaselineScore,
    NoDescendantsWithinBudget,
    NoScoredDescendants,
}

fn candidates(
    policy: Policy,
    considered: &[EvaluationPayload],
    considered_sources: &[TraversalCandidateSource],
    candidate_set: Option<&CandidateSetCommitment>,
) -> Result<Vec<Candidate>, HistoryError> {
    let rows = considered
        .iter()
        .enumerate()
        .map(|(payload_index, payload)| {
            let membership = match (candidate_set, considered_sources.get(payload_index)) {
                (Some(set), Some(source)) => {
                    set.membership_for_payload(payload, source.candidate_source_class())?
                }
                _ => None,
            };
            Ok(Candidate {
                payload_index,
                payload_hash: payload.payload_hash()?,
                candidate: payload.candidate.as_str().to_string(),
                occurrence_id: membership.and_then(|member| member.occurrence_id.clone()),
                membership_id: membership.and_then(|member| member.membership_id.clone()),
                imp_at_k: imp_at_k(policy, payload_index, considered),
            })
        })
        .collect::<Result<Vec<_>, HistoryError>>()?;
    Ok(rows)
}

fn imp_at_k(
    policy: Policy,
    payload_index: usize,
    considered: &[EvaluationPayload],
) -> Option<ImpAtK> {
    let imp = policy.imp_at_k;
    if !imp.enabled {
        return Some(disabled_row(considered.get(payload_index)?, imp.budget_k));
    }
    let payload = considered.get(payload_index)?;
    let mut reasons = Vec::new();
    let Some(start_node_id) = node_id(payload) else {
        reasons.push(ImpAtKReason::MissingStartIdentity);
        return Some(ImpAtK {
            start_node_id: payload.candidate.as_str().to_string(),
            start_branch_id: branch_id(payload),
            start_generation: generation(payload),
            parent_node_id: parent_node_id(payload),
            primary_runtime_id: primary_runtime_id(payload),
            evaluator: evaluator(payload),
            eval_set: eval_set(payload),
            budget_k: imp.budget_k,
            baseline_score: None,
            best_descendant_score: None,
            improvement: None,
            descendant_count: 0,
            scored_descendant_count: 0,
            incomplete_reasons: reasons,
        });
    };

    let baseline_score = score(payload, policy.score_profile);
    if baseline_score.is_none() {
        reasons.push(ImpAtKReason::MissingBaselineScore);
    }

    let descendants = descendants_within(start_node_id.as_str(), considered, imp.budget_k);
    if descendants.is_empty() {
        reasons.push(ImpAtKReason::NoDescendantsWithinBudget);
    }
    let descendant_scores = descendants
        .iter()
        .filter_map(|index| considered.get(*index))
        .filter_map(|payload| score(payload, policy.score_profile))
        .collect::<Vec<_>>();
    if !descendants.is_empty() && descendant_scores.is_empty() {
        reasons.push(ImpAtKReason::NoScoredDescendants);
    }
    let best_descendant_score = descendant_scores.iter().copied().max();
    let improvement = baseline_score
        .zip(best_descendant_score)
        .map(|(baseline, best)| best - baseline);

    Some(ImpAtK {
        start_node_id,
        start_branch_id: branch_id(payload),
        start_generation: generation(payload),
        parent_node_id: parent_node_id(payload),
        primary_runtime_id: primary_runtime_id(payload),
        evaluator: evaluator(payload),
        eval_set: eval_set(payload),
        budget_k: imp.budget_k,
        baseline_score,
        best_descendant_score,
        improvement,
        descendant_count: descendants.len(),
        scored_descendant_count: descendant_scores.len(),
        incomplete_reasons: reasons,
    })
}

fn disabled_row(payload: &EvaluationPayload, budget_k: usize) -> ImpAtK {
    ImpAtK {
        start_node_id: node_id(payload).unwrap_or_else(|| payload.candidate.as_str().to_string()),
        start_branch_id: branch_id(payload),
        start_generation: generation(payload),
        parent_node_id: parent_node_id(payload),
        primary_runtime_id: primary_runtime_id(payload),
        evaluator: evaluator(payload),
        eval_set: eval_set(payload),
        budget_k,
        baseline_score: None,
        best_descendant_score: None,
        improvement: None,
        descendant_count: 0,
        scored_descendant_count: 0,
        incomplete_reasons: vec![ImpAtKReason::Disabled],
    }
}

fn descendants_within(
    start_node_id: &str,
    considered: &[EvaluationPayload],
    budget_k: usize,
) -> Vec<usize> {
    let mut children_by_parent = BTreeMap::<String, Vec<usize>>::new();
    for (index, payload) in considered.iter().enumerate() {
        if let Some(parent) = parent_node_id(payload) {
            children_by_parent.entry(parent).or_default().push(index);
        }
    }

    let mut out = Vec::new();
    let mut queue = VecDeque::from([start_node_id.to_string()]);
    let mut seen_nodes = BTreeSet::from([start_node_id.to_string()]);
    let mut seen_indices = BTreeSet::new();
    while let Some(parent) = queue.pop_front() {
        let Some(children) = children_by_parent.get(&parent) else {
            continue;
        };
        for index in children {
            if !seen_indices.insert(*index) {
                continue;
            }
            out.push(*index);
            if out.len() >= budget_k {
                return out;
            }
            if let Some(child_node) = considered.get(*index).and_then(node_id)
                && seen_nodes.insert(child_node.clone())
            {
                queue.push_back(child_node);
            }
        }
    }
    out
}

fn score(payload: &EvaluationPayload, profile: ScoreProfile) -> Option<i64> {
    match profile {
        ScoreProfile::OperationalQualityV1 => operational_score(payload),
    }
}

fn operational_score(payload: &EvaluationPayload) -> Option<i64> {
    let from_selection_input = payload.selection_input.as_ref().and_then(|input| {
        let scores = input
            .comparisons
            .iter()
            .filter_map(|comparison| comparison.child_metrics.as_ref())
            .map(metric::Operational::selection_points)
            .collect::<Vec<_>>();
        (!scores.is_empty()).then(|| scores.into_iter().sum())
    });
    from_selection_input.or_else(|| {
        payload.sealed_evidence.as_ref().and_then(|sealed| {
            let scores = sealed
                .evaluations
                .iter()
                .flat_map(|evaluation| evaluation.compared_runs.iter())
                .filter_map(|run| run.treatment_metrics.as_ref())
                .map(metric::Operational::selection_points)
                .collect::<Vec<_>>();
            (!scores.is_empty()).then(|| scores.into_iter().sum())
        })
    })
}

pub(crate) fn considered_order_hash(
    considered: &[EvaluationPayload],
) -> Result<HistoryHash, HistoryError> {
    let preimage = considered
        .iter()
        .map(|payload| payload.payload_hash())
        .collect::<Result<Vec<_>, _>>()?;
    HistoryHash::of_domain_json(
        "prototype1.history.selection_considered_order.v1",
        &preimage,
    )
}

fn node_id(payload: &EvaluationPayload) -> Option<String> {
    payload
        .selection_input
        .as_ref()
        .map(|input| input.candidate.node_id.clone())
        .or_else(|| {
            payload
                .sealed_evidence
                .as_ref()
                .map(|sealed| sealed.coordinate.node_id.clone())
        })
}

fn branch_id(payload: &EvaluationPayload) -> Option<String> {
    payload
        .selection_input
        .as_ref()
        .map(|input| input.candidate.branch_id.clone())
        .or_else(|| {
            payload
                .sealed_evidence
                .as_ref()
                .and_then(|sealed| sealed.coordinate.branch_id.clone())
        })
}

fn generation(payload: &EvaluationPayload) -> Option<u32> {
    payload
        .selection_input
        .as_ref()
        .map(|input| input.candidate.generation)
        .or_else(|| {
            payload
                .sealed_evidence
                .as_ref()
                .and_then(|sealed| sealed.coordinate.generation)
        })
}

fn parent_node_id(payload: &EvaluationPayload) -> Option<String> {
    payload
        .sealed_evidence
        .as_ref()
        .and_then(|sealed| sealed.coordinate.parent_node_id.clone())
}

fn primary_runtime_id(payload: &EvaluationPayload) -> Option<String> {
    payload
        .sealed_evidence
        .as_ref()
        .and_then(|sealed| sealed.coordinate.primary_runtime_id.clone())
}

fn evaluator(payload: &EvaluationPayload) -> Option<Evaluator> {
    payload
        .sealed_evidence
        .as_ref()?
        .evaluations
        .iter()
        .find_map(|evaluation| evaluation.evaluator_identity.as_ref())
        .map(Evaluator::from)
}

fn eval_set(payload: &EvaluationPayload) -> Option<EvalSet> {
    payload
        .sealed_evidence
        .as_ref()?
        .evaluations
        .iter()
        .find_map(|evaluation| evaluation.eval_set_identity.as_ref())
        .map(EvalSet::from)
}

impl From<&SealedEvaluatorIdentity> for Evaluator {
    fn from(value: &SealedEvaluatorIdentity) -> Self {
        Self {
            id: value.id.clone(),
            version: value.version.clone(),
        }
    }
}

impl From<&SealedEvalSetIdentity> for EvalSet {
    fn from(value: &SealedEvalSetIdentity) -> Self {
        Self {
            id: value.id.clone(),
            kind: value.kind.clone(),
            authority: value.authority.clone(),
            explicit: value.explicit,
            benchmark_family: value.benchmark_family.clone(),
            dataset_source_count: value.dataset_source_count,
            instance_ids: value.instance_ids.clone(),
            missing_treatment_instance_ids: value.missing_treatment_instance_ids.clone(),
            note: value.note.clone(),
        }
    }
}
