//! Mechanism metadata catalog preserving stable formal-note lookup keys.

pub mod registry;
pub mod source;
pub mod spec;

pub use source::SourceRef;
pub use spec::{Exactness, MechanismKind, MechanismSpec};
