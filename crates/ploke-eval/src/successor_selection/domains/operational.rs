use crate::{BranchDisposition, OperationalRunMetrics};

use super::{Confidence, Domain, DomainFinding, DomainName, Verdict};
use crate::successor_selection::evidence::{MetricComparison, MetricDirection, SelectionInput};
use crate::successor_selection::{disposition_as_str, evidence_ref};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OperationalDomain;

impl Domain for OperationalDomain {
    fn name(&self) -> DomainName {
        DomainName::Operational
    }

    fn evaluate(&self, input: &SelectionInput) -> DomainFinding {
        let mut metrics = Vec::new();
        let mut comparable_instances = 0_usize;

        for comparison in &input.comparisons {
            let Some(parent) = comparison.parent_metrics.as_ref() else {
                continue;
            };
            let Some(child) = comparison.child_metrics.as_ref() else {
                continue;
            };
            comparable_instances += 1;
            compare_metrics(parent, child, &mut metrics);
        }

        let improvements = metrics
            .iter()
            .filter(|metric| metric.direction == MetricDirection::Improved)
            .count();
        let regressions = metrics
            .iter()
            .filter(|metric| metric.direction == MetricDirection::Regressed)
            .count();

        let verdict = match (
            input.branch_disposition.clone(),
            comparable_instances,
            improvements,
            regressions,
        ) {
            (_, 0, _, _) => Verdict::Inconclusive,
            (BranchDisposition::Keep, _, _, 0) => Verdict::Better,
            (BranchDisposition::Keep, _, _, _) => Verdict::Mixed,
            (BranchDisposition::Reject, _, 0, r) if r > 0 => Verdict::Worse,
            (BranchDisposition::Reject, _, i, r) if i > 0 && r > 0 => Verdict::Mixed,
            _ => Verdict::Worse,
        };

        let confidence = if comparable_instances == 0 {
            Confidence::Low
        } else if regressions == 0 || improvements == 0 {
            Confidence::High
        } else {
            Confidence::Medium
        };

        DomainFinding {
            domain: self.name(),
            verdict,
            confidence,
            metrics,
            evidence_refs: vec![evidence_ref(&input.evaluation_artifact_path)],
            rationale: vec![format!(
                "branch_disposition={} comparable_instances={comparable_instances} improvements={improvements} regressions={regressions}",
                disposition_as_str(input.branch_disposition.clone()),
            )],
        }
    }
}

fn compare_metrics(
    parent: &OperationalRunMetrics,
    child: &OperationalRunMetrics,
    metrics: &mut Vec<MetricComparison>,
) {
    lower_is_better(
        "tool_calls_failed",
        parent.tool_calls_failed,
        child.tool_calls_failed,
        metrics,
    );
    lower_is_better(
        "partial_patch_failures",
        parent.partial_patch_failures,
        child.partial_patch_failures,
        metrics,
    );
    lower_is_better(
        "same_file_patch_retry_count",
        parent.same_file_patch_retry_count,
        child.same_file_patch_retry_count,
        metrics,
    );
    lower_is_better(
        "same_file_patch_max_streak",
        parent.same_file_patch_max_streak,
        child.same_file_patch_max_streak,
        metrics,
    );
    prefer_false("aborted", parent.aborted, child.aborted, metrics);
    prefer_false(
        "aborted_repair_loop",
        parent.aborted_repair_loop,
        child.aborted_repair_loop,
        metrics,
    );
    prefer_true(
        "nonempty_valid_patch",
        parent.nonempty_valid_patch,
        child.nonempty_valid_patch,
        metrics,
    );
    prefer_true(
        "convergence",
        parent.convergence,
        child.convergence,
        metrics,
    );
    prefer_true(
        "oracle_eligible",
        parent.oracle_eligible,
        child.oracle_eligible,
        metrics,
    );
}

fn lower_is_better(metric: &str, parent: usize, child: usize, metrics: &mut Vec<MetricComparison>) {
    if child < parent {
        metrics.push(MetricComparison::improved(metric, parent, child));
    } else if child > parent {
        metrics.push(MetricComparison::regressed(metric, parent, child));
    }
}

fn prefer_false(metric: &str, parent: bool, child: bool, metrics: &mut Vec<MetricComparison>) {
    match (parent, child) {
        (true, false) => metrics.push(MetricComparison::improved(metric, parent, child)),
        (false, true) => metrics.push(MetricComparison::regressed(metric, parent, child)),
        _ => {}
    }
}

fn prefer_true(metric: &str, parent: bool, child: bool, metrics: &mut Vec<MetricComparison>) {
    match (parent, child) {
        (false, true) => metrics.push(MetricComparison::improved(metric, parent, child)),
        (true, false) => metrics.push(MetricComparison::regressed(metric, parent, child)),
        _ => {}
    }
}
