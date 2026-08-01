//! Shared evidence class labels for Prototype 1 filesystem/importer records.
//!
//! Kept separate from [`super::history_preview`] so reporting schemas such as
//! [`super::evidence_inventory`] do not depend on the preview projection module.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceClass {
    TransitionJournal,
    Evaluation,
    Invocation,
    AttemptResult,
    SuccessorReady,
    SuccessorCompletion,
    Scheduler,
    BranchRegistry,
    NodeRecord,
    RunnerRequest,
    RunnerResult,
}

impl EvidenceClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::TransitionJournal => "transition_journal",
            Self::Evaluation => "evaluation",
            Self::Invocation => "invocation",
            Self::AttemptResult => "attempt_result",
            Self::SuccessorReady => "successor_ready",
            Self::SuccessorCompletion => "successor_completion",
            Self::Scheduler => "scheduler",
            Self::BranchRegistry => "branch_registry",
            Self::NodeRecord => "node_record",
            Self::RunnerRequest => "runner_request",
            Self::RunnerResult => "runner_result",
        }
    }

    /// How the **history preview importer** treats raw filesystem records (not sealed History authority).
    pub(crate) fn preview_import_treatment(self) -> &'static str {
        match self {
            Self::TransitionJournal => "admitted_preview",
            Self::Evaluation => "admitted_preview_raw",
            Self::Invocation => "admitted_preview_raw",
            Self::AttemptResult => "admitted_preview_raw",
            Self::SuccessorReady => "admitted_preview_raw_or_ingress",
            Self::SuccessorCompletion => "admitted_preview_raw",
            Self::Scheduler => "projection_only",
            Self::BranchRegistry => "projection_plus_evidence_refs",
            Self::NodeRecord => "projection_or_degraded_evidence",
            Self::RunnerRequest => "admitted_preview_raw",
            Self::RunnerResult => "projection_unless_attempt_result_missing",
        }
    }
}
