use serde::{Deserialize, Serialize};

use super::evidence::SelectionInput;

pub mod operational;

pub(crate) trait Domain {
    fn name(&self) -> DomainName;
    fn evaluate(&self, input: &SelectionInput) -> DomainFinding;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DomainName {
    Operational,
    Protocol,
    Patch,
    Oracle,
    Adjudication,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Verdict {
    Better,
    Worse,
    Mixed,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct DomainFinding {
    pub(crate) domain: DomainName,
    pub(crate) verdict: Verdict,
    pub(crate) confidence: Confidence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) metrics: Vec<super::evidence::MetricComparison>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) rationale: Vec<String>,
}
