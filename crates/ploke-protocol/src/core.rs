use serde::{Deserialize, Serialize};

/// A metric names a measurable quantity together with its subject and value
/// domain.
pub trait Measurement {
    type Subject;
    type Value;

    fn name(&self) -> &'static str;
}

/// A protocol is a bounded procedure for producing one typed output from one
/// typed subject.
pub trait Protocol {
    type Subject;
    type State;
    type Output;
    type Error;

    fn name(&self) -> &'static str;

    fn run(&self, subject: Self::Subject) -> Result<Self::Output, Self::Error>;
}

/// A single typed step inside a larger protocol.
pub trait ProtocolStep {
    type Input;
    type Output;
    type Error;

    fn name(&self) -> &'static str;

    fn run(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;
}

/// An executor carries out a step specification. Some executors are
/// deterministic code, others may be LLM-backed or human-supervised.
pub trait Executor<Spec, Input, Output> {
    type Error;

    fn kind(&self) -> ExecutorKind;

    fn execute(&self, spec: &Spec, input: Input) -> Result<Output, Self::Error>;
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

/// Minimal persisted artifact for a protocol run. Higher-level consumers can
/// wrap this with protocol-specific input/output payloads and raw model traces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolArtifact {
    pub protocol_name: String,
    pub executor_kind: ExecutorKind,
    pub subject_id: String,
}
