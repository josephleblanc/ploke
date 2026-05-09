//! Shared passive record metadata.
//!
//! This metadata identifies persisted record families for emitters and readers.
//! It does not grant write authority or validate runtime transitions.

use serde::Serialize;

/// Logical persisted record family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordFamily {
    SchedulerState,
    SchedulerNode,
    RunnerRequest,
    RunnerResult,
    EvaluationArtifact,
    ProtocolArtifact,
}

/// Wire format used by a persisted record family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordFormat {
    Json,
    JsonLines,
}

/// Passive record shape that can be serialized by an authority-owning crate.
pub trait Record: Serialize {
    const FAMILY: RecordFamily;
    const SCHEMA: &'static str;
    const FORMAT: RecordFormat;
}
