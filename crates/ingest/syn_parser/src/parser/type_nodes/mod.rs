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
    StructuralTypeId, TraitBoundTypeId, TraitObjectTypeId, TupleTypeId, UnknownTypeId,
};
use ploke_core::TypeId;
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

    #[inline]
    pub fn base_id(&self) -> TypeId {
        self.id().base_id()
    }

    pub fn child_type_ids(&self) -> impl Iterator<Item = AnyTypeId> + '_ {
        let children: &[AnyTypeId] = match self {
            Self::Named(node) => &node.arguments,
            Self::Reference(node) => std::slice::from_ref(&node.referenced),
            Self::Slice(node) => std::slice::from_ref(&node.element),
            Self::Array(node) => std::slice::from_ref(&node.element),
            Self::Tuple(node) => &node.elements,
            Self::Function(node) => &node.parameters,
            Self::Never(_) | Self::Inferred(_) | Self::Macro(_) | Self::Unknown(_) => &[],
            Self::RawPointer(node) => std::slice::from_ref(&node.pointee),
            Self::TraitObject(node) => &node.bounds,
            Self::ImplTrait(node) => &node.bounds,
            Self::TraitBound(node) => &node.arguments,
            Self::Paren(node) => std::slice::from_ref(&node.inner),
        };
        children.iter().copied().chain(match self {
            Self::Function(node) => node.return_type.into_iter(),
            _ => None.into_iter(),
        })
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
    pub arguments: Vec<AnyTypeId>,
}

impl_type_node_from!(NamedTypeNode, Named);

/// Reference type syntax, such as `&T` or `&'a mut T`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceTypeNode {
    pub id: ReferenceTypeId,
    pub lifetime: Option<String>,
    pub is_mutable: bool,
    pub referenced: AnyTypeId,
}

impl_type_node_from!(ReferenceTypeNode, Reference);

/// Slice type syntax, such as `[T]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceTypeNode {
    pub id: SliceTypeId,
    pub element: AnyTypeId,
}

impl_type_node_from!(SliceTypeNode, Slice);

/// Array type syntax, such as `[T; N]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrayTypeNode {
    pub id: ArrayTypeId,
    pub element: AnyTypeId,
    pub size: Option<String>,
}

impl_type_node_from!(ArrayTypeNode, Array);

/// Tuple type syntax, such as `(A, B)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TupleTypeNode {
    pub id: TupleTypeId,
    pub elements: Vec<AnyTypeId>,
}

impl_type_node_from!(TupleTypeNode, Tuple);

/// Function pointer type syntax, such as `unsafe extern "C" fn(A) -> B`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionTypeNode {
    pub id: FunctionTypeId,
    pub parameters: Vec<AnyTypeId>,
    pub return_type: Option<AnyTypeId>,
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
    pub pointee: AnyTypeId,
}

impl_type_node_from!(RawPointerTypeNode, RawPointer);

/// Trait object syntax, such as `dyn Display + Send`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraitObjectTypeNode {
    pub id: TraitObjectTypeId,
    pub dyn_token: bool,
    pub bounds: Vec<AnyTypeId>,
}

impl_type_node_from!(TraitObjectTypeNode, TraitObject);

/// `impl Trait` type syntax.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplTraitTypeNode {
    pub id: ImplTraitTypeId,
    pub bounds: Vec<AnyTypeId>,
}

impl_type_node_from!(ImplTraitTypeNode, ImplTrait);

/// Trait-bound type syntax, such as `Display` in `T: Display`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraitBoundTypeNode {
    pub id: TraitBoundTypeId,
    pub path: Vec<String>,
    pub is_fully_qualified: bool,
    pub arguments: Vec<AnyTypeId>,
}

impl_type_node_from!(TraitBoundTypeNode, TraitBound);

/// Parenthesized type syntax, such as `(T)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParenTypeNode {
    pub id: ParenTypeId,
    pub inner: AnyTypeId,
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
            arguments: Vec::new(),
        });

        assert_eq!(node.id(), AnyTypeId::from(id));
        assert_eq!(node.base_id(), TypeId::from(id));
    }
}
