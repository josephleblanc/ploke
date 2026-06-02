//! Typed structural nodes for the type graph.
//!
//! This module is the type-graph analogue of [`crate::parser::nodes`]: each
//! structural type node carries the refined ID that proves its admissible
//! `TypeKind` class. The erased [`TypeNode`] enum is only a sum over already
//! typed nodes, not the place where structural membership is proven.
//!
//! The older [`crate::parser::types::TypeNode`] representation remains in use
//! while the parser is migrated. New type-graph work should prefer these
//! typed-by-construction nodes so type-resolution relations can operate over
//! `Copy` endpoint IDs instead of repeatedly refining a raw `TypeId`.

use crate::parser::nodes::{
    AnyTypeId, ArrayTypeId, FunctionTypeId, ImplTraitTypeId, InferredTypeId, MacroTypeId,
    NamedTypeId, NeverTypeId, ParenTypeId, RawPointerTypeId, ReferenceTypeId, SliceTypeId,
    TraitBoundTypeId, TraitObjectTypeId, TupleTypeId, UnknownTypeId,
};
use crate::parser::type_slots::{OrdinaryTypeUseId, TraitTypeUseId};
use serde::{Deserialize, Serialize};

/// Erased structural type node.
///
/// Each variant contains a typed node whose `id` already proves membership in
/// that structural type class. Use this enum only at boundaries that need to
/// store or iterate over all structural type nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeNode {
    Named(NamedTypeNode),
    Reference(ReferenceTypeNode),
    Slice(SliceTypeNode),
    Array(ArrayTypeNode),
    Tuple(TupleTypeNode),
    Function(FunctionTypeNode),
    Never(NeverTypeNode),
    Inferred(InferredTypeNode),
    RawPointer(RawPointerTypeNode),
    TraitObject(TraitObjectTypeNode),
    ImplTrait(ImplTraitTypeNode),
    TraitBound(TraitBoundTypeNode),
    Paren(ParenTypeNode),
    Macro(MacroTypeNode),
    Unknown(UnknownTypeNode),
}

impl TypeNode {
    #[inline]
    pub fn id(&self) -> AnyTypeId {
        match self {
            Self::Named(node) => node.id.into(),
            Self::Reference(node) => node.id.into(),
            Self::Slice(node) => node.id.into(),
            Self::Array(node) => node.id.into(),
            Self::Tuple(node) => node.id.into(),
            Self::Function(node) => node.id.into(),
            Self::Never(node) => node.id.into(),
            Self::Inferred(node) => node.id.into(),
            Self::RawPointer(node) => node.id.into(),
            Self::TraitObject(node) => node.id.into(),
            Self::ImplTrait(node) => node.id.into(),
            Self::TraitBound(node) => node.id.into(),
            Self::Paren(node) => node.id.into(),
            Self::Macro(node) => node.id.into(),
            Self::Unknown(node) => node.id.into(),
        }
    }

    pub fn child_type_ids(&self) -> ChildTypeIds<'_> {
        match self {
            Self::Named(node) => {
                ChildTypeIds::named(node.qualified_self, node.qualified_trait, &node.arguments)
            }
            Self::Reference(node) => ChildTypeIds::single_ordinary(node.referenced),
            Self::Slice(node) => ChildTypeIds::single_ordinary(node.element),
            Self::Array(node) => ChildTypeIds::single_ordinary(node.element),
            Self::Tuple(node) => ChildTypeIds::ordinary_slice(&node.elements),
            Self::Function(node) => ChildTypeIds::function(&node.parameters, node.return_type),
            Self::Never(_) | Self::Inferred(_) | Self::Macro(_) | Self::Unknown(_) => {
                ChildTypeIds::empty()
            }
            Self::RawPointer(node) => ChildTypeIds::single_ordinary(node.pointee),
            Self::TraitObject(node) => ChildTypeIds::trait_slice(&node.bounds),
            Self::ImplTrait(node) => ChildTypeIds::trait_slice(&node.bounds),
            Self::TraitBound(node) => ChildTypeIds::ordinary_slice(&node.arguments),
            Self::Paren(node) => ChildTypeIds::single_ordinary(node.inner),
        }
    }
}

/// Iterator over child type IDs widened to the type-graph vertex umbrella.
///
/// Typed type-node payloads keep the narrow family proven by each syntactic
/// position. This iterator is the explicit traversal boundary where those
/// families widen into `AnyTypeId`.
pub enum ChildTypeIds<'a> {
    Empty,
    SingleOrdinary(Option<OrdinaryTypeUseId>),
    Named {
        qualified_self: Option<OrdinaryTypeUseId>,
        qualified_trait: Option<TraitTypeUseId>,
        arguments: &'a [OrdinaryTypeUseId],
        index: usize,
    },
    OrdinarySlice {
        ids: &'a [OrdinaryTypeUseId],
        index: usize,
    },
    TraitSlice {
        ids: &'a [TraitTypeUseId],
        index: usize,
    },
    Function {
        parameters: &'a [OrdinaryTypeUseId],
        index: usize,
        return_type: Option<OrdinaryTypeUseId>,
    },
}

impl<'a> ChildTypeIds<'a> {
    #[inline]
    fn empty() -> Self {
        Self::Empty
    }

    #[inline]
    fn single_ordinary(id: OrdinaryTypeUseId) -> Self {
        Self::SingleOrdinary(Some(id))
    }

    #[inline]
    fn named(
        qualified_self: Option<OrdinaryTypeUseId>,
        qualified_trait: Option<TraitTypeUseId>,
        arguments: &'a [OrdinaryTypeUseId],
    ) -> Self {
        Self::Named {
            qualified_self,
            qualified_trait,
            arguments,
            index: 0,
        }
    }

    #[inline]
    fn ordinary_slice(ids: &'a [OrdinaryTypeUseId]) -> Self {
        Self::OrdinarySlice { ids, index: 0 }
    }

    #[inline]
    fn trait_slice(ids: &'a [TraitTypeUseId]) -> Self {
        Self::TraitSlice { ids, index: 0 }
    }

    #[inline]
    fn function(
        parameters: &'a [OrdinaryTypeUseId],
        return_type: Option<OrdinaryTypeUseId>,
    ) -> Self {
        Self::Function {
            parameters,
            index: 0,
            return_type,
        }
    }
}

impl Iterator for ChildTypeIds<'_> {
    type Item = AnyTypeId;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::SingleOrdinary(id) => id.take().map(AnyTypeId::from),
            Self::Named {
                qualified_self,
                qualified_trait,
                arguments,
                index,
            } => {
                if let Some(id) = qualified_self.take() {
                    return Some(id.into());
                }
                if let Some(id) = qualified_trait.take() {
                    return Some(id.into());
                }
                let id = arguments.get(*index).copied()?;
                *index += 1;
                Some(id.into())
            }
            Self::OrdinarySlice { ids, index } => {
                let id = ids.get(*index).copied()?;
                *index += 1;
                Some(id.into())
            }
            Self::TraitSlice { ids, index } => {
                let id = ids.get(*index).copied()?;
                *index += 1;
                Some(id.into())
            }
            Self::Function {
                parameters,
                index,
                return_type,
            } => {
                if let Some(id) = parameters.get(*index).copied() {
                    *index += 1;
                    Some(id.into())
                } else {
                    return_type.take().map(AnyTypeId::from)
                }
            }
        }
    }
}

macro_rules! impl_type_node_from {
    ($Node:ty, $Variant:ident) => {
        impl From<$Node> for TypeNode {
            #[inline]
            fn from(node: $Node) -> Self {
                Self::$Variant(node)
            }
        }
    };
}

/// Named type syntax, such as `Foo`, `crate::m::Foo`, or `<T as Trait>::Assoc`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedTypeNode {
    pub id: NamedTypeId,
    pub path: Vec<String>,
    pub is_fully_qualified: bool,
    pub qualified_self: Option<OrdinaryTypeUseId>,
    pub qualified_trait: Option<TraitTypeUseId>,
    pub arguments: Vec<OrdinaryTypeUseId>,
}

impl_type_node_from!(NamedTypeNode, Named);

/// Reference type syntax, such as `&T` or `&'a mut T`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceTypeNode {
    pub id: ReferenceTypeId,
    pub lifetime: Option<String>,
    pub is_mutable: bool,
    pub referenced: OrdinaryTypeUseId,
}

impl_type_node_from!(ReferenceTypeNode, Reference);

/// Slice type syntax, such as `[T]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceTypeNode {
    pub id: SliceTypeId,
    pub element: OrdinaryTypeUseId,
}

impl_type_node_from!(SliceTypeNode, Slice);

/// Array type syntax, such as `[T; N]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrayTypeNode {
    pub id: ArrayTypeId,
    pub element: OrdinaryTypeUseId,
    pub size: Option<String>,
}

impl_type_node_from!(ArrayTypeNode, Array);

/// Tuple type syntax, such as `(A, B)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TupleTypeNode {
    pub id: TupleTypeId,
    pub elements: Vec<OrdinaryTypeUseId>,
}

impl_type_node_from!(TupleTypeNode, Tuple);

/// Function pointer type syntax, such as `unsafe extern "C" fn(A) -> B`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionTypeNode {
    pub id: FunctionTypeId,
    pub parameters: Vec<OrdinaryTypeUseId>,
    pub return_type: Option<OrdinaryTypeUseId>,
    pub is_unsafe: bool,
    pub is_extern: bool,
    pub abi: Option<String>,
}

impl_type_node_from!(FunctionTypeNode, Function);

/// Never type syntax, `!`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeverTypeNode {
    pub id: NeverTypeId,
}

impl_type_node_from!(NeverTypeNode, Never);

/// Inferred type syntax, `_`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InferredTypeNode {
    pub id: InferredTypeId,
}

impl_type_node_from!(InferredTypeNode, Inferred);

/// Raw pointer type syntax, such as `*const T` or `*mut T`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPointerTypeNode {
    pub id: RawPointerTypeId,
    pub is_mutable: bool,
    pub pointee: OrdinaryTypeUseId,
}

impl_type_node_from!(RawPointerTypeNode, RawPointer);

/// Trait object syntax, such as `dyn Display + Send`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraitObjectTypeNode {
    pub id: TraitObjectTypeId,
    pub dyn_token: bool,
    pub bounds: Vec<TraitTypeUseId>,
}

impl_type_node_from!(TraitObjectTypeNode, TraitObject);

/// `impl Trait` type syntax.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplTraitTypeNode {
    pub id: ImplTraitTypeId,
    pub bounds: Vec<TraitTypeUseId>,
}

impl_type_node_from!(ImplTraitTypeNode, ImplTrait);

/// Trait-bound type syntax, such as `Display` in `T: Display`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraitBoundTypeNode {
    pub id: TraitBoundTypeId,
    pub path: Vec<String>,
    pub is_fully_qualified: bool,
    pub arguments: Vec<OrdinaryTypeUseId>,
}

impl_type_node_from!(TraitBoundTypeNode, TraitBound);

/// Parenthesized type syntax, such as `(T)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParenTypeNode {
    pub id: ParenTypeId,
    pub inner: OrdinaryTypeUseId,
}

impl_type_node_from!(ParenTypeNode, Paren);

/// Macro type syntax.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroTypeNode {
    pub id: MacroTypeId,
    pub name: String,
    pub tokens: String,
}

impl_type_node_from!(MacroTypeNode, Macro);

/// Fallback type syntax that could not be structurally classified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownTypeNode {
    pub id: UnknownTypeId,
    pub type_str: String,
}

impl_type_node_from!(UnknownTypeNode, Unknown);

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_core::TypeId;
    use uuid::Uuid;

    fn synthetic_type_id() -> TypeId {
        TypeId::Synthetic(Uuid::nil())
    }

    #[test]
    fn erased_type_node_projects_refined_id() {
        let id = NamedTypeId::try_refine(
            synthetic_type_id(),
            &ploke_core::TypeKind::Named {
                path: vec!["Example".into()],
                is_fully_qualified: false,
            },
        )
        .expect("named type id");
        let node = TypeNode::from(NamedTypeNode {
            id,
            path: vec!["Example".into()],
            is_fully_qualified: false,
            qualified_self: None,
            qualified_trait: None,
            arguments: Vec::new(),
        });

        assert_eq!(node.id(), AnyTypeId::from(id));
    }
}
