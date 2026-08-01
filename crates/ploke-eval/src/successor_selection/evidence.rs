use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    BranchDisposition, OperationalRunMetrics,
    cli::prototype1_state::history::{HistoryHash, SealedEvidenceCitation},
    loop_graph::ArtifactId,
};

use super::{CandidateRef, domains::Confidence};

/// Generation-local evidence bundle available to the current successor selector.
///
/// This is a narrow projection from persisted child/evaluation records. It is
/// not the final authority object. Future selectors should receive a bounded
/// `History::candidates(...)` projection that records scope, sampling policy,
/// validator/admission evidence, and evidence refs before a `SelectionDecision`
/// is admitted through the Crown/History path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SelectionInput {
    pub(crate) candidate: CandidateRef,
    pub(crate) branch_disposition: BranchDisposition,
    pub(crate) evaluation_artifact_path: PathBuf,
    pub(crate) comparisons: Vec<RunComparison>,
}

impl SelectionInput {
    pub(crate) fn new(
        candidate: CandidateRef,
        branch_disposition: BranchDisposition,
        evaluation_artifact_path: PathBuf,
        comparisons: Vec<RunComparison>,
    ) -> Self {
        Self {
            candidate,
            branch_disposition,
            evaluation_artifact_path,
            comparisons,
        }
    }
}

/// Absolute safety verdict from an artifact-bound candidate patch review.
///
/// This is deliberately distinct from the comparative `domains::Verdict`.
/// A patch can be operationally better than its parent while still being
/// inadmissible for successor authority.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PatchVerdict {
    Admissible,
    Rejected,
    Inconclusive,
}

/// One path in the exact change set supplied to the patch adjudicator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PatchChange {
    pub(crate) relpath: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) proposed_content_hash: Option<String>,
}

/// Compact, hash-bound projection of the persisted candidate patch review.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PatchReview {
    pub(crate) schema_version: u32,
    pub(crate) procedure_id: String,
    pub(crate) candidate: CandidateRef,
    pub(crate) artifact_id: ArtifactId,
    pub(crate) artifact_surface_hash: HistoryHash,
    pub(crate) evaluation_hash: HistoryHash,
    pub(crate) config_hash: HistoryHash,
    pub(crate) change_set_hash: HistoryHash,
    pub(crate) changes: Vec<PatchChange>,
    pub(crate) verdict: PatchVerdict,
    pub(crate) confidence: Confidence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) blocking_findings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) missing_evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) rationale: Vec<String>,
    pub(crate) citation: SealedEvidenceCitation,
}

/// Parent-vs-child metrics for one benchmark instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RunComparison {
    pub(crate) instance_id: String,
    pub(crate) parent_metrics: Option<OperationalRunMetrics>,
    pub(crate) child_metrics: Option<OperationalRunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) oracle_evaluation: Option<crate::mbe::OracleEvaluation>,
    pub(crate) status: String,
}

/// One field-level operational comparison.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct MetricComparison {
    pub(crate) metric: String,
    pub(crate) parent: String,
    pub(crate) child: String,
    pub(crate) direction: MetricDirection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MetricDirection {
    Improved,
    Regressed,
    Unchanged,
}

impl MetricComparison {
    pub(crate) fn improved(metric: &str, parent: impl ToString, child: impl ToString) -> Self {
        Self::new(metric, parent, child, MetricDirection::Improved)
    }

    pub(crate) fn regressed(metric: &str, parent: impl ToString, child: impl ToString) -> Self {
        Self::new(metric, parent, child, MetricDirection::Regressed)
    }

    fn new(
        metric: &str,
        parent: impl ToString,
        child: impl ToString,
        direction: MetricDirection,
    ) -> Self {
        Self {
            metric: metric.to_string(),
            parent: parent.to_string(),
            child: child.to_string(),
            direction,
        }
    }
}
