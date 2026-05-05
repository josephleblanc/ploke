//! Feature-gated ID types for syntactic type-use slots on parser nodes.
//!
//! In the legacy graph these slots carry bare [`ploke_core::TypeId`] values.
//! Under `typed_type_graph` they carry already-proven structural type ID
//! families, so downstream relation construction does not need to rediscover
//! the admissible source set from a raw ID. Ordinary `syn::Type` slots use
//! [`OrdinaryTypeUseId`]; trait-position slots use [`TraitTypeUseId`].

#[cfg(feature = "typed_type_graph")]
pub type OrdinaryTypeUseId = crate::parser::nodes::OrdinaryTypeUseId;
#[cfg(not(feature = "typed_type_graph"))]
pub type OrdinaryTypeUseId = ploke_core::TypeId;

#[cfg(feature = "typed_type_graph")]
pub type TraitTypeUseId = crate::parser::nodes::TraitTypeSourceId;
#[cfg(not(feature = "typed_type_graph"))]
pub type TraitTypeUseId = ploke_core::TypeId;

#[inline]
#[cfg(feature = "typed_type_graph")]
pub(in crate::parser) fn ordinary_type_use_base_id(id: OrdinaryTypeUseId) -> ploke_core::TypeId {
    use crate::parser::nodes::StructuralTypeId as _;
    id.base_id()
}

#[inline]
#[cfg(not(feature = "typed_type_graph"))]
pub(in crate::parser) fn ordinary_type_use_base_id(id: OrdinaryTypeUseId) -> ploke_core::TypeId {
    id
}
