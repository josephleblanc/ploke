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
pub(in crate::parser) use internal::StructuralTypeId;
pub use internal::{
    AnyTypeId, OrdinaryTypeDefId, OrdinaryTypeSourceId, OrdinaryTypeTargetId, OrdinaryTypeUseId,
    TraitTypeSourceId, TraitTypeTargetId, TryFromAnyTypeError, TryFromOrdinaryTypeDefError,
    TryFromOrdinaryTypeSourceError, TryFromOrdinaryTypeTargetError, TryFromOrdinaryTypeUseError,
    TryFromTraitTypeSourceError, TryFromTraitTypeTargetError, TryFromTypeSourceError, TypeSourceId,
};
pub use internal::{
    ArrayTypeId, FunctionTypeId, ImplTraitTypeId, InferredTypeId, MacroTypeId, NamedTypeId,
    NeverTypeId, ParenTypeId, RawPointerTypeId, ReferenceTypeId, SliceTypeId, TraitBoundTypeId,
    TraitObjectTypeId, TupleTypeId, TypeIdRefinementError, UnknownTypeId,
};
// --- node ids ---
pub use internal::{
    ConstGenericParamNodeId, ConstNodeId, EnumNodeId, FieldNodeId, FunctionNodeId,
    GenericParamNodeId, ImplNodeId, ImportNodeId, LifetimeGenericParamNodeId, MacroNodeId,
    MethodNodeId, ModuleNodeId, ParamNodeId, ReexportNodeId, StaticNodeId, StructNodeId,
    TraitNodeId, TypeAliasNodeId, TypeGenericParamNodeId, UnionNodeId, UnresolvedNodeId,
    VariantNodeId,
};
// --- call-site ids ---
pub use internal::{DynamicCallSiteId, MacroCallSiteId, MethodCallSiteId, PathCallSiteId};
// --- traits ---
// Re-export marker traits (adjust list as needed)
pub use internal::{AssociatedItemNodeIdTrait, PrimaryNodeIdTrait, SecondaryNodeIdTrait, TypedId};
// Node trait for any node with an ID
// pub use internal::HasAnyNodeId;
// Re-exported convenience trait (same functionality as Into<AnyNodeId>)
// Helps be more explicit about conversions to `AnyNodeId`
pub use internal::AsAnyNodeId;
// Helps with displaying raw Uuid (useful in ploke-transform)
// Included here to prevent exposing underlying type for invalid comparisons.
pub use internal::{ToCozoUuid, ToUuidString};

// Re-export the getter trait (make it crate-visible)
// pub(crate) use internal::TypedNodeIdGet;
// --- enums ---
// Re-export category enums
pub use internal::{AnyCallSiteId, CallSiteKind};
pub use internal::{
    AnyGenericParamId, AnyNodeId, AssociatedItemNodeId, AssociatedItemOwnerId, CallBodyOwnerId,
    GenericParamOwnerId, PrimaryNodeId, SecondaryNodeId, SelfScopeOwnerId, TypeUseOwnerId,
};
// --- macro rules ---
// --- error types ---
pub use internal::{
    AnyNodeIdConversionError, GenericParamIdRefinementError, TryFromAnyGenericParamError,
    TryFromAssociatedItemError, TryFromAssociatedItemOwnerError, TryFromGenericParamOwnerError,
    TryFromPrimaryError, TryFromSecondaryError, TryFromSelfScopeOwnerError,
    TryFromTypeUseOwnerError,
};

// --- semi-private ---
// Would like to make these more private someday
#[allow(
    unused_imports,
    reason = "call-site extraction will use this parser-internal constructor"
)]
pub(in crate::parser) use internal::generate_method_call_site_id;
pub(in crate::parser) use internal::{GenerateTypeId, GeneratesAnyNodeId};

// Tests
pub use internal::test_ids;
