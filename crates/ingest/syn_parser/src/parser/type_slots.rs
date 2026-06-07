//! ID types for syntactic type-use slots on parser nodes.
//!
//! These slots carry already-proven structural type ID families, so downstream
//! relation construction does not need to rediscover the admissible source set
//! from a raw ID. Ordinary `syn::Type` slots use [`OrdinaryTypeUseId`]; trait-position
//! slots use [`TraitTypeUseId`].

pub type OrdinaryTypeUseId = crate::parser::nodes::OrdinaryTypeUseId;
pub type TraitTypeUseId = crate::parser::nodes::TraitTypeSourceId;

#[inline]
pub(in crate::parser) fn ordinary_type_use_base_id(id: OrdinaryTypeUseId) -> ploke_core::TypeId {
    use crate::parser::nodes::StructuralTypeId as _;
    id.base_id()
}
