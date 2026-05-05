//! Structural admissibility families for type resolution.
//!
//! These families play the same role for type-resolution endpoints that
//! `PrimaryNodeId` plays for module containment: they define which already-
//! typed IDs are admissible at a given boundary, so later relations can be
//! sharp about source and target shape instead of relying on conventions.
//!
//! The families in this module span two universes:
//!
//! ```text
//! V_type = the universe of structural type vertices, keyed by TypeId
//! V_node = the universe of code-graph node vertices, keyed by NodeId
//! ```
//!
//! Type-source families are subsets of `V_type`; they identify structural type
//! occurrences that can be resolved. Target families are subsets of `V_node`;
//! they identify code-graph definitions or generic parameters that a type
//! occurrence may resolve to.
//!
//! The currently encoded type-id families are:
//!
//! ```text
//! AnyTypeId =
//!     NamedTypeId
//!   ∪ ReferenceTypeId
//!   ∪ SliceTypeId
//!   ∪ ArrayTypeId
//!   ∪ TupleTypeId
//!   ∪ FunctionTypeId
//!   ∪ NeverTypeId
//!   ∪ InferredTypeId
//!   ∪ RawPointerTypeId
//!   ∪ TraitObjectTypeId
//!   ∪ ImplTraitTypeId
//!   ∪ TraitBoundTypeId
//!   ∪ ParenTypeId
//!   ∪ MacroTypeId
//!   ∪ UnknownTypeId
//!
//! TypeSourceId         = NamedTypeId ∪ TraitBoundTypeId
//! OrdinaryTypeSourceId = NamedTypeId
//! TraitTypeSourceId    = NamedTypeId ∪ TraitBoundTypeId
//! ```
//!
//! The currently encoded node-target families are:
//!
//! ```text
//! OrdinaryTypeDefId =
//!     StructNodeId
//!   ∪ EnumNodeId
//!   ∪ UnionNodeId
//!   ∪ TypeAliasNodeId
//!
//! OrdinaryTypeTargetId =
//!     OrdinaryTypeDefId
//!   ∪ TypeGenericParamNodeId
//!
//! TraitTypeTargetId = TraitNodeId
//! ```
//!
//! Therefore the corresponding relation endpoint products are:
//!
//! ```text
//! Ordinary ⊆ OrdinaryTypeSourceId × OrdinaryTypeTargetId
//! Trait    ⊆ TraitTypeSourceId    × TraitTypeTargetId
//! ```
//!
//! Some useful subset consequences are encoded directly in conversion APIs:
//!
//! ```text
//! OrdinaryTypeSourceId ⊂ TypeSourceId
//! TraitTypeSourceId    = TypeSourceId
//!
//! OrdinaryTypeDefId    ⊂ OrdinaryTypeTargetId
//! OrdinaryTypeTargetId ⊆ AnyNodeId
//! TraitTypeTargetId    ⊆ AnyNodeId
//! ```
//!
//! The forward arrows are infallible widenings, so they should be represented
//! with `From` when the membership proof is already present:
//!
//! ```text
//! OrdinaryTypeSourceId → TypeSourceId
//! TraitTypeSourceId    → TypeSourceId
//! OrdinaryTypeTargetId → AnyNodeId
//! TraitTypeTargetId    → AnyNodeId
//! ```
//!
//! The reverse arrows are refinements. They are `TryFrom` only when the source
//! value carries the discriminant needed to prove membership in the smaller
//! family:
//!
//! ```text
//! TypeSourceId ⇀ OrdinaryTypeSourceId
//! TypeSourceId ⇀ TraitTypeSourceId
//! ```
//!
//! By contrast, no general `TryFrom<AnyNodeId>` should be added here for target
//! families whose members may require payload refinement. For example,
//! `AnyNodeId::GenericParam` proves only `GenericParamNodeId`, not
//! `TypeGenericParamNodeId`; that narrower proof needs the corresponding
//! `GenericParamNode` payload.
//!
//! `Self`, primitives, builtins, and external dependency targets are not in
//! these node-backed target families. They require semantic target variants or
//! dependency graph vertices before they can be represented without weakening
//! the endpoint sets.

use super::{
    AnyNodeId, AnyTypedId, ArrayTypeId, CategoricalTypedId, EnumNodeId, FunctionTypeId,
    ImplTraitTypeId, InferredTypeId, MacroTypeId, NamedTypeId, NeverTypeId, ParenTypeId,
    RawPointerTypeId, ReferenceTypeId, SliceTypeId, StructNodeId, TraitBoundTypeId, TraitNodeId,
    TraitObjectTypeId, TupleTypeId, TypeAliasNodeId, TypeGenericParamNodeId, UnionNodeId,
    UnknownTypeId,
};
use crate::parser::nodes::ids::StructuralTypeId;
use ploke_core::{IdTrait, ItemKind, TypeId};
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
pub struct TryFromOrdinaryTypeSourceError;

impl Display for TryFromOrdinaryTypeSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OrdinaryTypeSourceId variant mismatch")
    }
}

impl Error for TryFromOrdinaryTypeSourceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TryFromTraitTypeSourceError;

impl Display for TryFromTraitTypeSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TraitTypeSourceId variant mismatch")
    }
}

impl Error for TryFromTraitTypeSourceError {}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TryFromAnyTypeError;

impl Display for TryFromAnyTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AnyTypeId variant mismatch")
    }
}

impl Error for TryFromAnyTypeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TryFromOrdinaryTypeDefError;

impl Display for TryFromOrdinaryTypeDefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OrdinaryTypeDefId variant mismatch")
    }
}

impl Error for TryFromOrdinaryTypeDefError {}

macro_rules! define_structural_type_family {
    (
        $(#[$outer:meta])*
        $Family:ident,
        $Error:ty,
        [
            ($Variant:ident, $IdType:ty $(,)?)
        ]
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
        pub enum $Family {
            $Variant($IdType),
        }

        impl $Family {
            #[inline]
            pub fn kind_name(&self) -> &'static str {
                match self {
                    Self::$Variant(_) => stringify!($IdType),
                }
            }
        }

        impl StructuralTypeId for $Family {
            #[inline]
            fn base_id(self) -> TypeId {
                match self {
                    Self::$Variant(id) => id.base_id(),
                }
            }
        }

        impl IdTrait for $Family {
            #[inline]
            fn uuid(&self) -> uuid::Uuid {
                self.base_id().uuid()
            }

            #[inline]
            fn is_resolved(&self) -> bool {
                self.base_id().is_resolved()
            }

            #[inline]
            fn is_synthetic(&self) -> bool {
                self.base_id().is_synthetic()
            }
        }

        impl AnyTypedId for $Family {}
        impl CategoricalTypedId for $Family {}

        impl From<$IdType> for $Family {
            #[inline]
            fn from(id: $IdType) -> Self {
                Self::$Variant(id)
            }
        }

        impl TryFrom<$Family> for $IdType {
            type Error = $Error;

            #[inline]
            fn try_from(value: $Family) -> Result<Self, Self::Error> {
                match value {
                    $Family::$Variant(id) => Ok(id),
                }
            }
        }

        impl From<$Family> for TypeId {
            #[inline]
            fn from(id: $Family) -> Self {
                id.base_id()
            }
        }

        impl Display for $Family {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::$Variant(id) => write!(f, "{}({})", stringify!($Variant), id),
                }
            }
        }

        impl Into<cozo::DataValue> for $Family {
            #[inline]
            fn into(self) -> cozo::DataValue {
                cozo::DataValue::Uuid(cozo::UuidWrapper(self.uuid()))
            }
        }
    };

    (
        $(#[$outer:meta])*
        $Family:ident,
        $Error:ty,
        [
            ($FirstVariant:ident, $FirstIdType:ty),
            $(($Variant:ident, $IdType:ty)),+ $(,)?
        ]
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
        pub enum $Family {
            $FirstVariant($FirstIdType),
            $(
                $Variant($IdType),
            )+
        }

        impl $Family {
            #[inline]
            pub fn kind_name(&self) -> &'static str {
                match self {
                    Self::$FirstVariant(_) => stringify!($FirstIdType),
                    $(
                        Self::$Variant(_) => stringify!($IdType),
                    )+
                }
            }
        }

        impl StructuralTypeId for $Family {
            #[inline]
            fn base_id(self) -> TypeId {
                match self {
                    Self::$FirstVariant(id) => id.base_id(),
                    $(
                        Self::$Variant(id) => id.base_id(),
                    )+
                }
            }
        }

        impl IdTrait for $Family {
            #[inline]
            fn uuid(&self) -> uuid::Uuid {
                self.base_id().uuid()
            }

            #[inline]
            fn is_resolved(&self) -> bool {
                self.base_id().is_resolved()
            }

            #[inline]
            fn is_synthetic(&self) -> bool {
                self.base_id().is_synthetic()
            }
        }

        impl AnyTypedId for $Family {}
        impl CategoricalTypedId for $Family {}

        impl From<$FirstIdType> for $Family {
            #[inline]
            fn from(id: $FirstIdType) -> Self {
                Self::$FirstVariant(id)
            }
        }

        impl TryFrom<$Family> for $FirstIdType {
            type Error = $Error;

            #[inline]
            fn try_from(value: $Family) -> Result<Self, Self::Error> {
                match value {
                    $Family::$FirstVariant(id) => Ok(id),
                    _ => Err(<$Error>::default()),
                }
            }
        }

        $(
            impl From<$IdType> for $Family {
                #[inline]
                fn from(id: $IdType) -> Self {
                    Self::$Variant(id)
                }
            }

            impl TryFrom<$Family> for $IdType {
                type Error = $Error;

                #[inline]
                fn try_from(value: $Family) -> Result<Self, Self::Error> {
                    match value {
                        $Family::$Variant(id) => Ok(id),
                        _ => Err(<$Error>::default()),
                    }
                }
            }
        )+

        impl From<$Family> for TypeId {
            #[inline]
            fn from(id: $Family) -> Self {
                id.base_id()
            }
        }

        impl Display for $Family {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::$FirstVariant(id) => write!(f, "{}({})", stringify!($FirstVariant), id),
                    $(
                        Self::$Variant(id) => write!(f, "{}({})", stringify!($Variant), id),
                    )+
                }
            }
        }

        impl Into<cozo::DataValue> for $Family {
            #[inline]
            fn into(self) -> cozo::DataValue {
                cozo::DataValue::Uuid(cozo::UuidWrapper(self.uuid()))
            }
        }
    };
}

macro_rules! define_node_type_target_family {
    (
        $(#[$outer:meta])*
        $Family:ident,
        $Error:ty,
        [
            ($Variant:ident, $IdType:ty, $ItemKindVal:expr $(,)?)
        ]
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
        pub enum $Family {
            $Variant($IdType),
        }

        impl $Family {
            #[inline]
            pub fn base_id(&self) -> ploke_core::NodeId {
                match *self {
                    Self::$Variant(id) => id.base_id(),
                }
            }

            #[inline]
            pub fn kind(&self) -> ItemKind {
                match *self {
                    Self::$Variant(_) => $ItemKindVal,
                }
            }
        }

        impl AnyTypedId for $Family {}
        impl CategoricalTypedId for $Family {}

        impl From<$Family> for AnyNodeId {
            #[inline]
            fn from(id: $Family) -> Self {
                match id {
                    $Family::$Variant(id) => id.into(),
                }
            }
        }

        impl From<$IdType> for $Family {
            #[inline]
            fn from(id: $IdType) -> Self {
                Self::$Variant(id)
            }
        }

        impl TryFrom<$Family> for $IdType {
            type Error = $Error;

            #[inline]
            fn try_from(value: $Family) -> Result<Self, Self::Error> {
                match value {
                    $Family::$Variant(id) => Ok(id),
                }
            }
        }

        impl Display for $Family {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match *self {
                    Self::$Variant(id) => write!(f, "{}({})", stringify!($Variant), id),
                }
            }
        }
    };

    (
        $(#[$outer:meta])*
        $Family:ident,
        $Error:ty,
        [
            ($FirstVariant:ident, $FirstIdType:ty, $FirstItemKindVal:expr),
            $(($Variant:ident, $IdType:ty, $ItemKindVal:expr)),+ $(,)?
        ]
    ) => {
        $(#[$outer])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
        pub enum $Family {
            $FirstVariant($FirstIdType),
            $(
                $Variant($IdType),
            )+
        }

        impl $Family {
            #[inline]
            pub fn base_id(&self) -> ploke_core::NodeId {
                match *self {
                    Self::$FirstVariant(id) => id.base_id(),
                    $(
                        Self::$Variant(id) => id.base_id(),
                    )+
                }
            }

            #[inline]
            pub fn kind(&self) -> ItemKind {
                match *self {
                    Self::$FirstVariant(_) => $FirstItemKindVal,
                    $(
                        Self::$Variant(_) => $ItemKindVal,
                    )+
                }
            }
        }

        impl AnyTypedId for $Family {}
        impl CategoricalTypedId for $Family {}

        impl From<$Family> for AnyNodeId {
            #[inline]
            fn from(id: $Family) -> Self {
                match id {
                    $Family::$FirstVariant(id) => id.into(),
                    $(
                        $Family::$Variant(id) => id.into(),
                    )+
                }
            }
        }

        impl From<$FirstIdType> for $Family {
            #[inline]
            fn from(id: $FirstIdType) -> Self {
                Self::$FirstVariant(id)
            }
        }

        impl TryFrom<$Family> for $FirstIdType {
            type Error = $Error;

            #[inline]
            fn try_from(value: $Family) -> Result<Self, Self::Error> {
                match value {
                    $Family::$FirstVariant(id) => Ok(id),
                    _ => Err(<$Error>::default()),
                }
            }
        }

        $(
            impl From<$IdType> for $Family {
                #[inline]
                fn from(id: $IdType) -> Self {
                    Self::$Variant(id)
                }
            }

            impl TryFrom<$Family> for $IdType {
                type Error = $Error;

                #[inline]
                fn try_from(value: $Family) -> Result<Self, Self::Error> {
                    match value {
                        $Family::$Variant(id) => Ok(id),
                        _ => Err(<$Error>::default()),
                    }
                }
            }
        )+

        impl Display for $Family {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match *self {
                    Self::$FirstVariant(id) => write!(f, "{}({})", stringify!($FirstVariant), id),
                    $(
                        Self::$Variant(id) => write!(f, "{}({})", stringify!($Variant), id),
                    )+
                }
            }
        }
    };
}

define_structural_type_family!(
    /// Represents the ID of any currently modeled structural type node.
    ///
    /// Set-theoretically, this is the top family for the type-id universe,
    /// analogous to `AnyNodeId` for node IDs.
    AnyTypeId,
    TryFromAnyTypeError,
    [
        (Named, NamedTypeId),
        (Reference, ReferenceTypeId),
        (Slice, SliceTypeId),
        (Array, ArrayTypeId),
        (Tuple, TupleTypeId),
        (Function, FunctionTypeId),
        (Never, NeverTypeId),
        (Inferred, InferredTypeId),
        (RawPointer, RawPointerTypeId),
        (TraitObject, TraitObjectTypeId),
        (ImplTrait, ImplTraitTypeId),
        (TraitBound, TraitBoundTypeId),
        (Paren, ParenTypeId),
        (Macro, MacroTypeId),
        (Unknown, UnknownTypeId),
    ]
);

define_structural_type_family!(
    /// Structural type-source classes that can directly participate in resolution.
    ///
    /// This is intentionally narrower than "all type IDs". Composite type nodes
    /// like tuples, references, or arrays remain containers in the type graph and
    /// are traversed through their nested children instead of resolving directly.
    TypeSourceId,
    TryFromTypeSourceError,
    [(Named, NamedTypeId), (TraitBound, TraitBoundTypeId)]
);

define_structural_type_family!(
    /// Type-source family for ordinary type-position resolution.
    ///
    /// Set-theoretically:
    ///
    /// ```text
    /// OrdinaryTypeSourceId = NamedTypeId
    /// ```
    ///
    /// This admits syntactic named type occurrences such as `Foo`, `crate::m::Foo`,
    /// or the `Vec` in `Vec<T>`. Trait-bound syntax is intentionally excluded
    /// because it resolves in trait position, not ordinary type position.
    OrdinaryTypeSourceId,
    TryFromOrdinaryTypeSourceError,
    [(Named, NamedTypeId)]
);

impl From<OrdinaryTypeSourceId> for TypeSourceId {
    #[inline]
    fn from(id: OrdinaryTypeSourceId) -> Self {
        match id {
            OrdinaryTypeSourceId::Named(id) => TypeSourceId::Named(id),
        }
    }
}

impl From<OrdinaryTypeSourceId> for AnyTypeId {
    #[inline]
    fn from(id: OrdinaryTypeSourceId) -> Self {
        match id {
            OrdinaryTypeSourceId::Named(id) => AnyTypeId::Named(id),
        }
    }
}

impl TryFrom<TypeSourceId> for OrdinaryTypeSourceId {
    type Error = TryFromOrdinaryTypeSourceError;

    #[inline]
    fn try_from(value: TypeSourceId) -> Result<Self, Self::Error> {
        match value {
            TypeSourceId::Named(id) => Ok(Self::Named(id)),
            TypeSourceId::TraitBound(_) => Err(TryFromOrdinaryTypeSourceError),
        }
    }
}

define_structural_type_family!(
    /// Type-source family for trait-position resolution.
    ///
    /// Set-theoretically:
    ///
    /// ```text
    /// TraitTypeSourceId = NamedTypeId ∪ TraitBoundTypeId
    /// ```
    ///
    /// `NamedTypeId` covers trait paths in positions such as `impl Display for T`
    /// or supertrait paths. `TraitBoundTypeId` covers bounds such as `T: Display`.
    TraitTypeSourceId,
    TryFromTraitTypeSourceError,
    [(Named, NamedTypeId), (TraitBound, TraitBoundTypeId)]
);

impl From<TraitTypeSourceId> for TypeSourceId {
    #[inline]
    fn from(id: TraitTypeSourceId) -> Self {
        match id {
            TraitTypeSourceId::Named(id) => TypeSourceId::Named(id),
            TraitTypeSourceId::TraitBound(id) => TypeSourceId::TraitBound(id),
        }
    }
}

impl From<TraitTypeSourceId> for AnyTypeId {
    #[inline]
    fn from(id: TraitTypeSourceId) -> Self {
        match id {
            TraitTypeSourceId::Named(id) => AnyTypeId::Named(id),
            TraitTypeSourceId::TraitBound(id) => AnyTypeId::TraitBound(id),
        }
    }
}

impl TryFrom<TypeSourceId> for TraitTypeSourceId {
    type Error = TryFromTraitTypeSourceError;

    #[inline]
    fn try_from(value: TypeSourceId) -> Result<Self, Self::Error> {
        match value {
            TypeSourceId::Named(id) => Ok(Self::Named(id)),
            TypeSourceId::TraitBound(id) => Ok(Self::TraitBound(id)),
        }
    }
}

define_node_type_target_family!(
    /// Node-ID family for ordinary nominal type definitions.
    ///
    /// This is the node-backed definition subset of ordinary type targets. It
    /// intentionally excludes generic parameters, which are admissible ordinary
    /// type targets but not type definitions.
    OrdinaryTypeDefId,
    TryFromOrdinaryTypeDefError,
    [
        (Struct, StructNodeId, ItemKind::Struct),
        (Enum, EnumNodeId, ItemKind::Enum),
        (Union, UnionNodeId, ItemKind::Union),
        (TypeAlias, TypeAliasNodeId, ItemKind::TypeAlias),
    ]
);

define_node_type_target_family!(
    /// Node-ID family for ordinary type-position targets.
    ///
    /// This covers the code-graph nodes that are structurally valid targets for
    /// plain named type use-sites. Semantic-only targets like builtins or `Self`
    /// are intentionally left out of this family because they are not node IDs.
    OrdinaryTypeTargetId,
    TryFromOrdinaryTypeTargetError,
    [
        (Struct, StructNodeId, ItemKind::Struct),
        (Enum, EnumNodeId, ItemKind::Enum),
        (Union, UnionNodeId, ItemKind::Union),
        (TypeAlias, TypeAliasNodeId, ItemKind::TypeAlias),
        (GenericParam, TypeGenericParamNodeId, ItemKind::GenericParam),
    ]
);

define_node_type_target_family!(
    /// Node-ID family for trait-position targets.
    TraitTypeTargetId,
    TryFromTraitTypeTargetError,
    [(Trait, TraitNodeId, ItemKind::Trait)]
);

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
    fn any_type_accepts_all_structural_type_wrappers() {
        let named = NamedTypeId::try_refine(
            TypeId::Synthetic(Uuid::new_v4()),
            &ploke_core::TypeKind::Named {
                path: vec!["Example".into()],
                is_fully_qualified: false,
            },
        )
        .expect("named kind should refine");
        let never = NeverTypeId::try_refine(
            TypeId::Synthetic(Uuid::new_v4()),
            &ploke_core::TypeKind::Never,
        )
        .expect("never kind should refine");

        let named_any = AnyTypeId::from(named);
        let never_any = AnyTypeId::from(never);

        assert_eq!(NamedTypeId::try_from(named_any).expect("named"), named);
        assert_eq!(NeverTypeId::try_from(never_any).expect("never"), never);
        assert!(NeverTypeId::try_from(named_any).is_err());
    }

    #[test]
    fn ordinary_type_defs_are_separate_from_generic_type_targets() {
        let struct_id = StructNodeId::create(synthetic_node_id());
        let def = OrdinaryTypeDefId::from(struct_id);
        let target = OrdinaryTypeTargetId::from(struct_id);

        assert_eq!(def.kind(), ItemKind::Struct);
        assert_eq!(target.kind(), ItemKind::Struct);
        assert_eq!(AnyNodeId::from(def), AnyNodeId::from(struct_id));
        assert_eq!(AnyNodeId::from(target), AnyNodeId::from(struct_id));
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
