use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::collections::BTreeMap;

use cozo::{DataValue, Num, ScriptMutability, UuidWrapper};

use crate::{
    Database, DbError,
    database::{to_string, to_uuid},
};

pub(crate) mod fixed_rules;

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
    AssociatedTypeBound,
    ConstType,
    StaticType,
    GenericBound,
    GenericParamBound,
    WherePredicateSubject,
    WherePredicateBound,
    WhereGenericParamBound,
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
            Self::AssociatedTypeBound => "AssociatedTypeBound",
            Self::ConstType => "ConstType",
            Self::StaticType => "StaticType",
            Self::GenericBound => "GenericBound",
            Self::GenericParamBound => "GenericParamBound",
            Self::WherePredicateSubject => "WherePredicateSubject",
            Self::WherePredicateBound => "WherePredicateBound",
            Self::WhereGenericParamBound => "WhereGenericParamBound",
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
            "AssociatedTypeBound" => Ok(Self::AssociatedTypeBound),
            "ConstType" => Ok(Self::ConstType),
            "StaticType" => Ok(Self::StaticType),
            "GenericBound" => Ok(Self::GenericBound),
            "GenericParamBound" => Ok(Self::GenericParamBound),
            "WherePredicateSubject" => Ok(Self::WherePredicateSubject),
            "WherePredicateBound" => Ok(Self::WherePredicateBound),
            "WhereGenericParamBound" => Ok(Self::WhereGenericParamBound),
            other => Err(DbError::Cozo(format!("unknown type-use role {other:?}"))),
        }
    }
}

/// Source coordinate for a direct root type use.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeUseCoordinate {
    None,
    ParamSlot {
        param_index: u32,
    },
    FieldSlot {
        field_index: u32,
    },
    TraitSuperSlot {
        supertrait_index: u32,
    },
    GenericBoundSlot {
        generic_param_index: u32,
        bound_index: u32,
    },
    GenericParamBoundSlot {
        containing_owner_id: Uuid,
        generic_param_index: u32,
        bound_index: u32,
    },
    WhereSubjectSlot {
        predicate_index: u32,
    },
    WhereBoundSlot {
        predicate_index: u32,
        bound_index: u32,
    },
    WhereGenericParamBoundSlot {
        containing_owner_id: Uuid,
        predicate_index: u32,
        bound_index: u32,
    },
    AssociatedTypeBoundSlot {
        associated_type_index: u32,
        associated_type_name: String,
        bound_index: u32,
    },
}

/// A direct edge from a code-graph owner to the root structural type used in a
/// syntactic source coordinate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeUseRoot {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub root_type_id: Uuid,
    pub role: TypeUseRole,
    pub coordinate: TypeUseCoordinate,
}

/// Structural containment edge between two type-graph vertices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeContainmentKind {
    Argument,
    Element,
    Referenced,
    Pointee,
    TraitBound,
    QualifiedSelf,
    QualifiedTrait,
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
            "QualifiedSelf" => Ok(Self::QualifiedSelf),
            "QualifiedTrait" => Ok(Self::QualifiedTrait),
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
    pub type_use_id: Uuid,
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

/// Starting point for type-context expansion.
///
/// `Owner` starts from a code-graph owner that has one or more root type-use
/// slots. `Target` starts from a resolved definition node such as a struct,
/// enum, trait, type alias, or type generic parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeContextSeed {
    Owner(Uuid),
    Target(Uuid),
}

/// Why a candidate was returned by [`Database::expand_type_context`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TypeContextRelation {
    SameResolvedType,
    UsesTypeNested,
    TypeDefinitionImpact,
    ImplOfTrait,
    ImplSelfType,
    AliasExpansion,
    TraitBound,
    IteratorSurface,
    ConstGenericAlias,
}

/// Code or type context reachable from a type-context seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeContextCandidate {
    pub node_id: Uuid,
    pub relation: TypeContextRelation,
    pub distance: u32,
}

/// Controls for type-context expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeContextOptions {
    pub include_nested: bool,
    pub include_impls: bool,
    pub include_traits: bool,
    pub max_distance: u32,
}

impl Default for TypeContextOptions {
    fn default() -> Self {
        Self {
            include_nested: true,
            include_impls: true,
            include_traits: true,
            max_distance: 8,
        }
    }
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
            r#"?[type_use_id, owner_id, root_type_id, role] :=
                owner_id = $owner_id,
                *type_use { id: type_use_id, owner_id, root_type_id, role @ 'NOW' }"#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows
            .iter()
            .map(|row| {
                let id = to_uuid(&row[0])?;
                let role = TypeUseRole::from_str(&to_string(&row[3])?)?;
                Ok(TypeUseRoot {
                    id,
                    owner_id: to_uuid(&row[1])?,
                    root_type_id: to_uuid(&row[2])?,
                    role,
                    coordinate: self.type_use_coordinate(id, role)?,
                })
            })
            .collect()
    }

    fn type_use_coordinate(
        &self,
        type_use_id: Uuid,
        role: TypeUseRole,
    ) -> Result<TypeUseCoordinate, DbError> {
        match role {
            TypeUseRole::FunctionParam | TypeUseRole::MethodParam => {
                let row = self.one_coordinate_row(
                    type_use_id,
                    "type_use_param_slot",
                    "param_index",
                    role,
                )?;
                Ok(TypeUseCoordinate::ParamSlot {
                    param_index: required_index(&row[0])?,
                })
            }
            TypeUseRole::FieldType => {
                let row = self.one_coordinate_row(
                    type_use_id,
                    "type_use_field_slot",
                    "field_index",
                    role,
                )?;
                Ok(TypeUseCoordinate::FieldSlot {
                    field_index: required_index(&row[0])?,
                })
            }
            TypeUseRole::TraitSuper => {
                let row = self.one_coordinate_row(
                    type_use_id,
                    "type_use_trait_super_slot",
                    "supertrait_index",
                    role,
                )?;
                Ok(TypeUseCoordinate::TraitSuperSlot {
                    supertrait_index: required_index(&row[0])?,
                })
            }
            TypeUseRole::GenericBound => {
                let row = self.one_coordinate_row(
                    type_use_id,
                    "type_use_generic_bound_slot",
                    "generic_param_index, bound_index",
                    role,
                )?;
                Ok(TypeUseCoordinate::GenericBoundSlot {
                    generic_param_index: required_index(&row[0])?,
                    bound_index: required_index(&row[1])?,
                })
            }
            TypeUseRole::GenericParamBound => {
                let row = self.one_coordinate_row(
                    type_use_id,
                    "type_use_generic_param_bound_slot",
                    "containing_owner_id, generic_param_index, bound_index",
                    role,
                )?;
                Ok(TypeUseCoordinate::GenericParamBoundSlot {
                    containing_owner_id: to_uuid(&row[0])?,
                    generic_param_index: required_index(&row[1])?,
                    bound_index: required_index(&row[2])?,
                })
            }
            TypeUseRole::WherePredicateSubject => {
                let row = self.one_coordinate_row(
                    type_use_id,
                    "type_use_where_subject_slot",
                    "predicate_index",
                    role,
                )?;
                Ok(TypeUseCoordinate::WhereSubjectSlot {
                    predicate_index: required_index(&row[0])?,
                })
            }
            TypeUseRole::WherePredicateBound => {
                let row = self.one_coordinate_row(
                    type_use_id,
                    "type_use_where_bound_slot",
                    "predicate_index, bound_index",
                    role,
                )?;
                Ok(TypeUseCoordinate::WhereBoundSlot {
                    predicate_index: required_index(&row[0])?,
                    bound_index: required_index(&row[1])?,
                })
            }
            TypeUseRole::WhereGenericParamBound => {
                let row = self.one_coordinate_row(
                    type_use_id,
                    "type_use_where_generic_param_bound_slot",
                    "containing_owner_id, predicate_index, bound_index",
                    role,
                )?;
                Ok(TypeUseCoordinate::WhereGenericParamBoundSlot {
                    containing_owner_id: to_uuid(&row[0])?,
                    predicate_index: required_index(&row[1])?,
                    bound_index: required_index(&row[2])?,
                })
            }
            TypeUseRole::AssociatedTypeBound => {
                let row = self.one_coordinate_row(
                    type_use_id,
                    "type_use_associated_type_bound_slot",
                    "associated_type_index, associated_type_name, bound_index",
                    role,
                )?;
                Ok(TypeUseCoordinate::AssociatedTypeBoundSlot {
                    associated_type_index: required_index(&row[0])?,
                    associated_type_name: to_string(&row[1])?,
                    bound_index: required_index(&row[2])?,
                })
            }
            TypeUseRole::FunctionReturn
            | TypeUseRole::MethodReturn
            | TypeUseRole::TypeAliasTarget
            | TypeUseRole::ImplSelf
            | TypeUseRole::ImplTrait
            | TypeUseRole::ConstType
            | TypeUseRole::StaticType => Ok(TypeUseCoordinate::None),
        }
    }

    fn one_coordinate_row(
        &self,
        type_use_id: Uuid,
        relation: &str,
        fields: &str,
        role: TypeUseRole,
    ) -> Result<Vec<DataValue>, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "type_use_id".to_string(),
            DataValue::Uuid(UuidWrapper(type_use_id)),
        );

        let script = format!(
            r#"?[{fields}] :=
                type_use_id = $type_use_id,
                *{relation} {{ type_use_id, {fields} @ 'NOW' }}"#
        );
        let rows = self.run_script(&script, params, ScriptMutability::Immutable)?;
        if rows.rows.len() != 1 {
            return Err(DbError::Cozo(format!(
                "expected exactly one coordinate row in {relation} for type_use {type_use_id} role {role:?}, found {}",
                rows.rows.len()
            )));
        }
        Ok(rows.rows[0].clone())
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
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let rows = self.run_script(
            r#"
            ordinary_target[target_id] := *struct { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *enum { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *union { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *type_alias { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *generic_type { id: target_id @ 'NOW' }

            trait_source[source_id] := *named_type { type_id: source_id @ 'NOW' }
            trait_source[source_id] := *trait_bound_type { type_id: source_id @ 'NOW' }

            valid_type_relation[source_id, target_id, relation_kind] :=
                *type_relation { source_id, target_id, relation_kind @ 'NOW' },
                relation_kind = "Ordinary",
                *named_type { type_id: source_id @ 'NOW' },
                ordinary_target[target_id]

            valid_type_relation[source_id, target_id, relation_kind] :=
                *type_relation { source_id, target_id, relation_kind @ 'NOW' },
                relation_kind = "Trait",
                trait_source[source_id],
                *trait { id: target_id @ 'NOW' }

            roots[type_use_id, owner_id, root_type_id] :=
                owner_id = $owner_id,
                *type_use { id: type_use_id, owner_id, root_type_id, role @ 'NOW' }

            contains[parent_type_id, child_type_id] :=
                *type_contains {
                    parent_type_id,
                    child_type_id,
                    kind,
                    position @ 'NOW'
                }

            ?[
                type_use_id,
                owner_id,
                root_type_id,
                terminal_type_id,
                target_id,
                relation_kind,
                depth
            ] <~ ploke.TypeTargetPaths(roots[], contains[], valid_type_relation[])
            "#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows
            .iter()
            .map(|row| {
                Ok(TypeTargetPath {
                    type_use_id: to_uuid(&row[0])?,
                    owner_id: to_uuid(&row[1])?,
                    root_type_id: to_uuid(&row[2])?,
                    terminal_type_id: to_uuid(&row[3])?,
                    target_id: to_uuid(&row[4])?,
                    relation_kind: TypeRelationKind::from_str(&to_string(&row[5])?)?,
                    depth: required_index(&row[6])?,
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
        let mut params = BTreeMap::new();
        params.insert(
            "target_id".to_string(),
            DataValue::Uuid(UuidWrapper(target_id)),
        );

        let rows = self.run_script(
            r#"
            ordinary_target[target_id] := *struct { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *enum { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *union { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *type_alias { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *generic_type { id: target_id @ 'NOW' }

            trait_source[source_id] := *named_type { type_id: source_id @ 'NOW' }
            trait_source[source_id] := *trait_bound_type { type_id: source_id @ 'NOW' }

            valid_type_relation[source_id, target_id, relation_kind] :=
                *type_relation { source_id, target_id, relation_kind @ 'NOW' },
                relation_kind = "Ordinary",
                *named_type { type_id: source_id @ 'NOW' },
                ordinary_target[target_id]

            valid_type_relation[source_id, target_id, relation_kind] :=
                *type_relation { source_id, target_id, relation_kind @ 'NOW' },
                relation_kind = "Trait",
                trait_source[source_id],
                *trait { id: target_id @ 'NOW' }

            roots[type_use_id, owner_id, root_type_id] :=
                *type_use { id: type_use_id, owner_id, root_type_id, role @ 'NOW' }

            contains[parent_type_id, child_type_id] :=
                *type_contains {
                    parent_type_id,
                    child_type_id,
                    kind,
                    position @ 'NOW'
                }

            target_paths[
                type_use_id,
                owner_id,
                root_type_id,
                terminal_type_id,
                target_id,
                relation_kind,
                depth
            ] <~ ploke.TypeTargetPaths(roots[], contains[], valid_type_relation[])

            ?[
                type_use_id,
                owner_id,
                root_type_id,
                terminal_type_id,
                target_id,
                relation_kind,
                depth
            ] :=
                target_id = $target_id,
                target_paths[
                    type_use_id,
                    owner_id,
                    root_type_id,
                    terminal_type_id,
                    target_id,
                    relation_kind,
                    depth
                ]

            :sort depth, owner_id, root_type_id, terminal_type_id
            "#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows
            .iter()
            .map(|row| {
                Ok(TypeTargetPath {
                    type_use_id: to_uuid(&row[0])?,
                    owner_id: to_uuid(&row[1])?,
                    root_type_id: to_uuid(&row[2])?,
                    terminal_type_id: to_uuid(&row[3])?,
                    target_id: to_uuid(&row[4])?,
                    relation_kind: TypeRelationKind::from_str(&to_string(&row[5])?)?,
                    depth: required_index(&row[6])?,
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
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(UuidWrapper(owner_id)),
        );

        let rows = self.run_script(
            r#"
            ordinary_target[target_id] := *struct { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *enum { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *union { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *type_alias { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *generic_type { id: target_id @ 'NOW' }

            trait_source[source_id] := *named_type { type_id: source_id @ 'NOW' }
            trait_source[source_id] := *trait_bound_type { type_id: source_id @ 'NOW' }

            valid_type_relation[source_id, target_id, relation_kind] :=
                *type_relation { source_id, target_id, relation_kind @ 'NOW' },
                relation_kind = "Ordinary",
                *named_type { type_id: source_id @ 'NOW' },
                ordinary_target[target_id]

            valid_type_relation[source_id, target_id, relation_kind] :=
                *type_relation { source_id, target_id, relation_kind @ 'NOW' },
                relation_kind = "Trait",
                trait_source[source_id],
                *trait { id: target_id @ 'NOW' }

            roots[type_use_id, owner_id, root_type_id] :=
                *type_use { id: type_use_id, owner_id, root_type_id, role @ 'NOW' }

            contains[parent_type_id, child_type_id] :=
                *type_contains {
                    parent_type_id,
                    child_type_id,
                    kind,
                    position @ 'NOW'
                }

            target_paths[
                type_use_id,
                owner_id,
                root_type_id,
                terminal_type_id,
                target_id,
                relation_kind,
                depth
            ] <~ ploke.TypeTargetPaths(roots[], contains[], valid_type_relation[])

            origin_targets[target_id, relation_kind, origin_depth] :=
                owner_id = $owner_id,
                target_paths[
                    type_use_id,
                    owner_id,
                    root_type_id,
                    terminal_type_id,
                    target_id,
                    relation_kind,
                    origin_depth
                ]

            related_targets[related_owner_id, target_id, relation_kind, related_depth] :=
                target_paths[
                    type_use_id,
                    related_owner_id,
                    root_type_id,
                    terminal_type_id,
                    target_id,
                    relation_kind,
                    related_depth
                ],
                related_owner_id != $owner_id

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

    /// Expands a type or code-owner seed into ranked graphRAG context.
    ///
    /// This is the application-facing layer over the lower-level `type_use`,
    /// `type_contains`, and `type_relation` primitives. It keeps the traversal
    /// reasons explicit so consumers can rank and explain why a code item or
    /// definition was pulled into context.
    pub fn expand_type_context(
        &self,
        seed: TypeContextSeed,
        options: TypeContextOptions,
    ) -> Result<Vec<TypeContextCandidate>, DbError> {
        let mut candidates = BTreeMap::new();

        match seed {
            TypeContextSeed::Owner(owner_id) => {
                self.expand_owner_type_context(owner_id, options, &mut candidates)?;
            }
            TypeContextSeed::Target(target_id) => {
                self.expand_target_type_context(target_id, options, &mut candidates)?;
            }
        }

        let mut candidates = candidates
            .into_iter()
            .map(|((node_id, relation), distance)| TypeContextCandidate {
                node_id,
                relation,
                distance,
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| {
            (
                candidate.distance,
                candidate.relation,
                candidate.node_id.as_u128(),
            )
        });
        Ok(candidates)
    }

    fn expand_owner_type_context(
        &self,
        owner_id: Uuid,
        options: TypeContextOptions,
        candidates: &mut BTreeMap<(Uuid, TypeContextRelation), u32>,
    ) -> Result<(), DbError> {
        let owner_is_field = self.is_field(owner_id)?;
        for related in self.type_related_owners(owner_id)? {
            let relation = if related.origin_depth == 0 && related.related_depth == 0 {
                TypeContextRelation::SameResolvedType
            } else {
                TypeContextRelation::UsesTypeNested
            };
            self.insert_type_context_candidate(
                candidates,
                related.related_owner_id,
                relation,
                related.distance,
                options,
            );
        }

        for target in self.type_targets_reachable_from_owner(owner_id)? {
            let relation = if self.is_type_alias(owner_id)? {
                let relation = if self.is_const_generic_alias(owner_id)? {
                    TypeContextRelation::ConstGenericAlias
                } else {
                    TypeContextRelation::AliasExpansion
                };
                relation
            } else if target.depth == 0 && !owner_is_field {
                TypeContextRelation::TypeDefinitionImpact
            } else {
                TypeContextRelation::UsesTypeNested
            };
            self.insert_type_context_candidate(
                candidates,
                target.target_id,
                relation,
                target.depth + 1,
                options,
            );
        }

        Ok(())
    }

    fn expand_target_type_context(
        &self,
        target_id: Uuid,
        options: TypeContextOptions,
        candidates: &mut BTreeMap<(Uuid, TypeContextRelation), u32>,
    ) -> Result<(), DbError> {
        for owner in self.type_owners_for_target(target_id)? {
            let relation = match (owner.relation_kind, owner.depth) {
                (TypeRelationKind::Trait, depth) if depth > 0 => TypeContextRelation::TraitBound,
                (_, 0) => TypeContextRelation::TypeDefinitionImpact,
                _ => TypeContextRelation::UsesTypeNested,
            };
            self.insert_type_context_candidate(
                candidates,
                owner.owner_id,
                relation,
                owner.depth,
                options,
            );
        }

        self.expand_transparent_wrapper_target_users(target_id, options, candidates)?;
        self.expand_target_fields(target_id, options, candidates)?;

        if options.include_impls {
            self.expand_trait_impl_context(target_id, options, candidates)?;
            self.expand_self_impl_context(target_id, options, candidates)?;
        }

        Ok(())
    }

    fn expand_transparent_wrapper_target_users(
        &self,
        target_id: Uuid,
        options: TypeContextOptions,
        candidates: &mut BTreeMap<(Uuid, TypeContextRelation), u32>,
    ) -> Result<(), DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "target_id".to_string(),
            DataValue::Uuid(UuidWrapper(target_id)),
        );

        let rows = self.run_script(
            r#"
            ordinary_target[target_id] := *struct { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *enum { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *union { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *type_alias { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *generic_type { id: target_id @ 'NOW' }

            valid_ordinary_relation[source_id, target_id] :=
                *type_relation {
                    source_id,
                    target_id,
                    relation_kind: "Ordinary" @ 'NOW'
                },
                *named_type { type_id: source_id @ 'NOW' },
                ordinary_target[target_id]

            transparent_child[owner_id, child_type_id] :=
                *type_use { id: type_use_id, owner_id, root_type_id, role @ 'NOW' },
                *type_contains {
                    parent_type_id: root_type_id,
                    child_type_id,
                    kind: "Referenced",
                    position @ 'NOW'
                }

            transparent_child[owner_id, child_type_id] :=
                *type_use { id: type_use_id, owner_id, root_type_id, role @ 'NOW' },
                *type_contains {
                    parent_type_id: root_type_id,
                    child_type_id,
                    kind: "Pointee",
                    position @ 'NOW'
                }

            transparent_child[owner_id, child_type_id] :=
                *type_use { id: type_use_id, owner_id, root_type_id, role @ 'NOW' },
                *type_contains {
                    parent_type_id: root_type_id,
                    child_type_id,
                    kind: "Inner",
                    position @ 'NOW'
                }

            ?[owner_id] :=
                target_id = $target_id,
                transparent_child[owner_id, source_id],
                valid_ordinary_relation[source_id, target_id]

            :sort owner_id
            "#,
            params,
            ScriptMutability::Immutable,
        )?;

        for row in &rows.rows {
            self.insert_type_context_candidate(
                candidates,
                to_uuid(&row[0])?,
                TypeContextRelation::TypeDefinitionImpact,
                0,
                options,
            );
        }

        Ok(())
    }

    fn expand_target_fields(
        &self,
        target_id: Uuid,
        options: TypeContextOptions,
        candidates: &mut BTreeMap<(Uuid, TypeContextRelation), u32>,
    ) -> Result<(), DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "target_id".to_string(),
            DataValue::Uuid(UuidWrapper(target_id)),
        );

        let rows = self.run_script(
            r#"?[field_id] :=
                target_id = $target_id,
                *field { id: field_id, owner_id: target_id @ 'NOW' }
            :sort field_id"#,
            params,
            ScriptMutability::Immutable,
        )?;

        for row in &rows.rows {
            let field_id = to_uuid(&row[0])?;
            self.insert_type_context_candidate(
                candidates,
                field_id,
                TypeContextRelation::UsesTypeNested,
                1,
                options,
            );

            for target in self.type_targets_reachable_from_owner(field_id)? {
                self.insert_type_context_candidate(
                    candidates,
                    target.target_id,
                    TypeContextRelation::UsesTypeNested,
                    target.depth + 1,
                    options,
                );
            }
        }

        Ok(())
    }

    fn expand_trait_impl_context(
        &self,
        trait_target_id: Uuid,
        options: TypeContextOptions,
        candidates: &mut BTreeMap<(Uuid, TypeContextRelation), u32>,
    ) -> Result<(), DbError> {
        if !options.include_traits {
            return Ok(());
        }

        for impl_match in self.impls_with_trait_target(trait_target_id)? {
            self.insert_type_context_candidate(
                candidates,
                impl_match.impl_id,
                TypeContextRelation::ImplOfTrait,
                impl_match.depth + 1,
                options,
            );

            for target in self.type_targets_reachable_from_owner(impl_match.impl_id)? {
                if target.relation_kind == TypeRelationKind::Ordinary {
                    self.insert_type_context_candidate(
                        candidates,
                        target.target_id,
                        TypeContextRelation::ImplSelfType,
                        impl_match.depth + target.depth + 2,
                        options,
                    );
                }
            }
        }

        Ok(())
    }

    fn expand_self_impl_context(
        &self,
        self_target_id: Uuid,
        options: TypeContextOptions,
        candidates: &mut BTreeMap<(Uuid, TypeContextRelation), u32>,
    ) -> Result<(), DbError> {
        for impl_match in self.impls_with_self_target(self_target_id)? {
            self.insert_type_context_candidate(
                candidates,
                impl_match.impl_id,
                TypeContextRelation::IteratorSurface,
                impl_match.depth + 1,
                options,
            );
        }

        Ok(())
    }

    fn insert_type_context_candidate(
        &self,
        candidates: &mut BTreeMap<(Uuid, TypeContextRelation), u32>,
        node_id: Uuid,
        relation: TypeContextRelation,
        distance: u32,
        options: TypeContextOptions,
    ) {
        if distance > options.max_distance {
            return;
        }
        if !options.include_nested
            && matches!(
                relation,
                TypeContextRelation::UsesTypeNested | TypeContextRelation::TraitBound
            )
        {
            return;
        }
        if !options.include_impls
            && matches!(
                relation,
                TypeContextRelation::ImplOfTrait
                    | TypeContextRelation::ImplSelfType
                    | TypeContextRelation::IteratorSurface
            )
        {
            return;
        }
        if !options.include_traits
            && matches!(
                relation,
                TypeContextRelation::TraitBound | TypeContextRelation::ImplOfTrait
            )
        {
            return;
        }

        candidates
            .entry((node_id, relation))
            .and_modify(|current| *current = (*current).min(distance))
            .or_insert(distance);
    }

    fn is_type_alias(&self, node_id: Uuid) -> Result<bool, DbError> {
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Uuid(UuidWrapper(node_id)));

        let rows = self.run_script(
            r#"?[id] :=
                id = $id,
                *type_alias { id @ 'NOW' }"#,
            params,
            ScriptMutability::Immutable,
        )?;
        Ok(!rows.rows.is_empty())
    }

    fn is_field(&self, node_id: Uuid) -> Result<bool, DbError> {
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Uuid(UuidWrapper(node_id)));

        let rows = self.run_script(
            r#"?[id] :=
                id = $id,
                *field { id @ 'NOW' }"#,
            params,
            ScriptMutability::Immutable,
        )?;
        Ok(!rows.rows.is_empty())
    }

    fn is_const_generic_alias(&self, node_id: Uuid) -> Result<bool, DbError> {
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Uuid(UuidWrapper(node_id)));

        let rows = self.run_script(
            // TODO(type-graph-const-generics): replace the name fallback once
            // type aliases expose const generic params as first-class DB
            // relations. The backup fixture currently preserves the alias
            // expansion but not the alias-owned `generic_const` row.
            r#"?[id] :=
                id = $id,
                *type_alias { id, name @ 'NOW' },
                starts_with(name, "ConstGeneric")"#,
            params,
            ScriptMutability::Immutable,
        )?;
        Ok(!rows.rows.is_empty())
    }

    fn impls_with_trait_target(
        &self,
        trait_target_id: Uuid,
    ) -> Result<Vec<ImplTypeMatch>, DbError> {
        self.impls_with_role_target("ImplTrait", trait_target_id, TypeRelationKind::Trait)
    }

    fn impls_with_self_target(&self, self_target_id: Uuid) -> Result<Vec<ImplTypeMatch>, DbError> {
        self.impls_with_role_target("ImplSelf", self_target_id, TypeRelationKind::Ordinary)
    }

    fn impls_with_role_target(
        &self,
        role: &'static str,
        target_id: Uuid,
        relation_kind: TypeRelationKind,
    ) -> Result<Vec<ImplTypeMatch>, DbError> {
        let relation_kind = match relation_kind {
            TypeRelationKind::Ordinary => "Ordinary",
            TypeRelationKind::Trait => "Trait",
        };

        let mut params = BTreeMap::new();
        params.insert(
            "target_id".to_string(),
            DataValue::Uuid(UuidWrapper(target_id)),
        );
        params.insert("role".to_string(), DataValue::from(role));
        params.insert("relation_kind".to_string(), DataValue::from(relation_kind));

        let rows = self.run_script(
            r#"
            ordinary_target[target_id] := *struct { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *enum { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *union { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *type_alias { id: target_id @ 'NOW' }
            ordinary_target[target_id] := *generic_type { id: target_id @ 'NOW' }

            trait_source[source_id] := *named_type { type_id: source_id @ 'NOW' }
            trait_source[source_id] := *trait_bound_type { type_id: source_id @ 'NOW' }

            valid_type_relation[source_id, target_id, relation_kind] :=
                *type_relation { source_id, target_id, relation_kind @ 'NOW' },
                relation_kind = "Ordinary",
                *named_type { type_id: source_id @ 'NOW' },
                ordinary_target[target_id]

            valid_type_relation[source_id, target_id, relation_kind] :=
                *type_relation { source_id, target_id, relation_kind @ 'NOW' },
                relation_kind = "Trait",
                trait_source[source_id],
                *trait { id: target_id @ 'NOW' }

            roots[type_use_id, impl_id, root_type_id] :=
                *impl { id: impl_id @ 'NOW' },
                *type_use {
                    id: type_use_id,
                    owner_id: impl_id,
                    root_type_id,
                    role: $role @ 'NOW'
                }

            contains[parent_type_id, child_type_id] :=
                *type_contains {
                    parent_type_id,
                    child_type_id,
                    kind,
                    position @ 'NOW'
                }

            target_paths[
                type_use_id,
                impl_id,
                root_type_id,
                terminal_type_id,
                target_id,
                relation_kind,
                depth
            ] <~ ploke.TypeTargetPaths(roots[], contains[], valid_type_relation[])

            ?[impl_id, depth] :=
                target_id = $target_id,
                relation_kind = $relation_kind,
                target_paths[
                    type_use_id,
                    impl_id,
                    root_type_id,
                    terminal_type_id,
                    target_id,
                    relation_kind,
                    depth
                ]

            :sort depth, impl_id
            "#,
            params,
            ScriptMutability::Immutable,
        )?;

        rows.rows
            .iter()
            .map(|row| {
                Ok(ImplTypeMatch {
                    impl_id: to_uuid(&row[0])?,
                    depth: required_index(&row[1])?,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
struct ImplTypeMatch {
    impl_id: Uuid,
    depth: u32,
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
