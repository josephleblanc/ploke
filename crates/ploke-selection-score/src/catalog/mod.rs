//! Mechanism metadata catalog preserving the formal-note boundaries.

pub mod registry;
pub mod source;
pub mod spec;

pub use source::{LineSpan, SourceRef};
pub use spec::{Exactness, MechanismKind, MechanismSpec};
