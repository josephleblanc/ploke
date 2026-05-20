//! Passive successor-selection record DTOs.
//!
//! These records describe selector inputs, domain findings, and decisions. They
//! do not select a successor or authorize handoff.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::branch::Disposition;
use crate::evaluation::RunMetrics;
use crate::ids::{CandidateMembershipId, CandidateOccurrenceId, HistoryHash};
use crate::oracle;

/// Candidate coordinate considered by successor selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateRef {
    pub node_id: String,
    pub branch_id: String,
    pub generation: u32,
}

/// Generation-local evidence bundle available to the successor selector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Input {
    pub candidate: CandidateRef,
    pub branch_disposition: Disposition,
    pub evaluation_artifact_path: PathBuf,
    #[serde(default)]
    pub comparisons: Vec<RunComparison>,
}

/// Parent-vs-child metrics for one benchmark instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunComparison {
    pub instance_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_metrics: Option<RunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_metrics: Option<RunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_evaluation: Option<oracle::Evaluation>,
    pub status: String,
}

/// One field-level operational comparison.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricComparison {
    pub metric: String,
    pub parent: String,
    pub child: String,
    pub direction: MetricDirection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricDirection {
    Improved,
    Regressed,
    Unchanged,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DomainName {
    Operational,
    Protocol,
    Patch,
    Oracle,
    Adjudication,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Better,
    Worse,
    Mixed,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// Per-domain selector finding considered by a decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainFinding {
    pub domain: DomainName,
    pub verdict: Verdict,
    pub confidence: Confidence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<MetricComparison>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rationale: Vec<String>,
}

/// Selector outcome label.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Accepted,
    ExploreFrom,
    Stop,
}

/// Durable selector decision projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Decision {
    pub procedure_id: String,
    pub candidate_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_branch_id: Option<String>,
    pub branch_disposition: String,
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<DomainFinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rationale: Vec<String>,
}

/// Selection-time metric evidence sealed beside one decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricSet {
    pub schema_version: u32,
    pub id: HistoryHash,
    pub considered_order_hash: HistoryHash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_set_root: Option<HistoryHash>,
    pub policy: MetricPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<MetricCandidate>,
}

/// Selector formula sealed beside one selection decision.
///
/// This record is selection-entry scoped by containment in
/// `SelectionDecisionEntryRecord`; `metric_set_id` binds the formula to the
/// metric evidence used by that selection entry, but is not the formula
/// identity on its own.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectionFormulaRecord {
    pub metric_set_id: HistoryHash,
    pub formula: FormulaRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormulaRecord {
    ScoreChildProp(ScoreChildPropRecord),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreChildPropRecord {
    pub schema_version: u32,
    pub seed: u64,
    pub top_m: usize,
    pub lambda_millis: u32,
    pub lambda: f64,
    pub metric_inputs: String,
    pub oracle_mode: String,
    pub oracle_require_evidence: bool,
    pub total_weight: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uniform_fallback_slot: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_candidate: Option<String>,
    pub alpha_mid: f64,
    pub oracle_used_for_alpha: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<ScoreChildPropRowRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreChildPropRowRecord {
    pub payload_index: usize,
    pub candidate: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_disposition: Option<String>,
    pub base_outcome: Outcome,
    pub outcome_points: i64,
    pub operational_points: i64,
    pub protocol_points: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imp_at_k_delta: Option<i64>,
    pub imp_at_k_score_excluded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_resolved: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_configured: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_rate: Option<f64>,
    pub oracle_used_for_alpha: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_mid: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exploitation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exploration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cumulative_lower: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cumulative_upper: Option<f64>,
    pub sample_hit: bool,
    pub selectable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusion_reason: Option<String>,
    pub performance_present: bool,
    pub selection_input_present: bool,
    pub decision_present: bool,
    pub selected: bool,
}

impl MetricSet {
    pub fn absent_from_legacy_record() -> Self {
        Self {
            schema_version: 1,
            id: HistoryHash("0".repeat(64)),
            considered_order_hash: HistoryHash("0".repeat(64)),
            candidate_set_root: None,
            policy: MetricPolicy::default(),
            candidates: Vec::new(),
        }
    }
}

impl Default for MetricSet {
    fn default() -> Self {
        Self::absent_from_legacy_record()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricPolicy {
    pub persist: bool,
    pub score_profile: ScoreProfile,
    pub imp_at_k: ImpAtKPolicy,
}

impl Default for MetricPolicy {
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
pub enum ScoreProfile {
    OperationalQualityV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpAtKPolicy {
    pub enabled: bool,
    pub budget_k: usize,
    pub archive_scope: ArchiveScope,
    pub score_points_per_imp_point: i64,
    pub require_for_score: bool,
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
pub enum ArchiveScope {
    SelectionScope,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricCandidate {
    pub payload_index: usize,
    pub payload_hash: HistoryHash,
    pub candidate: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_id: Option<CandidateOccurrenceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub membership_id: Option<CandidateMembershipId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imp_at_k: Option<ImpAtK>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpAtK {
    pub start_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_generation: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_runtime_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluator: Option<Evaluator>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_set: Option<EvalSet>,
    pub budget_k: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_score: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_descendant_score: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub improvement: Option<i64>,
    pub descendant_count: usize,
    pub scored_descendant_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub incomplete_reasons: Vec<ImpAtKReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Evaluator {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalSet {
    pub id: String,
    pub kind: String,
    pub authority: String,
    pub explicit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_family: Option<String>,
    #[serde(default)]
    pub dataset_source_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instance_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_treatment_instance_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImpAtKReason {
    Disabled,
    MissingStartIdentity,
    MissingBaselineScore,
    NoDescendantsWithinBudget,
    NoScoredDescendants,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::HistoryHash;

    #[test]
    fn selection_metrics_roundtrip_passive_dto() {
        let metrics = MetricSet {
            schema_version: 1,
            id: HistoryHash("a".repeat(64)),
            considered_order_hash: HistoryHash("b".repeat(64)),
            candidate_set_root: Some(HistoryHash("c".repeat(64))),
            policy: MetricPolicy {
                persist: true,
                score_profile: ScoreProfile::OperationalQualityV1,
                imp_at_k: ImpAtKPolicy {
                    enabled: true,
                    budget_k: 50,
                    archive_scope: ArchiveScope::SelectionScope,
                    score_points_per_imp_point: 0,
                    require_for_score: false,
                },
            },
            candidates: vec![MetricCandidate {
                payload_index: 0,
                payload_hash: HistoryHash("d".repeat(64)),
                candidate: "candidate:node-a:plan_index=0".to_string(),
                occurrence_id: Some(CandidateOccurrenceId("e".repeat(64))),
                membership_id: Some(CandidateMembershipId("f".repeat(64))),
                imp_at_k: Some(ImpAtK {
                    start_node_id: "node-a".to_string(),
                    start_branch_id: Some("branch-a".to_string()),
                    start_generation: Some(1),
                    parent_node_id: None,
                    primary_runtime_id: Some("runtime:node-a".to_string()),
                    evaluator: Some(Evaluator {
                        id: "prototype1".to_string(),
                        version: "1".to_string(),
                    }),
                    eval_set: Some(EvalSet {
                        id: "eval-set".to_string(),
                        kind: "test".to_string(),
                        authority: "test-suite".to_string(),
                        explicit: true,
                        benchmark_family: Some("multi_swe_bench_rust".to_string()),
                        dataset_source_count: 1,
                        instance_ids: vec!["instance-a".to_string()],
                        missing_treatment_instance_ids: Vec::new(),
                        note: None,
                    }),
                    budget_k: 50,
                    baseline_score: Some(500),
                    best_descendant_score: Some(900),
                    improvement: Some(400),
                    descendant_count: 1,
                    scored_descendant_count: 1,
                    incomplete_reasons: Vec::new(),
                }),
            }],
        };

        let encoded = serde_json::to_string(&metrics).expect("serialize metrics");
        let decoded: MetricSet = serde_json::from_str(&encoded).expect("deserialize metrics");

        assert_eq!(decoded, metrics);
        assert_eq!(
            decoded.policy.score_profile,
            ScoreProfile::OperationalQualityV1
        );
        assert_eq!(
            decoded.candidates[0]
                .imp_at_k
                .as_ref()
                .and_then(|row| row.improvement),
            Some(400)
        );
    }

    #[test]
    fn selection_formula_roundtrip_passive_dto() {
        let formula = SelectionFormulaRecord {
            metric_set_id: HistoryHash("a".repeat(64)),
            formula: FormulaRecord::ScoreChildProp(ScoreChildPropRecord {
                schema_version: 1,
                seed: 7,
                top_m: 2,
                lambda_millis: 1500,
                lambda: 1.5,
                metric_inputs: "operational".to_string(),
                oracle_mode: "record_only".to_string(),
                oracle_require_evidence: true,
                total_weight: 0.75,
                sample: Some(0.25),
                sample_threshold: Some(0.1875),
                uniform_fallback_slot: None,
                selected_index: Some(0),
                selected_candidate: Some("candidate:node-a:plan_index=0".to_string()),
                alpha_mid: 0.5,
                oracle_used_for_alpha: false,
                rows: vec![ScoreChildPropRowRecord {
                    payload_index: 0,
                    candidate: "candidate:node-a:plan_index=0".to_string(),
                    node_id: Some("node-a".to_string()),
                    branch_id: Some("branch-a".to_string()),
                    branch_disposition: Some("keep".to_string()),
                    base_outcome: Outcome::Accepted,
                    outcome_points: 10_000,
                    operational_points: 900,
                    protocol_points: 0,
                    imp_at_k_delta: Some(0),
                    imp_at_k_score_excluded: false,
                    performance: Some(10_900),
                    oracle_resolved: None,
                    oracle_configured: None,
                    oracle_rate: None,
                    oracle_used_for_alpha: false,
                    child_count: Some(0),
                    alpha: Some(0.5),
                    alpha_mid: Some(0.5),
                    exploitation: Some(0.5),
                    exploration: Some(1.0),
                    weight: Some(0.5),
                    cumulative_lower: Some(0.0),
                    cumulative_upper: Some(0.5),
                    sample_hit: true,
                    selectable: true,
                    exclusion_reason: None,
                    performance_present: true,
                    selection_input_present: true,
                    decision_present: true,
                    selected: true,
                }],
            }),
        };

        let encoded = serde_json::to_string(&formula).expect("serialize formula");
        let decoded: SelectionFormulaRecord =
            serde_json::from_str(&encoded).expect("deserialize formula");

        assert_eq!(decoded, formula);
    }
}
