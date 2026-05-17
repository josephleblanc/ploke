//! Passive successor-selection record DTOs.
//!
//! These records describe selector inputs, domain findings, and decisions. They
//! do not select a successor or authorize handoff.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::branch::Disposition;
use crate::evaluation::RunMetrics;
use crate::oracle;

/// Candidate coordinate considered by successor selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateRef {
    pub node_id: String,
    pub branch_id: String,
    pub generation: u32,
}

/// Generation-local evidence bundle available to the successor selector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Input {
    pub candidate: CandidateRef,
    pub branch_disposition: Disposition,
    pub evaluation_artifact_path: PathBuf,
    #[serde(default)]
    pub comparisons: Vec<RunComparison>,
}

/// Parent-vs-child metrics for one benchmark instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunComparison {
    pub instance_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_metrics: Option<RunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_metrics: Option<RunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_evaluation: Option<oracle::Evaluation>,
    pub status: String,
}

/// One field-level operational comparison.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricComparison {
    pub metric: String,
    pub parent: String,
    pub child: String,
    pub direction: MetricDirection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricDirection {
    Improved,
    Regressed,
    Unchanged,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DomainName {
    Operational,
    Protocol,
    Patch,
    Oracle,
    Adjudication,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Better,
    Worse,
    Mixed,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// Per-domain selector finding considered by a decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainFinding {
    pub domain: DomainName,
    pub verdict: Verdict,
    pub confidence: Confidence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<MetricComparison>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rationale: Vec<String>,
}

/// Selector outcome label.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Accepted,
    ExploreFrom,
    Stop,
}

/// Durable selector decision projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Decision {
    pub procedure_id: String,
    pub candidate_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_branch_id: Option<String>,
    pub branch_disposition: String,
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<DomainFinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rationale: Vec<String>,
}
