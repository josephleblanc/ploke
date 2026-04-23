use serde::{Deserialize, Serialize};

use crate::operational_metrics::OperationalRunMetrics;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchEvaluationInput {
    pub baseline_metrics: OperationalRunMetrics,
    pub treatment_metrics: OperationalRunMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BranchDisposition {
    Keep,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchEvaluationResult {
    pub disposition: BranchDisposition,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

pub fn evaluate_branch(input: &BranchEvaluationInput) -> BranchEvaluationResult {
    let baseline = &input.baseline_metrics;
    let treatment = &input.treatment_metrics;
    let mut regressions = Vec::new();
    let mut improvements = Vec::new();

    compare_lower_is_better(
        &mut regressions,
        &mut improvements,
        "partial_patch_failures",
        baseline.partial_patch_failures,
        treatment.partial_patch_failures,
    );
    compare_lower_is_better(
        &mut regressions,
        &mut improvements,
        "same_file_patch_retry_count",
        baseline.same_file_patch_retry_count,
        treatment.same_file_patch_retry_count,
    );
    compare_lower_is_better(
        &mut regressions,
        &mut improvements,
        "same_file_patch_max_streak",
        baseline.same_file_patch_max_streak,
        treatment.same_file_patch_max_streak,
    );
    compare_bool_prefer_false(
        &mut regressions,
        &mut improvements,
        "aborted_repair_loop",
        baseline.aborted_repair_loop,
        treatment.aborted_repair_loop,
    );
    compare_bool_prefer_true(
        &mut regressions,
        &mut improvements,
        "convergence",
        baseline.convergence,
        treatment.convergence,
    );
    compare_bool_prefer_true(
        &mut regressions,
        &mut improvements,
        "oracle_eligible",
        baseline.oracle_eligible,
        treatment.oracle_eligible,
    );

    if regressions.is_empty() {
        BranchEvaluationResult {
            disposition: BranchDisposition::Keep,
            reasons: improvements,
        }
    } else {
        BranchEvaluationResult {
            disposition: BranchDisposition::Reject,
            reasons: regressions,
        }
    }
}

fn compare_lower_is_better(
    regressions: &mut Vec<String>,
    improvements: &mut Vec<String>,
    field: &str,
    baseline: usize,
    treatment: usize,
) {
    if treatment > baseline {
        regressions.push(format!("{field} regressed: {baseline} -> {treatment}"));
    } else if treatment < baseline {
        improvements.push(format!("{field} improved: {baseline} -> {treatment}"));
    }
}

fn compare_bool_prefer_false(
    regressions: &mut Vec<String>,
    improvements: &mut Vec<String>,
    field: &str,
    baseline: bool,
    treatment: bool,
) {
    match (baseline, treatment) {
        (false, true) => regressions.push(format!("{field} regressed: false -> true")),
        (true, false) => improvements.push(format!("{field} improved: true -> false")),
        _ => {}
    }
}

fn compare_bool_prefer_true(
    regressions: &mut Vec<String>,
    improvements: &mut Vec<String>,
    field: &str,
    baseline: bool,
    treatment: bool,
) {
    match (baseline, treatment) {
        (true, false) => regressions.push(format!("{field} regressed: true -> false")),
        (false, true) => improvements.push(format!("{field} improved: false -> true")),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{BranchDisposition, BranchEvaluationInput, evaluate_branch};
    use crate::operational_metrics::{OperationalRunMetrics, PatchApplyState};
    use crate::record::SubmissionArtifactState;

    fn metrics() -> OperationalRunMetrics {
        OperationalRunMetrics {
            tool_calls_total: 10,
            tool_calls_failed: 2,
            patch_attempted: true,
            patch_apply_state: PatchApplyState::Applied,
            submission_artifact_state: SubmissionArtifactState::Nonempty,
            partial_patch_failures: 2,
            same_file_patch_retry_count: 3,
            same_file_patch_max_streak: 3,
            aborted: false,
            aborted_repair_loop: false,
            nonempty_valid_patch: true,
            convergence: true,
            oracle_eligible: true,
        }
    }

    #[test]
    fn evaluate_branch_rejects_regressions() {
        let baseline = metrics();
        let mut treatment = metrics();
        treatment.same_file_patch_max_streak = 5;

        let result = evaluate_branch(&BranchEvaluationInput {
            baseline_metrics: baseline,
            treatment_metrics: treatment,
        });

        assert_eq!(result.disposition, BranchDisposition::Reject);
        assert!(
            result
                .reasons
                .iter()
                .any(|reason| reason.contains("same_file_patch_max_streak"))
        );
    }

    #[test]
    fn evaluate_branch_keeps_non_regressing_improvements() {
        let baseline = metrics();
        let mut treatment = metrics();
        treatment.partial_patch_failures = 0;
        treatment.same_file_patch_retry_count = 1;

        let result = evaluate_branch(&BranchEvaluationInput {
            baseline_metrics: baseline,
            treatment_metrics: treatment,
        });

        assert_eq!(result.disposition, BranchDisposition::Keep);
        assert!(
            result
                .reasons
                .iter()
                .any(|reason| reason.contains("partial_patch_failures improved"))
        );
    }
}
