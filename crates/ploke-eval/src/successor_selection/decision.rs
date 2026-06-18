use serde::{Deserialize, Serialize};

use super::domains::{DomainFinding, Verdict};
use super::evidence::SelectionInput;
use super::{PROCEDURE_ID, disposition_as_str};
use crate::BranchDisposition;

/// Parent-side successor-selection record.
///
/// `branch_disposition`/`outcome` explain how the branch scored; they are not
/// the same question as "may this coordinate receive successor authority?"
/// With traversal policies such as `explore_from_rejected`, a branch whose
/// evaluation disposition is `reject` can still be selected as the next Parent
/// coordinate. See:
///
/// - `docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`
/// - `docs/workflow/evalnomicon/drafts/runtime/child.md`
/// - `docs/workflow/evalnomicon/chat-history/on-hyper-agents.md`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SuccessorDecision {
    pub(crate) procedure_id: String,
    pub(crate) candidate_node_id: String,
    /// Branch coordinate selected for successor/traversal authority, if any.
    ///
    /// Do not interpret this as "branch evaluation kept the child". In History
    /// traversal, the selected coordinate may carry `branch_disposition` equal
    /// to `"reject"` when policy intentionally explores from rejected children.
    pub(crate) selected_branch_id: Option<String>,
    /// Branch evaluation disposition recorded with the selection evidence.
    ///
    /// This remains the evaluation answer; selection may still proceed from a
    /// rejected branch when traversal policy allows it.
    pub(crate) branch_disposition: String,
    pub(crate) outcome: SuccessorOutcome,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) findings: Vec<DomainFinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) rationale: Vec<String>,
}

impl SuccessorDecision {
    pub(crate) fn from_findings(input: &SelectionInput, findings: Vec<DomainFinding>) -> Self {
        let operational = findings
            .iter()
            .find(|finding| finding.domain == super::domains::DomainName::Operational);

        let outcome = match (
            input.branch_disposition.clone(),
            operational.map(|finding| finding.verdict),
        ) {
            (_, Some(Verdict::Better)) => SuccessorOutcome::Accepted,
            (BranchDisposition::Keep, Some(Verdict::Mixed)) => SuccessorOutcome::ExploreFrom,
            (_, Some(Verdict::Mixed)) => SuccessorOutcome::Stop,
            (_, Some(Verdict::Worse)) => SuccessorOutcome::Stop,
            _ => SuccessorOutcome::Stop,
        };

        let selected_branch_id = match outcome {
            SuccessorOutcome::Accepted | SuccessorOutcome::ExploreFrom => {
                Some(input.candidate.branch_id.clone())
            }
            SuccessorOutcome::Stop => None,
        };

        let rationale = vec![format!(
            "operational verdict selected outcome={:?} for branch_disposition={}",
            outcome,
            disposition_as_str(input.branch_disposition.clone())
        )];

        Self {
            procedure_id: PROCEDURE_ID.to_string(),
            candidate_node_id: input.candidate.node_id.clone(),
            selected_branch_id,
            branch_disposition: disposition_as_str(input.branch_disposition.clone()).to_string(),
            outcome,
            findings,
            rationale,
        }
    }

    pub(crate) fn selected_branch_disposition(&self) -> Option<&str> {
        self.selected_branch_id
            .as_ref()
            .map(|_| self.branch_disposition.as_str())
    }

    #[cfg(test)]
    pub(crate) fn selects_successor(&self) -> bool {
        self.selected_branch_id.is_some()
            && matches!(
                self.outcome,
                SuccessorOutcome::Accepted | SuccessorOutcome::ExploreFrom
            )
    }

    #[cfg(test)]
    pub(crate) fn selects_keep_successor(&self) -> bool {
        self.selects_successor() && self.branch_disposition == "keep"
    }
}

/// Candidate-level scoring outcome.
///
/// History traversal may still bind `selected_branch_id` for an exploratory
/// successor when this is `Stop`; that binding records parent traversal policy,
/// not a branch-evaluation keep verdict.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SuccessorOutcome {
    Accepted,
    ExploreFrom,
    Stop,
}
