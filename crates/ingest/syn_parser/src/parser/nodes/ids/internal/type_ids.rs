//! Refined structural wrappers over [`TypeId`].
//!
//! These wrappers play the same role for the type graph that `FunctionNodeId`,
//! `StructNodeId`, and related typed node IDs play for the node graph: they
//! let later code express admissible source kinds at the type level instead of
//! relying on conventions over a bare `TypeId`.

#[cfg(not(feature = "typed_type_graph"))]
use crate::parser::types::TypeNode;
use ploke_core::{IdTrait, TypeId, TypeKind};
use serde::{Deserialize, Serialize};
use std::fmt;

fn type_kind_name(kind: &TypeKind) -> &'static str {
    match kind {
        TypeKind::Named { .. } => "Named",
        TypeKind::Reference { .. } => "Reference",
        TypeKind::Slice { .. } => "Slice",
        TypeKind::Array { .. } => "Array",
        TypeKind::Tuple { .. } => "Tuple",
        TypeKind::Function { .. } => "Function",
        TypeKind::Never => "Never",
        TypeKind::Inferred => "Inferred",
        TypeKind::RawPointer { .. } => "RawPointer",
        TypeKind::TraitObject { .. } => "TraitObject",
        TypeKind::ImplTrait { .. } => "ImplTrait",
        TypeKind::TraitBound { .. } => "TraitBound",
        TypeKind::Paren { .. } => "Paren",
        TypeKind::Macro { .. } => "Macro",
        TypeKind::Unknown { .. } => "Unknown",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot refine TypeId {type_id} into {expected}; actual TypeKind is {found}")]
pub struct TypeIdRefinementError {
    expected: &'static str,
    found: &'static str,
    type_id: TypeId,
}

impl TypeIdRefinementError {
    fn new(expected: &'static str, found: &'static str, type_id: TypeId) -> Self {
        Self {
            expected,
            found,
            type_id,
        }
    }
}

pub(in crate::parser) trait StructuralTypeId:
    Copy + fmt::Debug + std::hash::Hash + Eq + Ord + Serialize + for<'a> Deserialize<'a> + Send + Sync
{
    fn base_id(self) -> TypeId;

    fn uuid(self) -> uuid::Uuid {
        self.base_id().uuid()
    }
}

macro_rules! define_structural_type_id {
    ($Name:ident, $Variant:ident { .. }) => {
        #[doc = concat!("Refined `TypeId` for `TypeKind::", stringify!($Variant), "`.")]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord,
        )]
        pub struct $Name(TypeId);

        impl $Name {
            #[inline]
            pub fn matches(kind: &TypeKind) -> bool {
                matches!(kind, TypeKind::$Variant { .. })
            }

            #[inline]
            pub(in crate::parser) fn try_refine(
                id: TypeId,
                kind: &TypeKind,
            ) -> Result<Self, TypeIdRefinementError> {
                if Self::matches(kind) {
                    Ok(Self(id))
                } else {
                    Err(TypeIdRefinementError::new(
                        stringify!($Name),
                        type_kind_name(kind),
                        id,
                    ))
                }
            }

            #[inline]
            pub fn kind_name() -> &'static str {
                stringify!($Name)
            }
        }

        impl StructuralTypeId for $Name {
            #[inline]
            fn base_id(self) -> TypeId {
                self.0
            }
        }

        impl IdTrait for $Name {
            #[inline]
            fn uuid(&self) -> uuid::Uuid {
                self.0.uuid()
            }

            #[inline]
            fn is_resolved(&self) -> bool {
                self.0.is_resolved()
            }

            #[inline]
            fn is_synthetic(&self) -> bool {
                self.0.is_synthetic()
            }
        }

        impl super::ToCozoUuid for $Name {
            #[inline]
            fn to_cozo_uuid(self) -> cozo::DataValue {
                cozo::DataValue::Uuid(cozo::UuidWrapper(self.uuid()))
            }
        }

        #[cfg(not(feature = "typed_type_graph"))]
        impl From<$Name> for TypeId {
            #[inline]
            fn from(id: $Name) -> Self {
                id.0
            }
        }

        #[cfg(not(feature = "typed_type_graph"))]
        impl TryFrom<&TypeNode> for $Name {
            type Error = TypeIdRefinementError;

            #[inline]
            fn try_from(node: &TypeNode) -> Result<Self, Self::Error> {
                Self::try_refine(node.id, &node.kind)
            }
        }

        impl fmt::Display for $Name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($Name), self.0)
            }
        }
    };

    ($Name:ident, $Variant:ident) => {
        #[doc = concat!("Refined `TypeId` for `TypeKind::", stringify!($Variant), "`.")]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord,
        )]
        pub struct $Name(TypeId);

        impl $Name {
            #[inline]
            pub fn matches(kind: &TypeKind) -> bool {
                matches!(kind, TypeKind::$Variant)
            }

            #[inline]
            pub(in crate::parser) fn try_refine(
                id: TypeId,
                kind: &TypeKind,
            ) -> Result<Self, TypeIdRefinementError> {
                if Self::matches(kind) {
                    Ok(Self(id))
                } else {
                    Err(TypeIdRefinementError::new(
                        stringify!($Name),
                        type_kind_name(kind),
                        id,
                    ))
                }
            }

            #[inline]
            pub fn kind_name() -> &'static str {
                stringify!($Name)
            }
        }

        impl StructuralTypeId for $Name {
            #[inline]
            fn base_id(self) -> TypeId {
                self.0
            }
        }

        impl IdTrait for $Name {
            #[inline]
            fn uuid(&self) -> uuid::Uuid {
                self.0.uuid()
            }

            #[inline]
            fn is_resolved(&self) -> bool {
                self.0.is_resolved()
            }

            #[inline]
            fn is_synthetic(&self) -> bool {
                self.0.is_synthetic()
            }
        }

        impl super::ToCozoUuid for $Name {
            #[inline]
            fn to_cozo_uuid(self) -> cozo::DataValue {
                cozo::DataValue::Uuid(cozo::UuidWrapper(self.uuid()))
            }
        }

        #[cfg(not(feature = "typed_type_graph"))]
        impl From<$Name> for TypeId {
            #[inline]
            fn from(id: $Name) -> Self {
                id.0
            }
        }

        #[cfg(not(feature = "typed_type_graph"))]
        impl TryFrom<&TypeNode> for $Name {
            type Error = TypeIdRefinementError;

            #[inline]
            fn try_from(node: &TypeNode) -> Result<Self, Self::Error> {
                Self::try_refine(node.id, &node.kind)
            }
        }

        impl fmt::Display for $Name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($Name), self.0)
            }
        }
    };
}

define_structural_type_id!(NamedTypeId, Named { .. });
define_structural_type_id!(ReferenceTypeId, Reference { .. });
define_structural_type_id!(SliceTypeId, Slice { .. });
define_structural_type_id!(ArrayTypeId, Array { .. });
define_structural_type_id!(TupleTypeId, Tuple { .. });
define_structural_type_id!(FunctionTypeId, Function { .. });
define_structural_type_id!(NeverTypeId, Never);
define_structural_type_id!(InferredTypeId, Inferred);
define_structural_type_id!(RawPointerTypeId, RawPointer { .. });
define_structural_type_id!(TraitObjectTypeId, TraitObject { .. });
define_structural_type_id!(ImplTraitTypeId, ImplTrait { .. });
define_structural_type_id!(TraitBoundTypeId, TraitBound { .. });
define_structural_type_id!(ParenTypeId, Paren { .. });
define_structural_type_id!(MacroTypeId, Macro { .. });
define_structural_type_id!(UnknownTypeId, Unknown { .. });

#[cfg(all(test, not(feature = "typed_type_graph")))]
mod tests {
    use super::*;

    fn named_node() -> TypeNode {
        TypeNode {
            id: TypeId::Synthetic(uuid::Uuid::nil()),
            kind: TypeKind::Named {
                path: vec!["Example".to_string()],
                is_fully_qualified: false,
            },
            related_types: Vec::new(),
        }
    }

    #[test]
    fn refines_named_type_ids() {
        let node = named_node();
        let named = NamedTypeId::try_from(&node).expect("named type should refine");
        assert_eq!(named.base_id(), node.id);
    }

    #[test]
    fn rejects_mismatched_type_kinds() {
        let node = TypeNode {
            id: TypeId::Synthetic(uuid::Uuid::nil()),
            kind: TypeKind::Tuple {},
            related_types: Vec::new(),
        };

        let err = NamedTypeId::try_from(&node).expect_err("tuple type should not refine to named");
        assert!(err.to_string().contains("NamedTypeId"));
        assert!(err.to_string().contains("Tuple"));
    }
}
