use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

use crate::core::{FanOutArtifact, ProcedureArtifact, ProcedureRun, SequenceArtifact};
use crate::step::{Step, StepExecutor, StepSpec};

#[async_trait]
pub trait Procedure: Send + Sync {
    type Subject: Send;
    type Output: Clone + Serialize + Send;
    type Artifact: Clone + Serialize + Send;
    type Error: Send;

    fn name(&self) -> &'static str;

    async fn run(
        &self,
        subject: Self::Subject,
    ) -> Result<ProcedureRun<Self::Output, Self::Artifact>, Self::Error>;
}

#[async_trait]
impl<Spec, Exec> Procedure for Step<Spec, Exec>
where
    Spec: StepSpec + Send + Sync,
    Spec::Input: Send,
    Spec::Output: Send,
    Exec: StepExecutor<Spec> + Send + Sync,
    Exec::Provenance: Send,
    Exec::Error: Send,
{
    type Subject = Spec::Input;
    type Output = Spec::Output;
    type Artifact = crate::core::StepArtifact<Spec::Input, Spec::Output, Exec::Provenance>;
    type Error = Exec::Error;

    fn name(&self) -> &'static str {
        self.spec.step_name()
    }

    async fn run(
        &self,
        subject: Self::Subject,
    ) -> Result<ProcedureRun<Self::Output, Self::Artifact>, Self::Error> {
        let artifact = Step::run(self, subject).await?;
        Ok(ProcedureRun {
            procedure_name: self.name().to_string(),
            output: artifact.output.clone(),
            artifact,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Sequence<First, Second> {
    pub first: First,
    pub second: Second,
}

#[derive(Debug, Error)]
pub enum SequenceError<FirstError, SecondError> {
    #[error("first procedure failed: {0}")]
    First(FirstError),
    #[error("second procedure failed: {0}")]
    Second(SecondError),
}

#[async_trait]
impl<First, Second> Procedure for Sequence<First, Second>
where
    First: Procedure,
    First::Output: Send,
    First::Artifact: Send,
    First::Error: Send,
    Second: Procedure<Subject = First::Output>,
    Second::Artifact: Send,
    Second::Error: Send,
{
    type Subject = First::Subject;
    type Output = Second::Output;
    type Artifact = SequenceArtifact<First::Artifact, Second::Artifact>;
    type Error = SequenceError<First::Error, Second::Error>;

    fn name(&self) -> &'static str {
        "sequence"
    }

    async fn run(
        &self,
        subject: Self::Subject,
    ) -> Result<ProcedureRun<Self::Output, Self::Artifact>, Self::Error> {
        let first = self
            .first
            .run(subject)
            .await
            .map_err(SequenceError::First)?;
        let second = self
            .second
            .run(first.output.clone())
            .await
            .map_err(SequenceError::Second)?;

        Ok(ProcedureRun {
            procedure_name: self.name().to_string(),
            output: second.output.clone(),
            artifact: SequenceArtifact {
                first: first.artifact,
                second: second.artifact,
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct FanOut<Left, Right> {
    pub left: Left,
    pub right: Right,
}

#[derive(Debug, Error)]
pub enum FanOutError<LeftError, RightError> {
    #[error("left branch failed: {0}")]
    Left(LeftError),
    #[error("right branch failed: {0}")]
    Right(RightError),
}

#[async_trait]
impl<Left, Right, Subject> Procedure for FanOut<Left, Right>
where
    Subject: Clone + Send + Sync + 'static,
    Left: Procedure<Subject = Subject>,
    Left::Artifact: Send,
    Left::Error: Send,
    Right: Procedure<Subject = Subject>,
    Right::Artifact: Send,
    Right::Error: Send,
{
    type Subject = Subject;
    type Output = (Left::Output, Right::Output);
    type Artifact = FanOutArtifact<Left::Artifact, Right::Artifact>;
    type Error = FanOutError<Left::Error, Right::Error>;

    fn name(&self) -> &'static str {
        "fan_out"
    }

    async fn run(
        &self,
        subject: Self::Subject,
    ) -> Result<ProcedureRun<Self::Output, Self::Artifact>, Self::Error> {
        // Execute sequentially for now while preserving the composition model.
        let left = self
            .left
            .run(subject.clone())
            .await
            .map_err(FanOutError::Left)?;
        let right = self.right.run(subject).await.map_err(FanOutError::Right)?;

        Ok(ProcedureRun {
            procedure_name: self.name().to_string(),
            output: (left.output.clone(), right.output.clone()),
            artifact: FanOutArtifact {
                left: left.artifact,
                right: right.artifact,
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct NamedProcedure<Inner> {
    pub name: &'static str,
    pub inner: Inner,
}

#[async_trait]
impl<Inner> Procedure for NamedProcedure<Inner>
where
    Inner: Procedure,
    Inner::Artifact: Send,
    Inner::Error: Send,
{
    type Subject = Inner::Subject;
    type Output = Inner::Output;
    type Artifact = ProcedureArtifact<Inner::Artifact>;
    type Error = Inner::Error;

    fn name(&self) -> &'static str {
        self.name
    }

    async fn run(
        &self,
        subject: Self::Subject,
    ) -> Result<ProcedureRun<Self::Output, Self::Artifact>, Self::Error> {
        let inner = self.inner.run(subject).await?;
        Ok(ProcedureRun {
            procedure_name: self.name().to_string(),
            output: inner.output,
            artifact: ProcedureArtifact {
                procedure_name: self.name().to_string(),
                artifact: inner.artifact,
            },
        })
    }
}

pub trait ProcedureExt: Procedure + Sized {
    fn then<Next>(self, next: Next) -> Sequence<Self, Next>
    where
        Next: Procedure<Subject = Self::Output>,
    {
        Sequence {
            first: self,
            second: next,
        }
    }

    fn fan_out<Right>(self, right: Right) -> FanOut<Self, Right>
    where
        Self::Subject: Clone + Send + Sync,
        Right: Procedure<Subject = Self::Subject>,
    {
        FanOut { left: self, right }
    }

    fn named(self, name: &'static str) -> NamedProcedure<Self> {
        NamedProcedure { name, inner: self }
    }
}

impl<T> ProcedureExt for T where T: Procedure + Sized {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EvidencePolicy;
    use crate::step::{MechanizedExecutor, MechanizedSpec, Step, StepSpec};

    #[derive(Clone, Copy)]
    struct AddOne;

    impl StepSpec for AddOne {
        type Input = i32;
        type Output = i32;

        fn step_id(&self) -> &'static str {
            "add_one"
        }

        fn step_name(&self) -> &'static str {
            "add_one"
        }

        fn evidence_policy(&self) -> EvidencePolicy {
            EvidencePolicy {
                allowed: vec!["input integer".to_string()],
                ..Default::default()
            }
        }
    }

    impl MechanizedSpec for AddOne {
        type Error = &'static str;

        fn execute_mechanized(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
            Ok(input + 1)
        }
    }

    #[derive(Clone, Copy)]
    struct Double;

    impl StepSpec for Double {
        type Input = i32;
        type Output = i32;

        fn step_id(&self) -> &'static str {
            "double"
        }

        fn step_name(&self) -> &'static str {
            "double"
        }
    }

    impl MechanizedSpec for Double {
        type Error = &'static str;

        fn execute_mechanized(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
            Ok(input * 2)
        }
    }

    #[tokio::test]
    async fn sequence_preserves_intermediate_artifacts() {
        let procedure = Step::new(AddOne, MechanizedExecutor)
            .then(Step::new(Double, MechanizedExecutor))
            .named("arithmetic");

        let run = procedure.run(3).await.expect("sequence should succeed");

        assert_eq!(run.procedure_name, "arithmetic");
        assert_eq!(run.output, 8);
        assert_eq!(run.artifact.artifact.first.input, 3);
        assert_eq!(run.artifact.artifact.first.output, 4);
        assert_eq!(run.artifact.artifact.second.input, 4);
        assert_eq!(run.artifact.artifact.second.output, 8);
    }

    #[tokio::test]
    async fn fan_out_keeps_both_branch_outputs() {
        let procedure = Step::new(AddOne, MechanizedExecutor)
            .fan_out(Step::new(Double, MechanizedExecutor))
            .named("branching_arithmetic");

        let run = procedure.run(5).await.expect("fan-out should succeed");

        assert_eq!(run.procedure_name, "branching_arithmetic");
        assert_eq!(run.output, (6, 10));
        assert_eq!(run.artifact.artifact.left.output, 6);
        assert_eq!(run.artifact.artifact.right.output, 10);
    }
}
