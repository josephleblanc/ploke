//! Successor-selection evidence and decision procedures.
//!
//! This module is intentionally separate from the operator-facing active
//! selection cache in `crate::selection`. It models candidate-local evidence
//! used by traversal strategies when deciding whether a completed child can be
//! selected as the next successor coordinate.
//!
//! Active Prototype 1 selection now flows through History-backed traversal with
//! current-generation candidates appended into the same candidate set. Selection
//! evidence remains evidence: the Crown/History path is what makes a selected
//! successor authoritative for a lineage.
//!
//! History-backed traversal policy lives in [`traversal`]. Its scoring
//! algorithms should start from `traversal::CandidateCase`, not only from the
//! generation-local [`SelectionInput`], because History payloads can also carry
//! sealed evaluation, runtime, branch, citation, diagnostic, and Artifact
//! material.

use std::path::PathBuf;

use crate::BranchDisposition;

pub mod decision;
pub mod domains;
pub mod evidence;
pub mod metrics;
pub mod operator_projection;
pub mod registry;
pub mod traversal;

pub(crate) use decision::SuccessorDecision;
pub(crate) use evidence::{RunComparison, SelectionInput};
pub(crate) use registry::SelectionRegistry;

pub(crate) const PROCEDURE_ID: &str = "successor-selection:v1";
pub(crate) const HISTORY_TRAVERSAL_PROCEDURE_ID: &str = "successor-selection:history-traversal:v1";

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum OracleMode {
    #[default]
    RecordOnly,
    RelativeScore,
}

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

        assert_eq!(decision.outcome, SuccessorOutcome::Accepted);
        assert_eq!(decision.selected_branch_id.as_deref(), Some("branch-child"));
        assert_eq!(decision.selected_branch_disposition(), Some("keep"));
    }

    #[test]
    fn operational_selection_explores_keep_child_with_mixed_metrics() {
        let parent = metrics(false, false, 0);
        let child = metrics(true, true, 2);
        let decision = decide(input(BranchDisposition::Keep, parent, child));

        assert_eq!(decision.outcome, SuccessorOutcome::ExploreFrom);
        assert_eq!(decision.selected_branch_id.as_deref(), Some("branch-child"));
        assert_eq!(decision.selected_branch_disposition(), Some("keep"));
        assert!(decision.selects_successor());
        assert!(decision.selects_keep_successor());
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
                oracle_evaluation: None,
                status: "missing_metrics".to_string(),
            }],
        ));

        assert_eq!(decision.outcome, SuccessorOutcome::Stop);
        assert_eq!(decision.selected_branch_id, None);
        assert_eq!(decision.selected_branch_disposition(), None);
    }

    #[test]
    fn operator_projection_generation_summary_accepts_first_keep_child() {
        let rejected = input_for(
            "node-reject",
            "branch-reject",
            BranchDisposition::Reject,
            metrics(false, false, 1),
            metrics(true, true, 1),
        );
        let accepted = input_for(
            "node-keep",
            "branch-keep",
            BranchDisposition::Keep,
            metrics(false, false, 1),
            metrics(true, true, 0),
        );

        let decision = super::operator_projection::generation_summary(vec![rejected, accepted])
            .expect("generation projection summary");

        assert_eq!(decision.outcome, SuccessorOutcome::Accepted);
        assert_eq!(decision.selected_branch_id.as_deref(), Some("branch-keep"));
        assert!(decision.selects_keep_successor());
    }

    #[test]
    fn operator_projection_generation_summary_explores_best_rejected_child_when_none_accepted() {
        let weaker = input_for(
            "node-weaker",
            "branch-weaker",
            BranchDisposition::Reject,
            metrics(false, false, 1),
            metrics(false, false, 4),
        );
        let stronger = input_for(
            "node-stronger",
            "branch-stronger",
            BranchDisposition::Reject,
            metrics(false, false, 1),
            metrics(true, true, 1),
        );

        let decision = super::operator_projection::generation_summary(vec![weaker, stronger])
            .expect("generation projection summary");

        assert_eq!(decision.outcome, SuccessorOutcome::ExploreFrom);
        assert_eq!(
            decision.selected_branch_id.as_deref(),
            Some("branch-stronger")
        );
        assert!(decision.selects_successor());
        assert!(!decision.selects_keep_successor());
        assert_eq!(decision.selected_branch_disposition(), Some("reject"));
    }

    #[test]
    fn operator_projection_generation_summary_prefers_mixed_keep_over_rejected_exploration() {
        let rejected = input_for(
            "node-reject",
            "branch-reject",
            BranchDisposition::Reject,
            metrics(false, false, 1),
            metrics(true, true, 1),
        );
        let mixed_keep = input_for(
            "node-keep-mixed",
            "branch-keep-mixed",
            BranchDisposition::Keep,
            metrics(false, false, 0),
            metrics(true, true, 2),
        );

        let decision = super::operator_projection::generation_summary(vec![rejected, mixed_keep])
            .expect("generation projection summary");

        assert_eq!(decision.outcome, SuccessorOutcome::ExploreFrom);
        assert_eq!(
            decision.selected_branch_id.as_deref(),
            Some("branch-keep-mixed")
        );
        assert!(decision.selects_keep_successor());
        assert_eq!(decision.selected_branch_disposition(), Some("keep"));
    }

    fn input(
        branch_disposition: BranchDisposition,
        parent: OperationalRunMetrics,
        child: OperationalRunMetrics,
    ) -> SelectionInput {
        input_for(
            "node-child",
            "branch-child",
            branch_disposition,
            parent,
            child,
        )
    }

    fn input_for(
        node_id: &str,
        branch_id: &str,
        branch_disposition: BranchDisposition,
        parent: OperationalRunMetrics,
        child: OperationalRunMetrics,
    ) -> SelectionInput {
        SelectionInput::new(
            CandidateRef {
                node_id: node_id.to_string(),
                branch_id: branch_id.to_string(),
                generation: 1,
            },
            branch_disposition,
            PathBuf::from(format!("evaluations/{branch_id}.json")),
            vec![RunComparison {
                instance_id: "instance-a".to_string(),
                parent_metrics: Some(parent),
                child_metrics: Some(child),
                oracle_evaluation: None,
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
            patch_projection_check_state: if oracle_eligible {
                ploke_records::evaluation::PatchProjectionCheckState::Passed
            } else {
                ploke_records::evaluation::PatchProjectionCheckState::NotApplicable
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
