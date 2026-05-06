use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::collections::BTreeMap;

use cozo::{DataValue, Num, ScriptMutability, UuidWrapper};

use crate::{
    Database, DbError,
    database::{to_string, to_uuid},
};

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

impl TypeUseRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::FunctionReturn => "FunctionReturn",
            Self::FunctionParam => "FunctionParam",
            Self::MethodReturn => "MethodReturn",
            Self::MethodParam => "MethodParam",
            Self::FieldType => "FieldType",
            Self::TypeAliasTarget => "TypeAliasTarget",
            Self::ImplSelf => "ImplSelf",
            Self::ImplTrait => "ImplTrait",
            Self::TraitSuper => "TraitSuper",
            Self::ConstType => "ConstType",
            Self::StaticType => "StaticType",
        }
    }

    fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "FunctionReturn" => Ok(Self::FunctionReturn),
            "FunctionParam" => Ok(Self::FunctionParam),
            "MethodReturn" => Ok(Self::MethodReturn),
            "MethodParam" => Ok(Self::MethodParam),
            "FieldType" => Ok(Self::FieldType),
            "TypeAliasTarget" => Ok(Self::TypeAliasTarget),
            "ImplSelf" => Ok(Self::ImplSelf),
            "ImplTrait" => Ok(Self::ImplTrait),
            "TraitSuper" => Ok(Self::TraitSuper),
            "ConstType" => Ok(Self::ConstType),
            "StaticType" => Ok(Self::StaticType),
            other => Err(DbError::Cozo(format!("unknown type-use role {other:?}"))),
        }
    }
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

impl TypeContainmentKind {
    fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "Argument" => Ok(Self::Argument),
            "Element" => Ok(Self::Element),
            "Referenced" => Ok(Self::Referenced),
            "Pointee" => Ok(Self::Pointee),
            "TraitBound" => Ok(Self::TraitBound),
            "Inner" => Ok(Self::Inner),
            "FunctionParam" => Ok(Self::FunctionParam),
            "FunctionReturn" => Ok(Self::FunctionReturn),
            other => Err(DbError::Cozo(format!(
                "unknown type-containment kind {other:?}"
            ))),
        }
    }
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

impl TypeRelationKind {
    fn from_str(value: &str) -> Result<Self, DbError> {
        match value {
            "Ordinary" => Ok(Self::Ordinary),
            "Trait" => Ok(Self::Trait),
            other => Err(DbError::Cozo(format!(
                "unknown type-relation kind {other:?}"
            ))),
        }
    }
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

/// Code-graph owner related to another owner through a shared resolved type
/// definition.
///
/// `origin_depth` and `related_depth` are measured from each owner's root type
/// use to the terminal type source that resolved to `shared_target_id`.
/// `distance` is the sum of those depths. This gives retrieval code a first
/// structural ranking signal: exact root matches are closer than matches found
/// only through nested generic arguments, tuple elements, references, or trait
/// object children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeRelatedOwner {
    pub origin_owner_id: Uuid,
    pub related_owner_id: Uuid,
    pub shared_target_id: Uuid,
    pub relation_kind: TypeRelationKind,
    pub origin_depth: u32,
    pub related_depth: u32,
    pub distance: u32,
}

impl Database {
    /// Returns all root structural type uses attached directly to `owner_id`.
    pub fn type_uses_for_owner(&self, owner_id: Uuid) -> Result<Vec<TypeUseRoot>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let rows = self.run_script(
            r#"?[owner_id, root_type_id, role, slot_index] :=
                owner_id = $owner_id,
                *type_use { owner_id, root_type_id, role, slot_index @ 'NOW' }"#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows
            .iter()
            .map(|row| {
                Ok(TypeUseRoot {
                    owner_id: to_uuid(&row[0])?,
                    root_type_id: to_uuid(&row[1])?,
                    role: TypeUseRole::from_str(&to_string(&row[2])?)?,
                    slot_index: optional_index(&row[3])?,
                })
            })
            .collect()
    }

    /// Returns direct structural containment edges for `root_type_id`.
    pub fn direct_type_contains(
        &self,
        root_type_id: Uuid,
    ) -> Result<Vec<TypeContainmentEdge>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "parent_type_id".to_string(),
            DataValue::Uuid(UuidWrapper(root_type_id)),
        );

        let rows = self.run_script(
            r#"?[parent_type_id, child_type_id, kind, position] :=
                parent_type_id = $parent_type_id,
                *type_contains { parent_type_id, child_type_id, kind, position @ 'NOW' }
            :sort position"#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows
            .iter()
            .map(|row| {
                Ok(TypeContainmentEdge {
                    parent_type_id: to_uuid(&row[0])?,
                    child_type_id: to_uuid(&row[1])?,
                    kind: TypeContainmentKind::from_str(&to_string(&row[2])?)?,
                    position: optional_index(&row[3])?,
                })
            })
            .collect()
    }

    /// Returns resolved definition targets reachable through an owner's root
    /// type uses, structural containment closure, and terminal type relations.
    pub fn type_targets_reachable_from_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<TypeTargetPath>, DbError> {
        const MAX_TYPE_DEPTH: u32 = 32;

        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );
        params.insert(
            "max_depth".to_string(),
            DataValue::Num(Num::Int(i64::from(MAX_TYPE_DEPTH))),
        );

        let rows = self.run_script(
            r#"
            roots[owner_id, root_type_id] :=
                owner_id = $owner_id,
                *type_use { owner_id, root_type_id, role, slot_index @ 'NOW' }

            reachable[owner_id, root_type_id, terminal_type_id, depth] :=
                roots[owner_id, root_type_id],
                terminal_type_id = root_type_id,
                depth = 0

            reachable[owner_id, root_type_id, terminal_type_id, depth] :=
                reachable[owner_id, root_type_id, parent_type_id, previous_depth],
                previous_depth < $max_depth,
                *type_contains {
                    parent_type_id,
                    child_type_id: terminal_type_id,
                    kind,
                    position @ 'NOW'
                },
                depth = previous_depth + 1

            ?[owner_id, root_type_id, terminal_type_id, target_id, relation_kind, depth] :=
                reachable[owner_id, root_type_id, terminal_type_id, depth],
                *type_relation {
                    source_id: terminal_type_id,
                    target_id,
                    relation_kind @ 'NOW'
                }
            "#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows
            .iter()
            .map(|row| {
                Ok(TypeTargetPath {
                    owner_id: to_uuid(&row[0])?,
                    root_type_id: to_uuid(&row[1])?,
                    terminal_type_id: to_uuid(&row[2])?,
                    target_id: to_uuid(&row[3])?,
                    relation_kind: TypeRelationKind::from_str(&to_string(&row[4])?)?,
                    depth: required_index(&row[5])?,
                })
            })
            .collect()
    }

    /// Returns code-graph owners whose type uses reach `target_id`.
    ///
    /// This is the target-centered form of [`Self::type_targets_reachable_from_owner`].
    /// It is the natural query when graphRAG starts from an edited type or
    /// trait definition and needs to find nearby functions, fields, aliases,
    /// impls, and other owners that mention it directly or through nested type
    /// structure.
    pub fn type_owners_for_target(&self, target_id: Uuid) -> Result<Vec<TypeTargetPath>, DbError> {
        const MAX_TYPE_DEPTH: u32 = 32;

        let mut params = BTreeMap::new();
        params.insert(
            "target_id".to_string(),
            DataValue::Uuid(UuidWrapper(target_id)),
        );
        params.insert(
            "max_depth".to_string(),
            DataValue::Num(Num::Int(i64::from(MAX_TYPE_DEPTH))),
        );

        let rows = self.run_script(
            r#"
            roots[owner_id, root_type_id] :=
                *type_use { owner_id, root_type_id, role, slot_index @ 'NOW' }

            reachable[owner_id, root_type_id, terminal_type_id, depth] :=
                roots[owner_id, root_type_id],
                terminal_type_id = root_type_id,
                depth = 0

            reachable[owner_id, root_type_id, terminal_type_id, depth] :=
                reachable[owner_id, root_type_id, parent_type_id, previous_depth],
                previous_depth < $max_depth,
                *type_contains {
                    parent_type_id,
                    child_type_id: terminal_type_id,
                    kind,
                    position @ 'NOW'
                },
                depth = previous_depth + 1

            ?[owner_id, root_type_id, terminal_type_id, target_id, relation_kind, depth] :=
                target_id = $target_id,
                reachable[owner_id, root_type_id, terminal_type_id, depth],
                *type_relation {
                    source_id: terminal_type_id,
                    target_id,
                    relation_kind @ 'NOW'
                }

            :sort depth, owner_id, root_type_id, terminal_type_id
            "#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows
            .iter()
            .map(|row| {
                Ok(TypeTargetPath {
                    owner_id: to_uuid(&row[0])?,
                    root_type_id: to_uuid(&row[1])?,
                    terminal_type_id: to_uuid(&row[2])?,
                    target_id: to_uuid(&row[3])?,
                    relation_kind: TypeRelationKind::from_str(&to_string(&row[4])?)?,
                    depth: required_index(&row[5])?,
                })
            })
            .collect()
    }

    /// Returns other code-graph owners that are type-related to `owner_id`
    /// because both owners reach the same resolved type-definition target.
    ///
    /// This query is the DB-facing graphRAG primitive layered on top of
    /// `type_use`, `type_contains`, and `type_relation`: it moves from one code
    /// owner to other code owners through semantic type identity while retaining
    /// enough depth information to rank exact/root matches ahead of nested
    /// terminal matches.
    pub fn type_related_owners(&self, owner_id: Uuid) -> Result<Vec<TypeRelatedOwner>, DbError> {
        const MAX_TYPE_DEPTH: u32 = 32;

        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );
        params.insert(
            "max_depth".to_string(),
            DataValue::Num(Num::Int(i64::from(MAX_TYPE_DEPTH))),
        );

        let rows = self.run_script(
            r#"
            origin_roots[root_type_id] :=
                owner_id = $owner_id,
                *type_use { owner_id, root_type_id, role, slot_index @ 'NOW' }

            origin_reachable[terminal_type_id, depth] :=
                origin_roots[root_type_id],
                terminal_type_id = root_type_id,
                depth = 0

            origin_reachable[terminal_type_id, depth] :=
                origin_reachable[parent_type_id, previous_depth],
                previous_depth < $max_depth,
                *type_contains {
                    parent_type_id,
                    child_type_id: terminal_type_id,
                    kind,
                    position @ 'NOW'
                },
                depth = previous_depth + 1

            origin_targets[target_id, relation_kind, origin_depth] :=
                origin_reachable[source_id, origin_depth],
                *type_relation {
                    source_id,
                    target_id,
                    relation_kind @ 'NOW'
                }

            related_roots[related_owner_id, root_type_id] :=
                *type_use {
                    owner_id: related_owner_id,
                    root_type_id,
                    role,
                    slot_index @ 'NOW'
                },
                related_owner_id != $owner_id

            related_reachable[related_owner_id, terminal_type_id, depth] :=
                related_roots[related_owner_id, root_type_id],
                terminal_type_id = root_type_id,
                depth = 0

            related_reachable[related_owner_id, terminal_type_id, depth] :=
                related_reachable[related_owner_id, parent_type_id, previous_depth],
                previous_depth < $max_depth,
                *type_contains {
                    parent_type_id,
                    child_type_id: terminal_type_id,
                    kind,
                    position @ 'NOW'
                },
                depth = previous_depth + 1

            related_targets[related_owner_id, target_id, relation_kind, related_depth] :=
                related_reachable[related_owner_id, source_id, related_depth],
                *type_relation {
                    source_id,
                    target_id,
                    relation_kind @ 'NOW'
                }

            ?[
                origin_owner_id,
                related_owner_id,
                shared_target_id,
                relation_kind,
                origin_depth,
                related_depth,
                distance
            ] :=
                origin_owner_id = $owner_id,
                origin_targets[shared_target_id, relation_kind, origin_depth],
                related_targets[
                    related_owner_id,
                    shared_target_id,
                    relation_kind,
                    related_depth
                ],
                distance = origin_depth + related_depth

            :sort distance, related_depth, related_owner_id
            "#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows
            .iter()
            .map(|row| {
                Ok(TypeRelatedOwner {
                    origin_owner_id: to_uuid(&row[0])?,
                    related_owner_id: to_uuid(&row[1])?,
                    shared_target_id: to_uuid(&row[2])?,
                    relation_kind: TypeRelationKind::from_str(&to_string(&row[3])?)?,
                    origin_depth: required_index(&row[4])?,
                    related_depth: required_index(&row[5])?,
                    distance: required_index(&row[6])?,
                })
            })
            .collect()
    }
}

fn optional_index(value: &DataValue) -> Result<Option<u32>, DbError> {
    let index = raw_index(value)?;
    if index < 0 {
        Ok(None)
    } else {
        Ok(Some(u32::try_from(index).map_err(|err| {
            DbError::Cozo(format!("invalid non-negative index {index}: {err}"))
        })?))
    }
}

fn required_index(value: &DataValue) -> Result<u32, DbError> {
    let index = raw_index(value)?;
    u32::try_from(index).map_err(|err| DbError::Cozo(format!("invalid index {index}: {err}")))
}

fn raw_index(value: &DataValue) -> Result<i64, DbError> {
    match value {
        DataValue::Num(Num::Int(index)) => Ok(*index),
        other => Err(DbError::Cozo(format!(
            "expected Int index, found {other:?}"
        ))),
    }
}
