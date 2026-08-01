use serde::{Deserialize, Serialize};

/// A metric names a measurable quantity together with its subject and value
/// domain.
pub trait Measurement {
    type Subject;
    type Value;

    fn name(&self) -> &'static str;
}

/// Marker trait for typed procedure states.
pub trait ProcedureState: Clone + Serialize + Send + Sync + std::fmt::Debug + 'static {}

impl<T> ProcedureState for T where T: Clone + Serialize + Send + Sync + std::fmt::Debug + 'static {}

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

/// Recording and forwarding are separate concerns in the procedure model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateDisposition {
    RecordOnly,
    ForwardOnly,
    RecordAndForward,
    Ephemeral,
}

impl StateDisposition {
    pub fn should_record(self) -> bool {
        matches!(self, Self::RecordOnly | Self::RecordAndForward)
    }

    pub fn should_forward(self) -> bool {
        matches!(self, Self::ForwardOnly | Self::RecordAndForward)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateEnvelope<State> {
    pub state: State,
    pub disposition: StateDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepArtifact<InputState, OutputState, Provenance> {
    pub step_id: String,
    pub step_name: String,
    pub executor_kind: ExecutorKind,
    pub executor_label: String,
    pub evidence_policy: EvidencePolicy,
    pub input: InputState,
    pub input_disposition: StateDisposition,
    pub output: OutputState,
    pub output_disposition: StateDisposition,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceArtifact<First, Second> {
    pub first: First,
    pub second: Second,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForkState<SourceState, LeftState, RightState> {
    pub source: SourceState,
    pub left: LeftState,
    pub right: RightState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FanOutArtifact<InputState, LeftArtifact, RightArtifact> {
    pub input: StateEnvelope<InputState>,
    pub left_branch: String,
    pub right_branch: String,
    pub left: LeftArtifact,
    pub right: RightArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeArtifact<BranchesArtifact, MergeStepArtifact> {
    pub branches: BranchesArtifact,
    pub merge: MergeStepArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcedureArtifact<Artifact> {
    pub procedure_name: String,
    pub artifact: Artifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcedureRun<OutputState, Artifact> {
    pub procedure_name: String,
    pub output: OutputState,
    pub artifact: Artifact,
}
