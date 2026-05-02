//! Structural admissibility families for type resolution.
//!
//! These families play the same role for type-resolution endpoints that
//! `PrimaryNodeId` plays for module containment: they define which already-
//! typed IDs are admissible at a given boundary, so later relations can be
//! sharp about source and target shape instead of relying on conventions.

use super::{
    AnyNodeId, AnyTypedId, CategoricalTypedId, EnumNodeId, GenericParamNodeId, NamedTypeId,
    StructNodeId, TraitBoundTypeId, TraitNodeId, TypeAliasNodeId, UnionNodeId,
};
use crate::parser::nodes::ids::StructuralTypeId;
use ploke_core::{ItemKind, TypeId};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TryFromTypeSourceError;

impl Display for TryFromTypeSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypeSourceId variant mismatch")
    }
}

impl Error for TryFromTypeSourceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TryFromOrdinaryTypeTargetError;

impl Display for TryFromOrdinaryTypeTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OrdinaryTypeTargetId variant mismatch")
    }
}

impl Error for TryFromOrdinaryTypeTargetError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TryFromTraitTypeTargetError;

impl Display for TryFromTraitTypeTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TraitTypeTargetId variant mismatch")
    }
}

impl Error for TryFromTraitTypeTargetError {}

/// Structural type-source classes that can directly participate in resolution.
///
/// This is intentionally narrower than "all type IDs". Composite type nodes
/// like tuples, references, or arrays remain containers in the type graph and
/// are traversed through their nested children instead of resolving directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum TypeSourceId {
    Named(NamedTypeId),
    TraitBound(TraitBoundTypeId),
}

impl TypeSourceId {
    #[inline]
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Named(_) => "NamedTypeId",
            Self::TraitBound(_) => "TraitBoundTypeId",
        }
    }
}

impl StructuralTypeId for TypeSourceId {
    #[inline]
    fn base_id(self) -> TypeId {
        match self {
            Self::Named(id) => id.base_id(),
            Self::TraitBound(id) => id.base_id(),
        }
    }
}

impl AnyTypedId for TypeSourceId {}
impl CategoricalTypedId for TypeSourceId {}

impl From<NamedTypeId> for TypeSourceId {
    #[inline]
    fn from(id: NamedTypeId) -> Self {
        Self::Named(id)
    }
}

impl From<TraitBoundTypeId> for TypeSourceId {
    #[inline]
    fn from(id: TraitBoundTypeId) -> Self {
        Self::TraitBound(id)
    }
}

impl TryFrom<TypeSourceId> for NamedTypeId {
    type Error = TryFromTypeSourceError;

    #[inline]
    fn try_from(value: TypeSourceId) -> Result<Self, Self::Error> {
        match value {
            TypeSourceId::Named(id) => Ok(id),
            _ => Err(TryFromTypeSourceError),
        }
    }
}

impl TryFrom<TypeSourceId> for TraitBoundTypeId {
    type Error = TryFromTypeSourceError;

    #[inline]
    fn try_from(value: TypeSourceId) -> Result<Self, Self::Error> {
        match value {
            TypeSourceId::TraitBound(id) => Ok(id),
            _ => Err(TryFromTypeSourceError),
        }
    }
}

impl From<TypeSourceId> for TypeId {
    #[inline]
    fn from(id: TypeSourceId) -> Self {
        id.base_id()
    }
}

impl Display for TypeSourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(id) => write!(f, "Named({})", id),
            Self::TraitBound(id) => write!(f, "TraitBound({})", id),
        }
    }
}

/// Node-ID family for ordinary type-position targets.
///
/// This covers the code-graph nodes that are structurally valid targets for
/// plain named type use-sites. Semantic-only targets like builtins or `Self`
/// are intentionally left out of this family because they are not node IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum OrdinaryTypeTargetId {
    Struct(StructNodeId),
    Enum(EnumNodeId),
    Union(UnionNodeId),
    TypeAlias(TypeAliasNodeId),
    GenericParam(GenericParamNodeId),
}

impl OrdinaryTypeTargetId {
    #[inline]
    pub fn base_id(&self) -> ploke_core::NodeId {
        match *self {
            Self::Struct(id) => id.base_id(),
            Self::Enum(id) => id.base_id(),
            Self::Union(id) => id.base_id(),
            Self::TypeAlias(id) => id.base_id(),
            Self::GenericParam(id) => id.base_id(),
        }
    }

    #[inline]
    pub fn kind(&self) -> ItemKind {
        match *self {
            Self::Struct(_) => ItemKind::Struct,
            Self::Enum(_) => ItemKind::Enum,
            Self::Union(_) => ItemKind::Union,
            Self::TypeAlias(_) => ItemKind::TypeAlias,
            Self::GenericParam(_) => ItemKind::GenericParam,
        }
    }
}

impl AnyTypedId for OrdinaryTypeTargetId {}
impl CategoricalTypedId for OrdinaryTypeTargetId {}

impl From<OrdinaryTypeTargetId> for AnyNodeId {
    #[inline]
    fn from(id: OrdinaryTypeTargetId) -> Self {
        match id {
            OrdinaryTypeTargetId::Struct(id) => id.into(),
            OrdinaryTypeTargetId::Enum(id) => id.into(),
            OrdinaryTypeTargetId::Union(id) => id.into(),
            OrdinaryTypeTargetId::TypeAlias(id) => id.into(),
            OrdinaryTypeTargetId::GenericParam(id) => id.into(),
        }
    }
}

macro_rules! impl_ordinary_type_target_variant {
    ($SpecificId:ty, $Variant:ident) => {
        impl From<$SpecificId> for OrdinaryTypeTargetId {
            #[inline]
            fn from(id: $SpecificId) -> Self {
                Self::$Variant(id)
            }
        }

        impl TryFrom<OrdinaryTypeTargetId> for $SpecificId {
            type Error = TryFromOrdinaryTypeTargetError;

            #[inline]
            fn try_from(value: OrdinaryTypeTargetId) -> Result<Self, Self::Error> {
                match value {
                    OrdinaryTypeTargetId::$Variant(id) => Ok(id),
                    _ => Err(TryFromOrdinaryTypeTargetError),
                }
            }
        }
    };
}

impl_ordinary_type_target_variant!(StructNodeId, Struct);
impl_ordinary_type_target_variant!(EnumNodeId, Enum);
impl_ordinary_type_target_variant!(UnionNodeId, Union);
impl_ordinary_type_target_variant!(TypeAliasNodeId, TypeAlias);
impl_ordinary_type_target_variant!(GenericParamNodeId, GenericParam);

impl Display for OrdinaryTypeTargetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Struct(id) => write!(f, "Struct({})", id),
            Self::Enum(id) => write!(f, "Enum({})", id),
            Self::Union(id) => write!(f, "Union({})", id),
            Self::TypeAlias(id) => write!(f, "TypeAlias({})", id),
            Self::GenericParam(id) => write!(f, "GenericParam({})", id),
        }
    }
}

/// Node-ID family for trait-position targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum TraitTypeTargetId {
    Trait(TraitNodeId),
}

impl TraitTypeTargetId {
    #[inline]
    pub fn base_id(&self) -> ploke_core::NodeId {
        match *self {
            Self::Trait(id) => id.base_id(),
        }
    }

    #[inline]
    pub fn kind(&self) -> ItemKind {
        match *self {
            Self::Trait(_) => ItemKind::Trait,
        }
    }
}

impl AnyTypedId for TraitTypeTargetId {}
impl CategoricalTypedId for TraitTypeTargetId {}

impl From<TraitNodeId> for TraitTypeTargetId {
    #[inline]
    fn from(id: TraitNodeId) -> Self {
        Self::Trait(id)
    }
}

impl From<TraitTypeTargetId> for AnyNodeId {
    #[inline]
    fn from(id: TraitTypeTargetId) -> Self {
        match id {
            TraitTypeTargetId::Trait(id) => id.into(),
        }
    }
}

impl TryFrom<TraitTypeTargetId> for TraitNodeId {
    type Error = TryFromTraitTypeTargetError;

    #[inline]
    fn try_from(value: TraitTypeTargetId) -> Result<Self, Self::Error> {
        match value {
            TraitTypeTargetId::Trait(id) => Ok(id),
        }
    }
}

impl Display for TraitTypeTargetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Trait(id) => write!(f, "Trait({})", id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_core::NodeId;
    use uuid::Uuid;

    fn synthetic_node_id() -> NodeId {
        NodeId::Synthetic(Uuid::nil())
    }

    #[test]
    fn type_source_round_trips_named_and_trait_bound_ids() {
        let named = NamedTypeId::try_refine(
            TypeId::Synthetic(Uuid::new_v4()),
            &ploke_core::TypeKind::Named {
                path: vec!["Example".into()],
                is_fully_qualified: false,
            },
        )
        .expect("named kind should refine");
        let bound = TraitBoundTypeId::try_refine(
            TypeId::Synthetic(Uuid::new_v4()),
            &ploke_core::TypeKind::TraitBound {
                path: vec!["Display".into()],
                is_fully_qualified: false,
            },
        )
        .expect("trait-bound kind should refine");

        let named_src = TypeSourceId::from(named);
        let bound_src = TypeSourceId::from(bound);

        assert_eq!(NamedTypeId::try_from(named_src).expect("named"), named);
        assert_eq!(
            TraitBoundTypeId::try_from(bound_src).expect("trait bound"),
            bound
        );
    }

    #[test]
    fn ordinary_type_targets_accept_only_ordinary_type_nodes() {
        let target = OrdinaryTypeTargetId::from(StructNodeId::create(synthetic_node_id()));
        let struct_id = StructNodeId::try_from(target).expect("struct target should round-trip");

        assert_eq!(target.kind(), ItemKind::Struct);
        assert_eq!(AnyNodeId::from(target), AnyNodeId::from(struct_id));
        assert!(EnumNodeId::try_from(target).is_err());
    }

    #[test]
    fn trait_type_targets_are_separate_from_ordinary_targets() {
        let trait_id = TraitNodeId::create(synthetic_node_id());
        let target = TraitTypeTargetId::from(trait_id);

        assert_eq!(target.kind(), ItemKind::Trait);
        assert_eq!(
            TraitNodeId::try_from(target).expect("trait target"),
            trait_id
        );
    }
}
