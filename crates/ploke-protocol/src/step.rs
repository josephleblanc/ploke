use async_trait::async_trait;
use serde::Serialize;

use crate::core::{EvidencePolicy, ExecutorKind, StepArtifact};

pub trait StepSpec {
    type Input: Clone + Serialize;
    type Output: Clone + Serialize;

    fn step_id(&self) -> &'static str;

    fn step_name(&self) -> &'static str;

    fn evidence_policy(&self) -> EvidencePolicy {
        EvidencePolicy::default()
    }
}

pub struct StepExecution<Output, Provenance> {
    pub output: Output,
    pub provenance: Provenance,
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
        input: Spec::Input,
    ) -> Result<StepExecution<Spec::Output, Self::Provenance>, Self::Error>;
}

pub trait MechanizedSpec: StepSpec {
    type Error;

    fn execute_mechanized(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;
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
    Spec::Input: Send,
    Spec::Output: Send,
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
        input: Spec::Input,
    ) -> Result<StepExecution<Spec::Output, Self::Provenance>, Self::Error> {
        let output = spec.execute_mechanized(input)?;
        Ok(StepExecution {
            output,
            provenance: MechanizedProvenance {
                strategy: "native_rust".to_string(),
            },
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
        input: Spec::Input,
    ) -> Result<StepArtifact<Spec::Input, Spec::Output, Exec::Provenance>, Exec::Error> {
        let captured_input = input.clone();
        let execution = self.executor.execute(&self.spec, input).await?;
        Ok(StepArtifact {
            step_id: self.spec.step_id().to_string(),
            step_name: self.spec.step_name().to_string(),
            executor_kind: self.executor.kind(),
            executor_label: self.executor.label().to_string(),
            evidence_policy: self.spec.evidence_policy(),
            input: captured_input,
            output: execution.output,
            provenance: execution.provenance,
        })
    }
}
