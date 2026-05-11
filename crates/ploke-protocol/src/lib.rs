//! Typed protocol and procedure abstractions for mixed mechanized and
//! adjudicated eval workflows.
//!
//! The crate is organized around:
//! - typed step specifications
//! - executor-specific provenance
//! - composable procedures with explicit intermediate outputs
//! - persisted artifacts for each execution boundary

pub mod core;
#[cfg(feature = "llm")]
pub mod llm;
pub mod procedure;
pub mod step;
pub mod tool_calls;

pub use core::{
    Confidence, EvidencePolicy, ExecutorKind, FanOutArtifact, ForkState, Measurement,
    MergeArtifact, ProcedureArtifact, ProcedureRun, ProcedureState, SequenceArtifact,
    StateDisposition, StateEnvelope, StepArtifact,
};
#[cfg(feature = "llm")]
pub use llm::{
    JsonAdjudicationSpec, JsonAdjudicator, JsonChatPrompt, JsonLlmConfig, JsonLlmProvenance,
    JsonLlmResult, ProtocolLlmError, adjudicate_json,
};
pub use procedure::{
    FanOut, FanOutError, Merge, MergeError, NamedProcedure, Procedure, ProcedureExt, Sequence,
    SequenceError,
};
pub use step::{
    MechanizedExecutor, MechanizedProvenance, MechanizedSpec, Step, StepExecution, StepExecutor,
    StepSpec,
};
pub use tool_calls::review::{
    Concern, LocalAnalysisAssessment, LocalAnalysisContext, LocalAnalysisPacket,
    LocalAnalysisSignals, LocalAnalysisTargetKind, Metric as ToolCallReviewMetric, OverallVerdict,
    RecoverabilityAssessment, RecoverabilityVerdict, RedundancyAssessment, RedundancyVerdict,
    SegmentMetric as ToolCallSegmentReviewMetric, SegmentReviewSubject, UsefulnessAssessment,
    UsefulnessVerdict,
};
#[cfg(feature = "llm")]
pub use tool_calls::review::{
    ToolCallReview, ToolCallReviewArtifact, ToolCallReviewError, ToolCallSegmentReview,
    ToolCallSegmentReviewArtifact, ToolCallSegmentReviewError,
};
pub use tool_calls::segment::{
    IntentLabel, IntentSegment, IntentSegmentProposal, Metric as IntentSegmentationMetric,
    SegmentStatus, SegmentationCoverage, SegmentationJudgment, SegmentedToolCallSequence,
    SequenceReviewContext, SequenceSignals, UncoveredCallSpan,
};
#[cfg(feature = "llm")]
pub use tool_calls::segment::{
    IntentSegmentationArtifact, IntentSegmentationError, ToolCallIntentSegmentation,
};
pub use tool_calls::trace::{NeighborhoodCall, ToolCallNeighborhood, ToolCallSequence, ToolKind};
