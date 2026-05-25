

The types in the implementation of the formal-procedure-notation framework in
`ploke-protocol` are fairly unwieldy and poorly suited to composition. Instead
we want to go with something more like the following:

## Sketch 1
```rust
pub struct Artifact<S> {
    pub id: ArtifactId,
    pub state: S,
    pub prov: ProvRef,
    pub parents: smallvec::SmallVec<[ArtifactId; 2]>,
    pub recorded: bool,
    pub forwarded: bool,
}

pub trait Step {
    type In;
    type Out;
    type Exec;

    fn run(
        exec: &mut Self::Exec,
        input: Artifact<Self::In>,
        graph: &mut ExecGraph,
    ) -> Result<Artifact<Self::Out>, StepError>;
}

// semantic states
pub struct SequenceReviewContext { /* ... */ }
pub struct SegmentationJudgment { /* ... */ }
pub struct SegmentedToolCallSequence { /* ... */ }

// branch-preserving merge input
pub struct IntentSegmentationBranches {
    pub preserved_context: Artifact<SequenceReviewContext>,
    pub segmentation: Artifact<SegmentationJudgment>,
}

pub struct ContextualizeSequence;
impl Step for ContextualizeSequence {
    type In = ToolCallSequence;
    type Out = SequenceReviewContext;
    type Exec = MechanizedExecutor;
    // ...
}

pub struct SegmentByIntent;
impl Step for SegmentByIntent {
    type In = SequenceReviewContext;
    type Out = SegmentationJudgment;
    type Exec = JsonAdjudicator;
    // ...
}

pub struct NormalizeSegments;
impl Step for NormalizeSegments {
    type In = IntentSegmentationBranches;
    type Out = SegmentedToolCallSequence;
    type Exec = MechanizedExecutor;
    // ...
}
```

## Sketch 2
```rust
pub trait Proc {
    type In;
    type Out;
    type Error;

    fn run(self, input: Self::In) -> Result<Self::Out, Self::Error>;
}

pub struct Then<A, B>(pub A, pub B);

impl<A, B> Proc for Then<A, B>
where
    A: Proc,
    B: Proc<In = A::Out, Error = A::Error>,
{
    type In = A::In;
    type Out = B::Out;
    type Error = A::Error;

    fn run(self, input: Self::In) -> Result<Self::Out, Self::Error> {
        let mid = self.0.run(input)?;
        self.1.run(mid)
    }
}
```

## Sketch 3
```rust
pub struct Segmenter<I>
where
    I: Iterator<Item = ToolCallSummary>,
{
    input: std::iter::Peekable<I>,
    current: ClusterBuilder,
}

impl<I> Iterator for Segmenter<I>
where
    I: Iterator<Item = ToolCallSummary>,
{
    type Item = SegmentCandidate;

    fn next(&mut self) -> Option<Self::Item> {
        // inspect / accumulate / cut a segment / emit candidate
        // hidden runtime state machine
        todo!()
    }
}
```

## Sketch 4
```rust
pub trait Inquiry {
    type Terminal;
    type Error;

    fn admissible(&self) -> ActionSet;
    fn step(self, action: Action) -> Result<InquiryStep<Self, Self::Terminal>, Self::Error>
    where
        Self: Sized;
}

pub enum InquiryStep<Q, T> {
    Continue(Q),
    Done(T),
}
```

## Old/bad example
The current (as of 2026-04-18) implementation of one multi-step procedure looks
like the following, which we would rather avoid:
```rust
pub type IntentSegmentationArtifact = crate::core::ProcedureArtifact<
    crate::core::SequenceArtifact<
        crate::core::StepArtifact<
            ToolCallSequence,
            SequenceReviewContext,
            crate::step::MechanizedProvenance,
        >,
        crate::core::MergeArtifact<
            crate::core::FanOutArtifact<
                SequenceReviewContext,
                crate::core::StepArtifact<
                    SequenceReviewContext,
                    SequenceReviewContext,
                    crate::step::MechanizedProvenance,
                >,
                crate::core::StepArtifact<
                    SequenceReviewContext,
                    SegmentationJudgment,
                    crate::llm::JsonLlmProvenance,
                >,
            >,
            crate::core::StepArtifact<
                crate::core::ForkState<
                    SequenceReviewContext,
                    SequenceReviewContext,
                    SegmentationJudgment,
                >,
                SegmentedToolCallSequence,
                crate::step::MechanizedProvenance,
            >,
        >,
    >,
>;

```
