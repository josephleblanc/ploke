//! Operator-facing successor-selection projections.
//!
//! The active Prototype 1 parent does not use this module to choose a
//! successor. Active selection flows through History traversal. These helpers
//! preserve older generation-shaped summaries for score/report views that need
//! to explain how the legacy generation-local policy would have classified a
//! set of inputs.

use super::{SelectionInput, SuccessorDecision, decide, decision};
use crate::BranchDisposition;

/// Summarize one generation's child evidence for operator projections.
///
/// This is intentionally not an active selector. It returns the same
/// `SuccessorDecision` record shape so projection views can compare individual
/// candidate decisions with the legacy generation-local summary.
pub(crate) fn generation_summary(inputs: Vec<SelectionInput>) -> Option<SuccessorDecision> {
    let mut keep_exploration = Vec::new();
    let mut rejected = Vec::new();

    for input in inputs {
        let decision = decide(input.clone());
        if decision.outcome == decision::SuccessorOutcome::Accepted {
            return Some(decision);
        }
        if decision.outcome == decision::SuccessorOutcome::ExploreFrom {
            keep_exploration.push((exploration_score(&input), decision));
            continue;
        }
        if input.branch_disposition == BranchDisposition::Reject {
            rejected.push((exploration_score(&input), input, decision));
        }
    }

    if let Some((_, decision)) = keep_exploration.into_iter().max_by_key(|(score, _)| *score) {
        return Some(decision);
    }

    rejected
        .into_iter()
        .max_by_key(|(score, _, _)| *score)
        .map(|(_, input, mut decision)| {
            decision.outcome = decision::SuccessorOutcome::ExploreFrom;
            decision.selected_branch_id = Some(input.candidate.branch_id.clone());
            decision.rationale.push(format!(
                "projection summary: no accepted child in generation {}; selected rejected child as exploration coordinate",
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
