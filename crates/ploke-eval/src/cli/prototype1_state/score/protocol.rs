#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{
    ComparedRunEvidence, MetricScore, PROTOCOL_COMPONENT_ID, PROTOCOL_COMPONENT_PROCEDURE_ID,
    ScoreComponent, ScoreComponentProvenance, ScoreDiagnostic, ScoreDirection, aggregate_state,
    sum_scores,
};
use crate::cli::prototype1_state::evidence::RunEvidence;
use crate::protocol::protocol_aggregate::{
    ProtocolAggregate, ProtocolAggregateError, ProtocolArtifactRef, load_protocol_aggregate,
};

pub(super) fn protocol_component(compared: &ComparedRunEvidence) -> ScoreComponent {
    let mut diagnostics = Vec::new();
    push_protocol_evidence_diagnostics(
        "baseline",
        compared.baseline_run.as_ref(),
        &mut diagnostics,
    );
    push_protocol_evidence_diagnostics(
        "treatment",
        compared.treatment_run.as_ref(),
        &mut diagnostics,
    );

    let baseline = load_protocol_component_aggregate("baseline", compared.baseline_run.as_ref());
    let treatment = load_protocol_component_aggregate("treatment", compared.treatment_run.as_ref());
    let mut provenance = ScoreComponentProvenance::from_compared(compared);

    let metrics = match (baseline, treatment) {
        (Ok(baseline), Ok(treatment)) => {
            provenance.baseline_protocol_artifacts = protocol_artifacts(&baseline);
            provenance.treatment_protocol_artifacts = protocol_artifacts(&treatment);
            compare_protocol_aggregates(&baseline, &treatment)
        }
        (Err(diagnostic), Ok(treatment)) => {
            provenance.treatment_protocol_artifacts = protocol_artifacts(&treatment);
            diagnostics.push(diagnostic);
            Vec::new()
        }
        (Ok(baseline), Err(diagnostic)) => {
            provenance.baseline_protocol_artifacts = protocol_artifacts(&baseline);
            diagnostics.push(diagnostic);
            Vec::new()
        }
        (Err(left), Err(right)) => {
            diagnostics.push(left);
            diagnostics.push(right);
            Vec::new()
        }
    };
    let state = aggregate_state(
        diagnostics.iter().map(|diagnostic| diagnostic.state()),
        std::iter::empty(),
    );

    ScoreComponent {
        component_id: PROTOCOL_COMPONENT_ID.to_string(),
        procedure_id: PROTOCOL_COMPONENT_PROCEDURE_ID.to_string(),
        state,
        score: sum_scores(metrics.iter().map(|metric| metric.points)),
        metrics,
        provenance,
        diagnostics,
    }
}

pub(super) fn load_protocol_component_aggregate(
    arm: &'static str,
    run: Option<&RunEvidence>,
) -> Result<ProtocolAggregate, ScoreDiagnostic> {
    let run = run.ok_or_else(|| {
        ScoreDiagnostic::missing(
            format!("{arm}_run_registration"),
            None,
            format!("compared {arm} arm does not carry typed RunRegistration evidence"),
        )
    })?;
    load_protocol_aggregate(&run.artifacts.record_path)
        .map_err(|error| protocol_aggregate_diagnostic(arm, error))
}

fn push_protocol_evidence_diagnostics(
    arm: &'static str,
    run: Option<&RunEvidence>,
    diagnostics: &mut Vec<ScoreDiagnostic>,
) {
    let Some(run) = run else {
        return;
    };
    diagnostics.extend(
        run.protocol
            .diagnostics
            .iter()
            .map(|diagnostic| ScoreDiagnostic {
                severity: diagnostic.severity.clone(),
                field: format!("{arm}.protocol.{}", diagnostic.field),
                source_ref: None,
                message: diagnostic.message.clone(),
            }),
    );
}

fn protocol_aggregate_diagnostic(
    arm: &'static str,
    error: ProtocolAggregateError,
) -> ScoreDiagnostic {
    let message = error.to_string();
    match error {
        ProtocolAggregateError::MissingAnchor { record_path } => ScoreDiagnostic::missing(
            format!("{arm}_protocol_anchor"),
            Some(record_path.display().to_string()),
            message,
        ),
        ProtocolAggregateError::Source(_)
        | ProtocolAggregateError::InvalidArtifactField { .. }
        | ProtocolAggregateError::SegmentBasisMismatch { .. } => {
            ScoreDiagnostic::invalid(format!("{arm}_protocol_aggregate"), None, message)
        }
    }
}

fn compare_protocol_aggregates(
    baseline: &ProtocolAggregate,
    treatment: &ProtocolAggregate,
) -> Vec<MetricScore> {
    let mut scores = Vec::new();
    higher_is_better(
        &mut scores,
        "protocol.reviewed_call_count",
        baseline.coverage.reviewed_call_count,
        treatment.coverage.reviewed_call_count,
    );
    higher_is_better(
        &mut scores,
        "protocol.reviewed_segment_count",
        baseline.coverage.reviewed_segment_count,
        treatment.coverage.reviewed_segment_count,
    );
    lower_is_better(
        &mut scores,
        "protocol.missing_call_count",
        baseline.coverage.missing_call_indices.len(),
        treatment.coverage.missing_call_indices.len(),
    );
    lower_is_better(
        &mut scores,
        "protocol.missing_segment_count",
        baseline.coverage.missing_segment_indices.len(),
        treatment.coverage.missing_segment_indices.len(),
    );
    lower_is_better(
        &mut scores,
        "protocol.skipped_segment_review_count",
        baseline.coverage.skipped_segment_review_count,
        treatment.coverage.skipped_segment_review_count,
    );
    lower_is_better(
        &mut scores,
        "protocol.segment_anchor_mismatch_count",
        baseline.coverage.segment_anchor_mismatch_count,
        treatment.coverage.segment_anchor_mismatch_count,
    );
    higher_is_better(
        &mut scores,
        "protocol.calls_with_segment_crosswalk",
        baseline.derived_metrics.calls_with_segment_crosswalk,
        treatment.derived_metrics.calls_with_segment_crosswalk,
    );
    higher_is_better(
        &mut scores,
        "protocol.call_review_overall.focused_progress",
        protocol_count(
            &baseline.derived_metrics.call_review_overall_counts,
            "focused_progress",
        ),
        protocol_count(
            &treatment.derived_metrics.call_review_overall_counts,
            "focused_progress",
        ),
    );
    higher_is_better(
        &mut scores,
        "protocol.segment_review_overall.focused_progress",
        protocol_count(
            &baseline.derived_metrics.segment_review_overall_counts,
            "focused_progress",
        ),
        protocol_count(
            &treatment.derived_metrics.segment_review_overall_counts,
            "focused_progress",
        ),
    );
    scores
}

fn protocol_count(counts: &BTreeMap<String, usize>, key: &str) -> usize {
    counts.get(key).copied().unwrap_or(0)
}

fn protocol_artifacts(aggregate: &ProtocolAggregate) -> Vec<ProtocolArtifactRef> {
    let mut by_path = BTreeMap::<PathBuf, ProtocolArtifactRef>::new();
    insert_protocol_artifact(&mut by_path, aggregate.segmentation.artifact.clone());
    for row in &aggregate.call_reviews {
        insert_protocol_artifact(&mut by_path, row.artifact.clone());
    }
    for row in &aggregate.segment_reviews {
        insert_protocol_artifact(&mut by_path, row.artifact.clone());
    }
    for row in &aggregate.skipped_segment_reviews {
        insert_protocol_artifact(&mut by_path, row.artifact.clone());
    }
    by_path.into_values().collect()
}

fn insert_protocol_artifact(
    artifacts: &mut BTreeMap<PathBuf, ProtocolArtifactRef>,
    artifact: ProtocolArtifactRef,
) {
    artifacts.insert(artifact.path.clone(), artifact);
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
