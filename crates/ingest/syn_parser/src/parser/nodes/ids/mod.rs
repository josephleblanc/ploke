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
pub use internal::{
    ConstNodeId, EnumNodeId, FieldNodeId, FunctionNodeId, GenericParamNodeId, ImplNodeId,
    ImportNodeId, MacroNodeId, MethodNodeId, ModuleNodeId, ParamNodeId, ReexportNodeId,
    StaticNodeId, StructNodeId, TraitNodeId, TypeAliasNodeId, UnionNodeId, VariantNodeId,
};
// --- traits ---
// AI: Add re-exports for traits later
// pub use internal::{PrimaryNodeIdTrait, SecondaryNodeIdTrait /* ... */, TypedId};
// --- enums ---
// AI: Add re-exports for enums later
pub use internal::TypedNodeIdGet;
pub use internal::{AnyNodeId, AssociatedItemId, PrimaryNodeId};
