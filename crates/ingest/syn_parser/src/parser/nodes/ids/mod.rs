//! Public interface for typed node identifiers.
//!
//! This module re-exports the strictly encapsulated ID types defined in the private
//! `internal` module, along with necessary public traits and enums for working
//! with these IDs.

// Declare the private internal module
mod internal;
pub(self) use super::*;
// ----- Re-exports -----
// We will re-export the specific ID types, marker traits, category enums,
// and the TypedNodeIdGet trait from `internal` here later.
// --- type-bearing ids ---
// AI: Let's start by filling these type-bearing nodes out, since they are already done and I've
// moved them into the `internal` module already AI!
pub use internal::{FunctionNodeId /* ... */, StructNodeId};
// --- traits ---
pub use internal::{PrimaryNodeIdTrait, SecondaryNodeIdTrait /* ... */, TypedId};
// --- enums ---
pub use internal::TypedNodeIdGet;
pub use internal::{AnyNodeId, AssociatedItemId, PrimaryNodeId};
