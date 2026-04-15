//! Typed protocol abstractions for bounded review and adjudication procedures.
//!
//! This crate is intended to hold reusable protocol structure that can be
//! consumed by `ploke-eval` and, later, potentially by other crates that need
//! composed mechanized/adjudicative procedures.

pub mod core;
pub mod llm;
pub mod tool_calls;

pub use core::{
    Confidence, Executor, ExecutorKind, Measurement, Protocol, ProtocolArtifact, ProtocolStep,
};
pub use llm::{JsonChatPrompt, JsonLlmConfig, JsonLlmResult, ProtocolLlmError, adjudicate_json};
