use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Database, DbError};

/// Role of a root type use on a code-graph owner.
///
/// This is the DB-facing analogue of the parser-side type-use slots. It is
/// intentionally kept at the owner/root boundary: deeper structural traversal
/// belongs to [`TypeContainmentEdge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeUseRole {
    FunctionReturn,
    FunctionParam,
    MethodReturn,
    MethodParam,
    FieldType,
    TypeAliasTarget,
    ImplSelf,
    ImplTrait,
    TraitSuper,
    ConstType,
    StaticType,
}

/// A direct edge from a code-graph owner to the root structural type used in a
/// syntactic slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeUseRoot {
    pub owner_id: Uuid,
    pub root_type_id: Uuid,
    pub role: TypeUseRole,
    pub slot_index: Option<u32>,
}

/// Structural containment edge between two type-graph vertices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeContainmentKind {
    Argument,
    Element,
    Referenced,
    Pointee,
    TraitBound,
    Inner,
    FunctionParam,
    FunctionReturn,
}

/// A normalized containment edge for traversing complex structural types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeContainmentEdge {
    pub parent_type_id: Uuid,
    pub child_type_id: Uuid,
    pub kind: TypeContainmentKind,
    pub position: Option<u32>,
}

/// Semantic resolution family for a terminal structural type source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeRelationKind {
    Ordinary,
    Trait,
}

/// Resolved type-definition target reachable from a code-graph owner's type
/// use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeTargetPath {
    pub owner_id: Uuid,
    pub root_type_id: Uuid,
    pub terminal_type_id: Uuid,
    pub target_id: Uuid,
    pub relation_kind: TypeRelationKind,
    pub depth: u32,
}

impl Database {
    /// Returns all root structural type uses attached directly to `owner_id`.
    pub fn type_uses_for_owner(&self, owner_id: Uuid) -> Result<Vec<TypeUseRoot>, DbError> {
        let _ = owner_id;
        todo!("query direct owner -> root structural type-use rows")
    }

    /// Returns direct structural containment edges for `root_type_id`.
    pub fn direct_type_contains(
        &self,
        root_type_id: Uuid,
    ) -> Result<Vec<TypeContainmentEdge>, DbError> {
        let _ = root_type_id;
        todo!("query direct structural type containment edges")
    }

    /// Returns resolved definition targets reachable through an owner's root
    /// type uses, structural containment closure, and terminal type relations.
    pub fn type_targets_reachable_from_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<TypeTargetPath>, DbError> {
        let _ = owner_id;
        todo!("query owner -> root type -> contains* -> terminal -> type_relation")
    }
}
