//!
//! This module re-exports the strictly encapsulated ID types defined in the private
//! `internal` module, along with necessary public traits and enums for working
//! with these IDs.

// Declare the private internal module
mod internal;

// Removed: mod utility_macros;
use super::*;
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
pub use internal::{AssociatedItemNodeIdTrait, PrimaryNodeIdTrait, SecondaryNodeIdTrait, TypedId};
// Node trait for any node with an ID
pub use internal::HasAnyNodeId;
// Re-exported convenience trait (same functionality as Into<AnyNodeId>)
// Helps be more explicit about conversions to `AnyNodeId`
pub use internal::AsAnyNodeId;
// Helps with displaying raw Uuid (useful in ploke-transform)
// Included here to prevent exposing underlying type for invalid comparisons.
pub use internal::ToUuidString;

// Re-export the getter trait (make it crate-visible)
// pub(crate) use internal::TypedNodeIdGet;
// --- enums ---
// Re-export category enums
pub use internal::{AnyNodeId, AssociatedItemNodeId, PrimaryNodeId, SecondaryNodeId};
// --- macro rules ---
// --- error types ---
pub use internal::{
    AnyNodeIdConversionError, TryFromAssociatedItemError, TryFromPrimaryError,
    TryFromSecondaryError,
};

// --- semi-private ---
// Would like to make these more private someday
pub(in crate::parser) use internal::{GenerateTypeId, GeneratesAnyNodeId};

// Tests
pub use internal::test_ids;
