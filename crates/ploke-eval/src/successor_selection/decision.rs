use serde::{Deserialize, Serialize};

use super::domains::{DomainFinding, Verdict};
use super::evidence::SelectionInput;
use super::{PROCEDURE_ID, disposition_as_str};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SuccessorDecision {
    pub(crate) procedure_id: String,
    pub(crate) candidate_node_id: String,
    pub(crate) selected_branch_id: Option<String>,
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

        let outcome = match operational.map(|finding| finding.verdict) {
            Some(Verdict::Better) => SuccessorOutcome::Select,
            Some(Verdict::Mixed) => SuccessorOutcome::ContinueWithRisk,
            Some(Verdict::Worse) => SuccessorOutcome::Stop,
            _ => SuccessorOutcome::Stop,
        };

        let selected_branch_id = match outcome {
            SuccessorOutcome::Select | SuccessorOutcome::ContinueWithRisk => {
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SuccessorOutcome {
    Select,
    ContinueWithRisk,
    Stop,
}
