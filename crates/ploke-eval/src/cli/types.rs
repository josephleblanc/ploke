//! Shared CLI/domain value types without pulling handlers or dispatch.
//!
//! Domain code (`prototype1_state`, `run/core`) should prefer importing from this
//! module rather than the CLI barrel to avoid circular dependencies.

pub use super::args::{
    InspectOutputFormat, Prototype1CandidateGenerator, Prototype1ChildScheduleMode,
    Prototype1EditSurface, Prototype1LoopStopAfter, Prototype1StateStopAfter,
    Prototype1SuccessorSelection, Prototype1TraversalMetrics,
};
