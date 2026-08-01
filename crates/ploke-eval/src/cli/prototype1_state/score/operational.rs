#![allow(dead_code)]

use super::{
    ComparedRunEvidence, MetricScore, OPERATIONAL_COMPONENT_ID, PROCEDURE_ID, ScoreComponent,
    ScoreComponentProvenance, ScoreDiagnostic, ScoreDirection, aggregate_state, sum_scores,
};
use crate::OperationalRunMetrics;

pub(super) fn operational_component(compared: &ComparedRunEvidence) -> ScoreComponent {
    let mut diagnostics = Vec::new();
    let metrics = match (
        compared.baseline_metrics.as_ref(),
        compared.treatment_metrics.as_ref(),
    ) {
        (Some(baseline), Some(treatment)) => compare_operational_metrics(baseline, treatment),
        (None, Some(_)) => {
            diagnostics.push(ScoreDiagnostic::missing(
                "baseline_metrics",
                None,
                "compared run has treatment metrics but no baseline metrics",
            ));
            Vec::new()
        }
        (Some(_), None) => {
            diagnostics.push(ScoreDiagnostic::missing(
                "treatment_metrics",
                None,
                "compared run has baseline metrics but no treatment metrics",
            ));
            Vec::new()
        }
        (None, None) => {
            diagnostics.push(ScoreDiagnostic::missing(
                "operational_metrics",
                None,
                "compared run has no baseline or treatment metrics",
            ));
            Vec::new()
        }
    };
    let state = aggregate_state(
        diagnostics.iter().map(|diagnostic| diagnostic.state()),
        std::iter::empty(),
    );

    ScoreComponent {
        component_id: OPERATIONAL_COMPONENT_ID.to_string(),
        procedure_id: PROCEDURE_ID.to_string(),
        state,
        score: sum_scores(metrics.iter().map(|metric| metric.points)),
        metrics,
        provenance: ScoreComponentProvenance::from_compared(compared),
        diagnostics,
    }
}

fn compare_operational_metrics(
    baseline: &OperationalRunMetrics,
    treatment: &OperationalRunMetrics,
) -> Vec<MetricScore> {
    let mut scores = Vec::new();
    lower_is_better(
        &mut scores,
        "tool_calls_failed",
        baseline.tool_calls_failed,
        treatment.tool_calls_failed,
    );
    lower_is_better(
        &mut scores,
        "partial_patch_failures",
        baseline.partial_patch_failures,
        treatment.partial_patch_failures,
    );
    lower_is_better(
        &mut scores,
        "same_file_patch_retry_count",
        baseline.same_file_patch_retry_count,
        treatment.same_file_patch_retry_count,
    );
    lower_is_better(
        &mut scores,
        "same_file_patch_max_streak",
        baseline.same_file_patch_max_streak,
        treatment.same_file_patch_max_streak,
    );
    prefer_false(&mut scores, "aborted", baseline.aborted, treatment.aborted);
    prefer_false(
        &mut scores,
        "aborted_repair_loop",
        baseline.aborted_repair_loop,
        treatment.aborted_repair_loop,
    );
    prefer_true(
        &mut scores,
        "nonempty_valid_patch",
        baseline.nonempty_valid_patch,
        treatment.nonempty_valid_patch,
    );
    prefer_true(
        &mut scores,
        "convergence",
        baseline.convergence,
        treatment.convergence,
    );
    prefer_true(
        &mut scores,
        "oracle_eligible",
        baseline.oracle_eligible,
        treatment.oracle_eligible,
    );
    scores
}

fn higher_is_better(scores: &mut Vec<MetricScore>, field: &str, baseline: usize, treatment: usize) {
    let direction = if treatment > baseline {
        ScoreDirection::Improved
    } else if treatment < baseline {
        ScoreDirection::Regressed
    } else {
        ScoreDirection::Unchanged
    };
    scores.push(metric_score(field, baseline, treatment, direction));
}

fn lower_is_better(scores: &mut Vec<MetricScore>, field: &str, baseline: usize, treatment: usize) {
    let direction = if treatment < baseline {
        ScoreDirection::Improved
    } else if treatment > baseline {
        ScoreDirection::Regressed
    } else {
        ScoreDirection::Unchanged
    };
    scores.push(metric_score(field, baseline, treatment, direction));
}

fn prefer_false(scores: &mut Vec<MetricScore>, field: &str, baseline: bool, treatment: bool) {
    let direction = match (baseline, treatment) {
        (true, false) => ScoreDirection::Improved,
        (false, true) => ScoreDirection::Regressed,
        _ => ScoreDirection::Unchanged,
    };
    scores.push(metric_score(field, baseline, treatment, direction));
}

fn prefer_true(scores: &mut Vec<MetricScore>, field: &str, baseline: bool, treatment: bool) {
    let direction = match (baseline, treatment) {
        (false, true) => ScoreDirection::Improved,
        (true, false) => ScoreDirection::Regressed,
        _ => ScoreDirection::Unchanged,
    };
    scores.push(metric_score(field, baseline, treatment, direction));
}

fn metric_score(
    field: &str,
    baseline: impl ToString,
    treatment: impl ToString,
    direction: ScoreDirection,
) -> MetricScore {
    MetricScore {
        field: field.to_string(),
        baseline: baseline.to_string(),
        treatment: treatment.to_string(),
        direction,
        points: match direction {
            ScoreDirection::Improved => 1,
            ScoreDirection::Regressed => -1,
            ScoreDirection::Unchanged => 0,
        },
    }
}
