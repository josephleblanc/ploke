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

/// Select from one generation's child evidence.
///
/// This keeps acceptance and exploration distinct: the first accepted child wins
/// immediately, while a rejected child can only be selected as an exploration
/// coordinate after no accepted child exists in the provided generation set.
pub(crate) fn decide_generation(inputs: Vec<SelectionInput>) -> Option<SuccessorDecision> {
    let mut rejected = Vec::new();

    for input in inputs {
        let decision = decide(input.clone());
        if decision.outcome == decision::SuccessorOutcome::Accepted {
            return Some(decision);
        }
        if input.branch_disposition == BranchDisposition::Reject {
            rejected.push((exploration_score(&input), input, decision));
        }
    }

    rejected
        .into_iter()
        .max_by_key(|(score, _, _)| *score)
        .map(|(_, input, mut decision)| {
            decision.outcome = decision::SuccessorOutcome::ExploreFrom;
            decision.selected_branch_id = Some(input.candidate.branch_id.clone());
            decision.rationale.push(format!(
                "no accepted child in generation {}; selected rejected child as exploration coordinate",
                input.candidate.generation
            ));
            decision
        })
}

fn exploration_score(input: &SelectionInput) -> (usize, usize, usize, usize, usize, usize) {
    let mut oracle_eligible = 0;
    let mut converged = 0;
    let mut nonempty = 0;
    let mut patch_attempted = 0;
    let mut failed_tool_calls = 0;
    let mut total_tool_calls = 0;

    for metrics in input
        .comparisons
        .iter()
        .filter_map(|comparison| comparison.child_metrics.as_ref())
    {
        oracle_eligible += metrics.oracle_eligible as usize;
        converged += metrics.convergence as usize;
        nonempty += metrics.nonempty_valid_patch as usize;
        patch_attempted += metrics.patch_attempted as usize;
        failed_tool_calls += metrics.tool_calls_failed;
        total_tool_calls += metrics.tool_calls_total;
    }

    (
        oracle_eligible,
        converged,
        nonempty,
        patch_attempted,
        usize::MAX.saturating_sub(failed_tool_calls),
        usize::MAX.saturating_sub(total_tool_calls),
    )
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
    use crate::intervention::Prototype1SelectionPolicyOutcome;

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

    #[test]
    fn generation_selection_accepts_first_keep_child() {
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

        let decision =
            super::decide_generation(vec![rejected, accepted]).expect("generation decision");

        assert_eq!(decision.outcome, SuccessorOutcome::Accepted);
        assert_eq!(decision.selected_branch_id.as_deref(), Some("branch-keep"));
        assert_eq!(
            decision.selection_policy_outcome(),
            Some(Prototype1SelectionPolicyOutcome::Accepted)
        );
    }

    #[test]
    fn generation_selection_explores_best_rejected_child_when_none_accepted() {
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

        let decision =
            super::decide_generation(vec![weaker, stronger]).expect("generation decision");

        assert_eq!(decision.outcome, SuccessorOutcome::ExploreFrom);
        assert_eq!(
            decision.selected_branch_id.as_deref(),
            Some("branch-stronger")
        );
        assert_eq!(
            decision.selection_policy_outcome(),
            Some(Prototype1SelectionPolicyOutcome::ExploreFromRejected)
        );
        assert_eq!(decision.selected_branch_disposition(), Some("reject"));
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
