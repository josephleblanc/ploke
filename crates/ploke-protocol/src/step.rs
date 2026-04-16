use async_trait::async_trait;
use serde::Serialize;

use crate::core::{EvidencePolicy, ExecutorKind, ProcedureState, StateDisposition, StepArtifact};

pub trait StepSpec {
    type InputState: ProcedureState;
    type OutputState: ProcedureState;

    fn step_id(&self) -> &'static str;

    fn step_name(&self) -> &'static str;

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy::default()
    }

    fn disposition(&self) -> StateDisposition {
        StateDisposition::RecordAndForward
    }
}

pub struct StepExecution<OutputState, Provenance> {
    pub state: OutputState,
    pub provenance: Provenance,
    pub disposition: StateDisposition,
}

#[async_trait]
pub trait StepExecutor<Spec>: Send + Sync
where
    Spec: StepSpec + Send + Sync,
{
    type Provenance: Clone + Serialize + Send + Sync;
    type Error: Send;

    fn kind(&self) -> ExecutorKind;

    fn label(&self) -> &'static str;

    async fn execute(
        &self,
        spec: &Spec,
        input: Spec::InputState,
    ) -> Result<StepExecution<Spec::OutputState, Self::Provenance>, Self::Error>;
}

pub trait MechanizedSpec: StepSpec {
    type Error;

    fn execute_mechanized(&self, input: Self::InputState)
    -> Result<Self::OutputState, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MechanizedProvenance {
    pub strategy: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MechanizedExecutor;

#[async_trait]
impl<Spec> StepExecutor<Spec> for MechanizedExecutor
where
    Spec: MechanizedSpec + Send + Sync,
    Spec::InputState: Send,
    Spec::OutputState: Send,
    Spec::Error: Send,
{
    type Provenance = MechanizedProvenance;
    type Error = Spec::Error;

    fn kind(&self) -> ExecutorKind {
        ExecutorKind::Mechanized
    }

    fn label(&self) -> &'static str {
        "native_rust"
    }

    async fn execute(
        &self,
        spec: &Spec,
        input: Spec::InputState,
    ) -> Result<StepExecution<Spec::OutputState, Self::Provenance>, Self::Error> {
        let state = spec.execute_mechanized(input)?;
        Ok(StepExecution {
            state,
            provenance: MechanizedProvenance {
                strategy: "native_rust".to_string(),
            },
            disposition: spec.disposition(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Step<Spec, Exec> {
    pub spec: Spec,
    pub executor: Exec,
}

impl<Spec, Exec> Step<Spec, Exec> {
    pub fn new(spec: Spec, executor: Exec) -> Self {
        Self { spec, executor }
    }
}

impl<Spec, Exec> Step<Spec, Exec>
where
    Spec: StepSpec + Send + Sync,
    Exec: StepExecutor<Spec>,
{
    pub async fn run(
        &self,
        input: Spec::InputState,
    ) -> Result<StepArtifact<Spec::InputState, Spec::OutputState, Exec::Provenance>, Exec::Error>
    {
        let captured_input = input.clone();
        let execution = self.executor.execute(&self.spec, input).await?;
        Ok(StepArtifact {
            step_id: self.spec.step_id().to_string(),
            step_name: self.spec.step_name().to_string(),
            executor_kind: self.executor.kind(),
            executor_label: self.executor.label().to_string(),
            evidence_policy: self.spec.evidence_policy(),
            input: captured_input,
            input_disposition: StateDisposition::ForwardOnly,
            output: execution.state,
            output_disposition: execution.disposition,
            provenance: execution.provenance,
        })
    }
}
