//! Successor-selection evidence and decision procedures.
//!
//! This module is intentionally separate from the operator-facing active
//! selection cache in `crate::selection`. It models generation-local evidence
//! used by a ruling parent when deciding whether a completed child should be
//! selected as the next successor.

use std::path::PathBuf;

use crate::BranchDisposition;

pub mod decision;
pub mod domains;
pub mod evidence;
pub mod registry;

pub(crate) use decision::SuccessorDecision;
pub(crate) use evidence::{RunComparison, SelectionInput};
pub(crate) use registry::SelectionRegistry;

pub(crate) const PROCEDURE_ID: &str = "successor-selection:v1";

/// Build the default first-pass successor decision from available evidence.
pub(crate) fn decide(input: SelectionInput) -> SuccessorDecision {
    SelectionRegistry::default().decide(input)
}

pub(crate) fn evidence_ref(path: impl Into<PathBuf>) -> String {
    format!("path:{}", path.into().display())
}

fn disposition_as_str(disposition: BranchDisposition) -> &'static str {
    match disposition {
        BranchDisposition::Keep => "keep",
        BranchDisposition::Reject => "reject",
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct CandidateRef {
    pub(crate) node_id: String,
    pub(crate) branch_id: String,
    pub(crate) generation: u32,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::record::SubmissionArtifactState;
    use crate::{BranchDisposition, OperationalRunMetrics, PatchApplyState};

    use super::decision::SuccessorOutcome;
    use super::{CandidateRef, RunComparison, SelectionInput, decide};

    #[test]
    fn operational_selection_selects_child_with_keep_and_no_regression() {
        let parent = metrics(false, false, 3);
        let child = metrics(true, true, 1);
        let decision = decide(input(BranchDisposition::Keep, parent, child));

        assert_eq!(decision.outcome, SuccessorOutcome::Select);
        assert_eq!(decision.selected_branch_id.as_deref(), Some("branch-child"));
        assert_eq!(decision.selected_branch_disposition(), Some("keep"));
    }

    #[test]
    fn operational_selection_stops_rejected_child() {
        let parent = metrics(true, true, 0);
        let child = metrics(false, false, 4);
        let decision = decide(input(BranchDisposition::Reject, parent, child));

        assert_eq!(decision.outcome, SuccessorOutcome::Stop);
        assert_eq!(decision.selected_branch_id, None);
        assert_eq!(decision.selected_branch_disposition(), None);
    }

    #[test]
    fn operational_selection_stops_keep_without_comparable_metrics() {
        let decision = decide(SelectionInput::new(
            CandidateRef {
                node_id: "node-child".to_string(),
                branch_id: "branch-child".to_string(),
                generation: 1,
            },
            BranchDisposition::Keep,
            PathBuf::from("evaluations/branch-child.json"),
            vec![RunComparison {
                instance_id: "instance-a".to_string(),
                parent_metrics: None,
                child_metrics: None,
                status: "missing_metrics".to_string(),
            }],
        ));

        assert_eq!(decision.outcome, SuccessorOutcome::Stop);
        assert_eq!(decision.selected_branch_id, None);
        assert_eq!(decision.selected_branch_disposition(), None);
    }

    fn input(
        branch_disposition: BranchDisposition,
        parent: OperationalRunMetrics,
        child: OperationalRunMetrics,
    ) -> SelectionInput {
        SelectionInput::new(
            CandidateRef {
                node_id: "node-child".to_string(),
                branch_id: "branch-child".to_string(),
                generation: 1,
            },
            branch_disposition,
            PathBuf::from("evaluations/branch-child.json"),
            vec![RunComparison {
                instance_id: "instance-a".to_string(),
                parent_metrics: Some(parent),
                child_metrics: Some(child),
                status: "compared".to_string(),
            }],
        )
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
