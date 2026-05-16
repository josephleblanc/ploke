//! Shared passive record metadata.
//!
//! This metadata identifies persisted record families for emitters and readers.
//! It does not grant write authority or validate runtime transitions.

use serde::Serialize;

/// Logical persisted record family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordFamily {
    ChildPlan,
    SchedulerState,
    SchedulerNode,
    RunnerRequest,
    RunnerResult,
    EvaluationArtifact,
    ProtocolArtifact,
    RunProfile,
    RunProfileCommitment,
    AgentTurnTrace,
    AgentTurnSummary,
    RunRecord,
}

/// Wire format used by a persisted record family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordFormat {
    Json,
    JsonLines,
    Toml,
}

/// Passive record shape that can be serialized by an authority-owning crate.
pub trait Record: Serialize {
    const FAMILY: RecordFamily;
    const SCHEMA: &'static str;
    const FORMAT: RecordFormat;
}

/// Explicit live-to-record projection boundary.
///
/// This trait is intentionally structural only. It does not perform I/O or own
/// persistence side effects; authority-owning crates remain responsible for
/// deciding when and where a projected record is written.
pub trait ToRecord {
    type Record;

    fn to_record(&self) -> Self::Record;
}
