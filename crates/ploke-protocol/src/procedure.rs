use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

use crate::core::{
    FanOutArtifact, ForkState, MergeArtifact, ProcedureArtifact, ProcedureRun, ProcedureState,
    SequenceArtifact, StateDisposition, StateEnvelope,
};
use crate::step::{Step, StepExecutor, StepSpec};

#[async_trait]
pub trait Procedure: Send + Sync {
    type Subject: ProcedureState;
    type Output: ProcedureState;
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
    Spec::InputState: Send,
    Spec::OutputState: Send,
    Exec: StepExecutor<Spec> + Send + Sync,
    Exec::Provenance: Send,
    Exec::Error: Send,
{
    type Subject = Spec::InputState;
    type Output = Spec::OutputState;
    type Artifact =
        crate::core::StepArtifact<Spec::InputState, Spec::OutputState, Exec::Provenance>;
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
    pub left_branch: &'static str,
    pub right_branch: &'static str,
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
    Subject: ProcedureState,
    Left: Procedure<Subject = Subject>,
    Left::Artifact: Send,
    Left::Error: Send,
    Right: Procedure<Subject = Subject>,
    Right::Artifact: Send,
    Right::Error: Send,
{
    type Subject = Subject;
    type Output = ForkState<Subject, Left::Output, Right::Output>;
    type Artifact = FanOutArtifact<Subject, Left::Artifact, Right::Artifact>;
    type Error = FanOutError<Left::Error, Right::Error>;

    fn name(&self) -> &'static str {
        "fan_out"
    }

    async fn run(
        &self,
        subject: Self::Subject,
    ) -> Result<ProcedureRun<Self::Output, Self::Artifact>, Self::Error> {
        let captured_input = subject.clone();
        let left = self
            .left
            .run(subject.clone())
            .await
            .map_err(FanOutError::Left)?;
        let right = self
            .right
            .run(subject.clone())
            .await
            .map_err(FanOutError::Right)?;

        Ok(ProcedureRun {
            procedure_name: self.name().to_string(),
            output: ForkState {
                source: subject,
                left: left.output.clone(),
                right: right.output.clone(),
            },
            artifact: FanOutArtifact {
                input: StateEnvelope {
                    state: captured_input,
                    disposition: StateDisposition::ForwardOnly,
                },
                left_branch: self.left_branch.to_string(),
                right_branch: self.right_branch.to_string(),
                left: left.artifact,
                right: right.artifact,
            },
        })
    }
}

#[derive(Debug, Clone)]
pub struct Merge<Branches, Join> {
    pub branches: Branches,
    pub join: Join,
}

#[derive(Debug, Error)]
pub enum MergeError<BranchesError, JoinError> {
    #[error("branch procedure failed: {0}")]
    Branches(BranchesError),
    #[error("merge procedure failed: {0}")]
    Join(JoinError),
}

#[async_trait]
impl<Branches, Join> Procedure for Merge<Branches, Join>
where
    Branches: Procedure,
    Branches::Artifact: Send,
    Branches::Error: Send,
    Join: Procedure<Subject = Branches::Output>,
    Join::Artifact: Send,
    Join::Error: Send,
{
    type Subject = Branches::Subject;
    type Output = Join::Output;
    type Artifact = MergeArtifact<Branches::Artifact, Join::Artifact>;
    type Error = MergeError<Branches::Error, Join::Error>;

    fn name(&self) -> &'static str {
        "merge"
    }

    async fn run(
        &self,
        subject: Self::Subject,
    ) -> Result<ProcedureRun<Self::Output, Self::Artifact>, Self::Error> {
        let branches = self
            .branches
            .run(subject)
            .await
            .map_err(MergeError::Branches)?;
        let merged = self
            .join
            .run(branches.output.clone())
            .await
            .map_err(MergeError::Join)?;

        Ok(ProcedureRun {
            procedure_name: self.name().to_string(),
            output: merged.output.clone(),
            artifact: MergeArtifact {
                branches: branches.artifact,
                merge: merged.artifact,
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
        Self::Subject: ProcedureState,
        Right: Procedure<Subject = Self::Subject>,
    {
        FanOut {
            left: self,
            right,
            left_branch: "left",
            right_branch: "right",
        }
    }

    fn fan_out_named<Right>(
        self,
        left_branch: &'static str,
        right_branch: &'static str,
        right: Right,
    ) -> FanOut<Self, Right>
    where
        Self::Subject: ProcedureState,
        Right: Procedure<Subject = Self::Subject>,
    {
        FanOut {
            left: self,
            right,
            left_branch,
            right_branch,
        }
    }

    fn merge<Join>(self, join: Join) -> Merge<Self, Join>
    where
        Join: Procedure<Subject = Self::Output>,
    {
        Merge {
            branches: self,
            join,
        }
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

    #[derive(Clone, Copy, Debug)]
    struct AddOne;

    impl StepSpec for AddOne {
        type InputState = i32;
        type OutputState = i32;

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

        fn execute_mechanized(
            &self,
            input: Self::InputState,
        ) -> Result<Self::OutputState, Self::Error> {
            Ok(input + 1)
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct Double;

    impl StepSpec for Double {
        type InputState = i32;
        type OutputState = i32;

        fn step_id(&self) -> &'static str {
            "double"
        }

        fn step_name(&self) -> &'static str {
            "double"
        }
    }

    impl MechanizedSpec for Double {
        type Error = &'static str;

        fn execute_mechanized(
            &self,
            input: Self::InputState,
        ) -> Result<Self::OutputState, Self::Error> {
            Ok(input * 2)
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct SumBranches;

    impl StepSpec for SumBranches {
        type InputState = ForkState<i32, i32, i32>;
        type OutputState = i32;

        fn step_id(&self) -> &'static str {
            "sum_branches"
        }

        fn step_name(&self) -> &'static str {
            "sum_branches"
        }
    }

    impl MechanizedSpec for SumBranches {
        type Error = &'static str;

        fn execute_mechanized(
            &self,
            input: Self::InputState,
        ) -> Result<Self::OutputState, Self::Error> {
            Ok(input.left + input.right)
        }
    }

    #[tokio::test]
    async fn sequence_preserves_intermediate_state_artifacts() {
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
    async fn fan_out_keeps_branch_outputs_with_source_state() {
        let procedure = Step::new(AddOne, MechanizedExecutor)
            .fan_out_named(
                "incremented",
                "doubled",
                Step::new(Double, MechanizedExecutor),
            )
            .named("branching_arithmetic");

        let run = procedure.run(5).await.expect("fan-out should succeed");

        assert_eq!(run.procedure_name, "branching_arithmetic");
        assert_eq!(run.output.source, 5);
        assert_eq!(run.output.left, 6);
        assert_eq!(run.output.right, 10);
        assert_eq!(run.artifact.artifact.left_branch, "incremented");
        assert_eq!(run.artifact.artifact.right_branch, "doubled");
    }

    #[tokio::test]
    async fn merge_preserves_branch_artifacts_and_join_result() {
        let procedure = Step::new(AddOne, MechanizedExecutor)
            .fan_out_named(
                "incremented",
                "doubled",
                Step::new(Double, MechanizedExecutor),
            )
            .merge(Step::new(SumBranches, MechanizedExecutor))
            .named("merged_arithmetic");

        let run = procedure.run(5).await.expect("merge should succeed");

        assert_eq!(run.procedure_name, "merged_arithmetic");
        assert_eq!(run.output, 16);
        assert_eq!(run.artifact.artifact.branches.left_branch, "incremented");
        assert_eq!(run.artifact.artifact.branches.right_branch, "doubled");
        assert_eq!(run.artifact.artifact.merge.input.left, 6);
        assert_eq!(run.artifact.artifact.merge.input.right, 10);
        assert_eq!(run.artifact.artifact.merge.output, 16);
    }
}
