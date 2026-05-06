use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{BranchDisposition, OperationalRunMetrics};

use super::CandidateRef;

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

/// Parent-vs-child metrics for one benchmark instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RunComparison {
    pub(crate) instance_id: String,
    pub(crate) parent_metrics: Option<OperationalRunMetrics>,
    pub(crate) child_metrics: Option<OperationalRunMetrics>,
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
