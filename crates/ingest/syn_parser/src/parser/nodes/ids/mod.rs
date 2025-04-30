//! Public interface for typed node identifiers.
//!
//! This module re-exports the strictly encapsulated ID types defined in the private
//! `internal` module, along with necessary public traits and enums for working
//! with these IDs.

// Declare the private internal module
mod internal;

// --- Re-exports ---
// We will re-export the specific ID types, marker traits, category enums,
// and the TypedNodeIdGet trait from `internal` here later.
// pub use internal::{StructNodeId, FunctionNodeId, /* ... */};
// pub use internal::{TypedId, PrimaryNodeIdTrait, SecondaryNodeIdTrait, /* ... */};
// pub use internal::{AnyNodeId, PrimaryNodeId, AssociatedItemId};
// pub use internal::TypedNodeIdGet;
