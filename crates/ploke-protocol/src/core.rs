use serde::{Deserialize, Serialize};

/// A metric names a measurable quantity together with its subject and value
/// domain.
pub trait Measurement {
    type Subject;
    type Value;

    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorKind {
    Mechanized,
    LlmAdjudicator,
    HumanReviewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// Encodes the admissible evidence contract for a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvidencePolicy {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden: Vec<String>,
    #[serde(default)]
    pub hindsight_allowed: bool,
    #[serde(default)]
    pub external_context_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepArtifact<Input, Output, Provenance> {
    pub step_id: String,
    pub step_name: String,
    pub executor_kind: ExecutorKind,
    pub executor_label: String,
    pub evidence_policy: EvidencePolicy,
    pub input: Input,
    pub output: Output,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceArtifact<First, Second> {
    pub first: First,
    pub second: Second,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FanOutArtifact<Left, Right> {
    pub left: Left,
    pub right: Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcedureArtifact<Artifact> {
    pub procedure_name: String,
    pub artifact: Artifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcedureRun<Output, Artifact> {
    pub procedure_name: String,
    pub output: Output,
    pub artifact: Artifact,
}
