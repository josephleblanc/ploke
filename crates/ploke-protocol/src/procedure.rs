use async_trait::async_trait;
use serde::Serialize;
use std::env;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use thiserror::Error;

use crate::core::{
    FanOutArtifact, ForkState, MergeArtifact, ProcedureArtifact, ProcedureRun, ProcedureState,
    SequenceArtifact, StateDisposition, StateEnvelope,
};
use crate::step::{Step, StepExecutor, StepSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcedureDebugEventKind {
    ProcedureStarted,
    ProcedureFinished,
    ProcedureFailed,
    SubrequestStarted,
    SubrequestFinished,
    SubrequestFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProcedureDebugEvent {
    pub event: ProcedureDebugEventKind,
    pub procedure_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_total: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub type ProcedureDebugSink = Arc<dyn Fn(&ProcedureDebugEvent) + Send + Sync + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SubrequestDescriptor {
    pub label: &'static str,
    pub request_index: usize,
    pub request_total: usize,
}

#[derive(Debug, Clone)]
pub struct ObservedSubrequest<Inner> {
    pub inner: Inner,
    pub descriptor: SubrequestDescriptor,
}

impl<Inner> ObservedSubrequest<Inner> {
    pub fn new(inner: Inner, descriptor: SubrequestDescriptor) -> Self {
        Self { inner, descriptor }
    }
}

static PROCEDURE_DEBUG_SINK: OnceLock<Mutex<Option<ProcedureDebugSink>>> = OnceLock::new();
static PROCEDURE_DEBUG_STDERR: OnceLock<bool> = OnceLock::new();

pub fn set_procedure_debug_sink(sink: Option<ProcedureDebugSink>) {
    let lock = PROCEDURE_DEBUG_SINK.get_or_init(|| Mutex::new(None));
    *lock.lock().expect("procedure debug sink mutex poisoned") = sink;
}

fn procedure_debug_stderr_enabled() -> bool {
    *PROCEDURE_DEBUG_STDERR.get_or_init(|| {
        env::var("PLOKE_PROTOCOL_DEBUG").is_ok_and(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty() && normalized != "0" && normalized != "false"
        })
    })
}

fn emit_procedure_debug_event(event: ProcedureDebugEvent) {
    if let Some(sink) = PROCEDURE_DEBUG_SINK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("procedure debug sink mutex poisoned")
        .clone()
    {
        sink(&event);
    }

    if procedure_debug_stderr_enabled()
        && let Ok(line) = serde_json::to_string(&event)
    {
        eprintln!("{line}");
    }
}

struct ProcedureDebugRun {
    procedure_name: &'static str,
    start: Instant,
}

impl ProcedureDebugRun {
    fn start(procedure_name: &'static str) -> Self {
        emit_procedure_debug_event(ProcedureDebugEvent {
            event: ProcedureDebugEventKind::ProcedureStarted,
            procedure_name: procedure_name.to_string(),
            request_label: None,
            request_index: None,
            request_total: None,
            elapsed_ms: None,
            detail: None,
        });

        Self {
            procedure_name,
            start: Instant::now(),
        }
    }

    fn finish(self) {
        emit_procedure_debug_event(ProcedureDebugEvent {
            event: ProcedureDebugEventKind::ProcedureFinished,
            procedure_name: self.procedure_name.to_string(),
            request_label: None,
            request_index: None,
            request_total: None,
            elapsed_ms: Some(self.start.elapsed().as_millis()),
            detail: None,
        });
    }

    fn fail<Error>(self, error: &Error)
    where
        Error: std::fmt::Display,
    {
        emit_procedure_debug_event(ProcedureDebugEvent {
            event: ProcedureDebugEventKind::ProcedureFailed,
            procedure_name: self.procedure_name.to_string(),
            request_label: None,
            request_index: None,
            request_total: None,
            elapsed_ms: Some(self.start.elapsed().as_millis()),
            detail: Some(error.to_string()),
        });
    }
}

struct SubrequestDebugRun {
    procedure_name: &'static str,
    descriptor: SubrequestDescriptor,
    start: Instant,
}

impl SubrequestDebugRun {
    fn start(procedure_name: &'static str, descriptor: SubrequestDescriptor) -> Self {
        emit_procedure_debug_event(ProcedureDebugEvent {
            event: ProcedureDebugEventKind::SubrequestStarted,
            procedure_name: procedure_name.to_string(),
            request_label: Some(descriptor.label.to_string()),
            request_index: Some(descriptor.request_index),
            request_total: Some(descriptor.request_total),
            elapsed_ms: None,
            detail: None,
        });

        Self {
            procedure_name,
            descriptor,
            start: Instant::now(),
        }
    }

    fn finish(self) {
        emit_procedure_debug_event(ProcedureDebugEvent {
            event: ProcedureDebugEventKind::SubrequestFinished,
            procedure_name: self.procedure_name.to_string(),
            request_label: Some(self.descriptor.label.to_string()),
            request_index: Some(self.descriptor.request_index),
            request_total: Some(self.descriptor.request_total),
            elapsed_ms: Some(self.start.elapsed().as_millis()),
            detail: None,
        });
    }

    fn fail<Error>(self, error: &Error)
    where
        Error: std::fmt::Display,
    {
        emit_procedure_debug_event(ProcedureDebugEvent {
            event: ProcedureDebugEventKind::SubrequestFailed,
            procedure_name: self.procedure_name.to_string(),
            request_label: Some(self.descriptor.label.to_string()),
            request_index: Some(self.descriptor.request_index),
            request_total: Some(self.descriptor.request_total),
            elapsed_ms: Some(self.start.elapsed().as_millis()),
            detail: Some(error.to_string()),
        });
    }
}

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

#[async_trait]
impl<Inner> Procedure for ObservedSubrequest<Inner>
where
    Inner: Procedure,
    Inner::Artifact: Send,
    Inner::Error: Send + std::fmt::Display,
{
    type Subject = Inner::Subject;
    type Output = Inner::Output;
    type Artifact = Inner::Artifact;
    type Error = Inner::Error;

    fn name(&self) -> &'static str {
        self.inner.name()
    }

    async fn run(
        &self,
        subject: Self::Subject,
    ) -> Result<ProcedureRun<Self::Output, Self::Artifact>, Self::Error> {
        let debug = SubrequestDebugRun::start(self.name(), self.descriptor);
        match self.inner.run(subject).await {
            Ok(run) => {
                debug.finish();
                Ok(run)
            }
            Err(error) => {
                debug.fail(&error);
                Err(error)
            }
        }
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
    First::Error: Send + std::fmt::Display,
    Second: Procedure<Subject = First::Output>,
    Second::Artifact: Send,
    Second::Error: Send + std::fmt::Display,
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
        let debug = ProcedureDebugRun::start(self.name());
        let first = match self.first.run(subject).await {
            Ok(first) => first,
            Err(error) => {
                debug.fail(&error);
                return Err(SequenceError::First(error));
            }
        };
        let second = match self.second.run(first.output.clone()).await {
            Ok(second) => second,
            Err(error) => {
                debug.fail(&error);
                return Err(SequenceError::Second(error));
            }
        };

        let run = ProcedureRun {
            procedure_name: self.name().to_string(),
            output: second.output.clone(),
            artifact: SequenceArtifact {
                first: first.artifact,
                second: second.artifact,
            },
        };
        debug.finish();
        Ok(run)
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
    Left::Error: Send + std::fmt::Display,
    Right: Procedure<Subject = Subject>,
    Right::Artifact: Send,
    Right::Error: Send + std::fmt::Display,
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
        let debug = ProcedureDebugRun::start(self.name());
        let captured_input = subject.clone();
        let left = match self.left.run(subject.clone()).await {
            Ok(left) => left,
            Err(error) => {
                debug.fail(&error);
                return Err(FanOutError::Left(error));
            }
        };
        let right = match self.right.run(subject.clone()).await {
            Ok(right) => right,
            Err(error) => {
                debug.fail(&error);
                return Err(FanOutError::Right(error));
            }
        };

        let run = ProcedureRun {
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
        };
        debug.finish();
        Ok(run)
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
    Branches::Error: Send + std::fmt::Display,
    Join: Procedure<Subject = Branches::Output>,
    Join::Artifact: Send,
    Join::Error: Send + std::fmt::Display,
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
        let debug = ProcedureDebugRun::start(self.name());
        let branches = match self.branches.run(subject).await {
            Ok(branches) => branches,
            Err(error) => {
                debug.fail(&error);
                return Err(MergeError::Branches(error));
            }
        };
        let merged = match self.join.run(branches.output.clone()).await {
            Ok(merged) => merged,
            Err(error) => {
                debug.fail(&error);
                return Err(MergeError::Join(error));
            }
        };

        let run = ProcedureRun {
            procedure_name: self.name().to_string(),
            output: merged.output.clone(),
            artifact: MergeArtifact {
                branches: branches.artifact,
                merge: merged.artifact,
            },
        };
        debug.finish();
        Ok(run)
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
    Inner::Error: Send + std::fmt::Display,
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
        let debug = ProcedureDebugRun::start(self.name());
        let inner = match self.inner.run(subject).await {
            Ok(inner) => inner,
            Err(error) => {
                debug.fail(&error);
                return Err(error);
            }
        };
        let run = ProcedureRun {
            procedure_name: self.name().to_string(),
            output: inner.output,
            artifact: ProcedureArtifact {
                procedure_name: self.name().to_string(),
                artifact: inner.artifact,
            },
        };
        debug.finish();
        Ok(run)
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
    use std::sync::{Arc, Mutex};

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

    #[tokio::test]
    async fn observed_subrequest_emits_numbered_debug_events() {
        let events = Arc::new(Mutex::new(Vec::<ProcedureDebugEvent>::new()));
        let sink_events = Arc::clone(&events);
        set_procedure_debug_sink(Some(Arc::new(move |event| {
            sink_events
                .lock()
                .expect("test events mutex should not be poisoned")
                .push(event.clone());
        })));

        let procedure = ObservedSubrequest::new(
            Step::new(AddOne, MechanizedExecutor),
            SubrequestDescriptor {
                label: "observed_subrequest_test",
                request_index: 1,
                request_total: 3,
            },
        )
        .named("observed_debug_test");

        let run = procedure
            .run(4)
            .await
            .expect("observed subrequest should succeed");
        set_procedure_debug_sink(None);

        assert_eq!(run.output, 5);

        let recorded = events
            .lock()
            .expect("test events mutex should not be poisoned")
            .clone();

        assert!(recorded.iter().any(|event| {
            event.event == ProcedureDebugEventKind::ProcedureStarted
                && event.procedure_name == "observed_debug_test"
        }));
        assert!(recorded.iter().any(|event| {
            event.event == ProcedureDebugEventKind::ProcedureFinished
                && event.procedure_name == "observed_debug_test"
        }));
        assert!(recorded.iter().any(|event| {
            event.event == ProcedureDebugEventKind::SubrequestStarted
                && event.procedure_name == "add_one"
                && event.request_label.as_deref() == Some("observed_subrequest_test")
                && event.request_index == Some(1)
                && event.request_total == Some(3)
        }));
        assert!(recorded.iter().any(|event| {
            event.event == ProcedureDebugEventKind::SubrequestFinished
                && event.procedure_name == "add_one"
                && event.request_label.as_deref() == Some("observed_subrequest_test")
                && event.request_index == Some(1)
                && event.request_total == Some(3)
        }));
    }
}
