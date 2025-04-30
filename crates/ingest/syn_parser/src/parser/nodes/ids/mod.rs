//! Public interface for typed node identifiers.
//!
//! This module re-exports the strictly encapsulated ID types defined in the private
//! `internal` module, along with necessary public traits and enums for working
//! with these IDs.

// Declare the private internal module
mod internal;
// Removed: mod utility_macros;
pub(self) use super::*;
// Removed: pub(self) use utility_macros::*;
// ----- Re-exports -----
// We will re-export the specific ID types, marker traits, category enums,
// and the TypedNodeIdGet trait from `internal` here later.

// --- type-bearing ids ---
pub use internal::{
    ConstNodeId, EnumNodeId, FieldNodeId, FunctionNodeId, GenericParamNodeId, ImplNodeId,
    ImportNodeId, MacroNodeId, MethodNodeId, ModuleNodeId, ParamNodeId, ReexportNodeId,
    StaticNodeId, StructNodeId, TraitNodeId, TypeAliasNodeId, UnionNodeId, VariantNodeId,
};
// --- traits ---
// Re-export marker traits (adjust list as needed)
pub use internal::{AssociatedItemIdTrait, PrimaryNodeIdTrait, SecondaryNodeIdTrait, TypedId};
// Re-export the getter trait (make it crate-visible)
pub(crate) use internal::TypedNodeIdGet;
// --- enums ---
// Re-export category enums
pub use internal::{AnyNodeId, AssociatedItemId, PrimaryNodeId};
// --- macro rules ---
