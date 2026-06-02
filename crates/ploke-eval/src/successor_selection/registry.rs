use serde::{Deserialize, Serialize};

use super::decision::SuccessorDecision;
use super::domains::operational::OperationalDomain;
use super::domains::oracle::OracleDomain;
use super::domains::{Domain, DomainFinding};
use super::evidence::SelectionInput;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DomainKind {
    Operational,
    Oracle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SelectionRegistry {
    domains: Vec<DomainKind>,
}

impl Default for SelectionRegistry {
    fn default() -> Self {
        Self {
            domains: vec![DomainKind::Operational, DomainKind::Oracle],
        }
    }
}

impl SelectionRegistry {
    pub(crate) fn decide(&self, input: SelectionInput) -> SuccessorDecision {
        let findings = self
            .domains
            .iter()
            .map(|domain| evaluate_domain(*domain, &input))
            .collect();
        SuccessorDecision::from_findings(&input, findings)
    }
}

fn evaluate_domain(domain: DomainKind, input: &SelectionInput) -> DomainFinding {
    match domain {
        DomainKind::Operational => OperationalDomain.evaluate(input),
        DomainKind::Oracle => OracleDomain.evaluate(input),
    }
}
