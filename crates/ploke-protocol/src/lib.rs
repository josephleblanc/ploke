//! Typed protocol and procedure abstractions for mixed mechanized and
//! adjudicated eval workflows.
//!
//! The crate is organized around:
//! - typed step specifications
//! - executor-specific provenance
//! - composable procedures with explicit intermediate outputs
//! - persisted artifacts for each execution boundary

pub mod core;
pub mod llm;
pub mod procedure;
pub mod step;
pub mod tool_calls;

pub use core::{
    Confidence, EvidencePolicy, ExecutorKind, FanOutArtifact, Measurement, ProcedureArtifact,
    ProcedureRun, SequenceArtifact, StepArtifact,
};
pub use llm::{
    JsonAdjudicationSpec, JsonAdjudicator, JsonChatPrompt, JsonLlmConfig, JsonLlmProvenance,
    JsonLlmResult, ProtocolLlmError, adjudicate_json,
};
pub use procedure::{
    FanOut, FanOutError, NamedProcedure, Procedure, ProcedureExt, Sequence, SequenceError,
};
pub use step::{
    MechanizedExecutor, MechanizedProvenance, MechanizedSpec, Step, StepExecution, StepExecutor,
    StepSpec,
};
