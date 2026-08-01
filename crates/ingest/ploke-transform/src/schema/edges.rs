//! Edge schema definitions for the Cozo database
//!
//! This module contains schemas for edge relations in the graph, with a two-tiered approach:
//! 1. Syntactic relations - directly representing AST relationships
//! 2. Semantic relations - higher-level logical relationships derived from syntactic ones

use crate::define_schema;

use super::*;
use cozo::{Db, MemStorage};
use itertools::Itertools;
use std::collections::BTreeMap;
use syn_parser::parser::nodes::ToCozoUuid;
use syn_parser::parser::relations::SyntacticRelation;
use syn_parser::parser::relations::TypeRelation;
use syn_parser::resolve::Colorize;
use syn_parser::utils::{LogStyle, LogStyleDebug};

define_schema!(SyntacticRelationSchema {
    "syntax_edge",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    source_kind: "String",
    target_kind: "String"
});

define_schema!(TypeRelationSchema {
    "type_relation",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    source_kind: "String",
    target_kind: "String"
});

pub struct TypeUseSchema;

impl TypeUseSchema {
    pub const RELATION: &'static str = "type_use";

    pub fn create_and_insert_schema(db: &Db<MemStorage>) -> Result<(), TransformError> {
        db.run_script(
            r#":create type_use {
                id: Uuid,
                at: Validity =>
                owner_id: Uuid,
                root_type_id: Uuid,
                role: String
            }"#,
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )?;
        create_type_use_coordinate_schema(db)?;
        Ok(())
    }

    pub fn insert_relation(
        db: &Db<MemStorage>,
        id: cozo::DataValue,
        owner_id: cozo::DataValue,
        root_type_id: cozo::DataValue,
        role: &'static str,
    ) -> Result<(), TransformError> {
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), id);
        params.insert("owner_id".to_string(), owner_id);
        params.insert("root_type_id".to_string(), root_type_id);
        params.insert("role".to_string(), cozo::DataValue::from(role));

        db.run_script(
            r#"?[id, at, owner_id, root_type_id, role] :=
                id = $id,
                owner_id = $owner_id,
                root_type_id = $root_type_id,
                role = $role,
                at = 'ASSERT'
            :put type_use { id, at => owner_id, root_type_id, role }"#,
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }

    pub fn insert_param_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        param_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_param_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("param_index", int(param_index)),
            ],
            "param_index",
        )
    }

    pub fn insert_field_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        field_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_field_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("field_index", int(field_index)),
            ],
            "field_index",
        )
    }

    pub fn insert_trait_super_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        supertrait_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_trait_super_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("supertrait_index", int(supertrait_index)),
            ],
            "supertrait_index",
        )
    }

    pub fn insert_generic_bound_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        generic_param_index: usize,
        bound_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_generic_bound_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("generic_param_index", int(generic_param_index)),
                ("bound_index", int(bound_index)),
            ],
            "generic_param_index, bound_index",
        )
    }

    pub fn insert_generic_param_bound_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        containing_owner_id: cozo::DataValue,
        generic_param_index: usize,
        bound_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_generic_param_bound_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("containing_owner_id", containing_owner_id),
                ("generic_param_index", int(generic_param_index)),
                ("bound_index", int(bound_index)),
            ],
            "containing_owner_id, generic_param_index, bound_index",
        )
    }

    pub fn insert_where_subject_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        predicate_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_where_subject_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("predicate_index", int(predicate_index)),
            ],
            "predicate_index",
        )
    }

    pub fn insert_where_bound_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        predicate_index: usize,
        bound_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_where_bound_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("predicate_index", int(predicate_index)),
                ("bound_index", int(bound_index)),
            ],
            "predicate_index, bound_index",
        )
    }

    pub fn insert_where_generic_param_bound_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        containing_owner_id: cozo::DataValue,
        predicate_index: usize,
        bound_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_where_generic_param_bound_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("containing_owner_id", containing_owner_id),
                ("predicate_index", int(predicate_index)),
                ("bound_index", int(bound_index)),
            ],
            "containing_owner_id, predicate_index, bound_index",
        )
    }

    pub fn insert_associated_type_bound_slot(
        db: &Db<MemStorage>,
        type_use_id: cozo::DataValue,
        associated_type_index: usize,
        associated_type_name: &str,
        bound_index: usize,
    ) -> Result<(), TransformError> {
        insert_coordinate_relation(
            db,
            "type_use_associated_type_bound_slot",
            &["type_use_id", "at"],
            &[
                ("type_use_id", type_use_id),
                ("associated_type_index", int(associated_type_index)),
                (
                    "associated_type_name",
                    cozo::DataValue::from(associated_type_name),
                ),
                ("bound_index", int(bound_index)),
            ],
            "associated_type_index, associated_type_name, bound_index",
        )
    }
}

fn create_type_use_coordinate_schema(db: &Db<MemStorage>) -> Result<(), TransformError> {
    for script in [
        r#":create type_use_param_slot {
            type_use_id: Uuid,
            at: Validity =>
            param_index: Int
        }"#,
        r#":create type_use_field_slot {
            type_use_id: Uuid,
            at: Validity =>
            field_index: Int
        }"#,
        r#":create type_use_trait_super_slot {
            type_use_id: Uuid,
            at: Validity =>
            supertrait_index: Int
        }"#,
        r#":create type_use_generic_bound_slot {
            type_use_id: Uuid,
            at: Validity =>
            generic_param_index: Int,
            bound_index: Int
        }"#,
        r#":create type_use_generic_param_bound_slot {
            type_use_id: Uuid,
            at: Validity =>
            containing_owner_id: Uuid,
            generic_param_index: Int,
            bound_index: Int
        }"#,
        r#":create type_use_where_subject_slot {
            type_use_id: Uuid,
            at: Validity =>
            predicate_index: Int
        }"#,
        r#":create type_use_where_bound_slot {
            type_use_id: Uuid,
            at: Validity =>
            predicate_index: Int,
            bound_index: Int
        }"#,
        r#":create type_use_where_generic_param_bound_slot {
            type_use_id: Uuid,
            at: Validity =>
            containing_owner_id: Uuid,
            predicate_index: Int,
            bound_index: Int
        }"#,
        r#":create type_use_associated_type_bound_slot {
            type_use_id: Uuid,
            at: Validity =>
            associated_type_index: Int,
            associated_type_name: String,
            bound_index: Int
        }"#,
    ] {
        db.run_script(script, BTreeMap::new(), cozo::ScriptMutability::Mutable)?;
    }
    Ok(())
}

fn insert_coordinate_relation(
    db: &Db<MemStorage>,
    relation: &str,
    key_fields: &[&str],
    values: &[(&str, cozo::DataValue)],
    value_fields: &str,
) -> Result<(), TransformError> {
    let mut params = BTreeMap::new();
    let mut assignments = Vec::with_capacity(values.len() + 1);
    for (name, value) in values {
        params.insert((*name).to_string(), value.clone());
        assignments.push(format!("{name} = ${name}"));
    }
    assignments.push("at = 'ASSERT'".to_string());

    let key_fields = key_fields.join(", ");
    let assignments = assignments.join(",\n                ");
    let script = format!(
        r#"?[{key_fields}, {value_fields}] :=
                {assignments}
            :put {relation} {{ {key_fields} => {value_fields} }}"#
    );

    db.run_script(&script, params, cozo::ScriptMutability::Mutable)?;
    Ok(())
}

fn int(value: usize) -> cozo::DataValue {
    cozo::DataValue::from(value as i64)
}

pub struct TypeContainsSchema;

impl TypeContainsSchema {
    pub const RELATION: &'static str = "type_contains";

    pub fn create_and_insert_schema(db: &Db<MemStorage>) -> Result<(), TransformError> {
        db.run_script(
            r#":create type_contains {
                parent_type_id: Uuid,
                child_type_id: Uuid,
                kind: String,
                position: Int,
                at: Validity
            }"#,
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }

    pub fn insert_relation(
        db: &Db<MemStorage>,
        parent_type_id: cozo::DataValue,
        child_type_id: cozo::DataValue,
        kind: &'static str,
        position: Option<usize>,
    ) -> Result<(), TransformError> {
        let mut params = BTreeMap::new();
        params.insert("parent_type_id".to_string(), parent_type_id);
        params.insert("child_type_id".to_string(), child_type_id);
        params.insert("kind".to_string(), cozo::DataValue::from(kind));
        params.insert(
            "position".to_string(),
            cozo::DataValue::from(position.map_or(-1_i64, |idx| idx as i64)),
        );

        db.run_script(
            r#"?[parent_type_id, child_type_id, kind, position, at] :=
                parent_type_id = $parent_type_id,
                child_type_id = $child_type_id,
                kind = $kind,
                position = $position,
                at = 'ASSERT'
            :put type_contains { parent_type_id, child_type_id, kind, position, at }"#,
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }
}

impl SyntacticRelationSchema {
    /// Transforms a SyntacticRelation into parameters for database insertion
    pub fn relation_to_params(
        &self,
        relation: &SyntacticRelation,
    ) -> BTreeMap<String, cozo::DataValue> {
        let source_id = relation.source().to_cozo_uuid();
        let target_id = relation.target().to_cozo_uuid();
        let relation_kind = relation.kind_str();
        let schema = &SyntacticRelationSchema::SCHEMA;

        // ANCHOR: impl_trait_associated_edges
        // TODO(import-backlinks): Keep import-bearing relation kinds explicit here. `ModuleImports`,
        // `ReExports`, and `ImportedBy` now participate in the real graph contract, so if relation
        // kind folding or endpoint typing changes, verify the `syntax_edge` surface still preserves
        // import semantics cleanly for downstream queries/importers.
        // Extract the node type names from the relation variant
        let (source_kind, target_kind) = match relation {
            SyntacticRelation::Contains { .. } => ("Module", "Primary"),
            // NOTE: ResolvesToDefinition needs work, this ModuleNode->ModuleNode is ambiguous.
            SyntacticRelation::ResolvesToDefinition { .. } => ("Module", "Module"),
            SyntacticRelation::CustomPath { .. } => ("Module", "Module"),
            SyntacticRelation::Sibling { .. } => ("Module", "Module"),
            SyntacticRelation::ModuleImports { .. } => ("Module", "Import"),
            SyntacticRelation::ReExports { .. } => ("Import", "Primary"),
            SyntacticRelation::StructField { .. } => ("Struct", "Field"),
            SyntacticRelation::UnionField { .. } => ("Union", "Field"),
            SyntacticRelation::VariantField { .. } => ("Variant", "Field"),
            SyntacticRelation::EnumVariant { .. } => ("Enum", "Variant"),
            SyntacticRelation::ImplAssociatedItem { .. } => ("Impl", "AssociatedItem"),
            SyntacticRelation::TraitAssociatedItem { .. } => ("Trait", "AssociatedItem"),
            SyntacticRelation::ImportedBy { .. } => ("Primary", "Import"),
        };
        // ANCHOR_END: impl_trait_associated_edges

        BTreeMap::from([
            (schema.source_id().to_string(), source_id),
            (schema.target_id().to_string(), target_id),
            (
                schema.relation_kind().to_string(),
                cozo::DataValue::from(relation_kind),
            ),
            (
                schema.source_kind().to_string(),
                cozo::DataValue::from(source_kind),
            ),
            (
                schema.target_kind().to_string(),
                cozo::DataValue::from(target_kind),
            ),
        ])
    }

    /// Inserts a syntactic relation into the database
    pub fn insert_relation(
        &self,
        db: &Db<MemStorage>,
        relation: &SyntacticRelation,
    ) -> Result<(), TransformError> {
        let params = self.relation_to_params(relation);
        let script = &self.script_put(&params);
        db.run_script(script, params, cozo::ScriptMutability::Mutable)
            .inspect_err(|&_| {
                tracing::error!(target: "db", "{} {}\n{} {}",
                    "Error:".log_error().bold(),
                    format_args!("running script {}", script.log_orange()),
                    "type_node info:".log_foreground_primary_debug(),
                    format!("{:#?}", relation).log_orange()
                );
            })?;
        Ok(())
    }
}

impl TypeRelationSchema {
    pub fn insert_relation(
        &self,
        db: &Db<MemStorage>,
        relation: &TypeRelation,
    ) -> Result<(), TransformError> {
        let schema = &TypeRelationSchema::SCHEMA;
        let params = match relation {
            TypeRelation::Ordinary { source, target } => BTreeMap::from([
                (schema.source_id().to_string(), source.to_cozo_uuid()),
                (
                    schema.target_id().to_string(),
                    syn_parser::parser::nodes::AnyNodeId::from(*target).to_cozo_uuid(),
                ),
                (
                    schema.relation_kind().to_string(),
                    cozo::DataValue::from(relation.kind_str()),
                ),
                (
                    schema.source_kind().to_string(),
                    cozo::DataValue::from(source.kind_name()),
                ),
                (
                    schema.target_kind().to_string(),
                    cozo::DataValue::from(format!("{:?}", target.kind())),
                ),
            ]),
            TypeRelation::Trait { source, target } => BTreeMap::from([
                (schema.source_id().to_string(), source.to_cozo_uuid()),
                (
                    schema.target_id().to_string(),
                    syn_parser::parser::nodes::AnyNodeId::from(*target).to_cozo_uuid(),
                ),
                (
                    schema.relation_kind().to_string(),
                    cozo::DataValue::from(relation.kind_str()),
                ),
                (
                    schema.source_kind().to_string(),
                    cozo::DataValue::from(source.kind_name()),
                ),
                (
                    schema.target_kind().to_string(),
                    cozo::DataValue::from(format!("{:?}", target.kind())),
                ),
            ]),
        };

        let script = schema.script_put(&params);
        db.run_script(&script, params, cozo::ScriptMutability::Mutable)?;

        Ok(())
    }
}

#[cfg(feature = "level_up_relations")]
impl SemanticRelationSchema {
    /// Derives a semantic "contains" relation from syntactic relations
    pub fn derive_contains_relation<T, S>(
        &self,
        db: &Db<MemStorage>,
        source_id: T,
        target_id: S,
        // TODO: Consider seriously whether each edge should have its own hashed ID. This seems
        // wasteful since we will likely have an index for the edges generated by `cozo` as well,
        // but we would still like a way to refer to the edges more directly outside of cozo.
        // syntactic_ids: Vec<SyntacticRelationId>,
        //
        // This could work... as long as we had a strict rule that only one kind of relation
        // could exist between source->target. Not sure whether that is a good idea, or even
        // feasable.
        // syntactic_ids: Vec<(AnyNodeId, AnyNodeId)>,

        // This seems like it both makes the most sense but still needs to have some analog in
        // `cozo` that makes sense.
        // Need to think this over.
        syntactic_ids: Vec<SyntacticRelation>,
    ) -> Result<(), TransformError>
    where
        T: AsAnyNodeId + TypedId,
        S: AsAnyNodeId + TypedId,
    {
        let params = BTreeMap::from([
            (
                "source_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(source_id)),
            ),
            (
                "target_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(target_id)),
            ),
            (
                "relation_kind".to_string(),
                cozo::DataValue::from("Contains"),
            ),
            (
                "derived_from".to_string(),
                cozo::DataValue::List(
                    syntactic_ids
                        .into_iter()
                        .map(|id| cozo::DataValue::Uuid(cozo::UuidWrapper(id)))
                        .collect(),
                ),
            ),
        ]);

        db.run_script(
            &self.script_put(&params),
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }

    /// Derives a semantic "defines" relation from syntactic relations
    pub fn derive_defines_relation(
        &self,
        db: &Db<MemStorage>,
        source_id: uuid::Uuid,
        target_id: uuid::Uuid,
        syntactic_ids: Vec<uuid::Uuid>,
    ) -> Result<(), TransformError> {
        let params = BTreeMap::from([
            (
                "source_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(source_id)),
            ),
            (
                "target_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(target_id)),
            ),
            (
                "relation_kind".to_string(),
                cozo::DataValue::from("Defines"),
            ),
            (
                "derived_from".to_string(),
                cozo::DataValue::List(
                    syntactic_ids
                        .into_iter()
                        .map(|id| cozo::DataValue::Uuid(cozo::UuidWrapper(id)))
                        .collect(),
                ),
            ),
        ]);

        db.run_script(
            &self.script_put(&params),
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }

    /// Derives a semantic "uses" relation from syntactic relations
    pub fn derive_uses_relation(
        &self,
        db: &Db<MemStorage>,
        source_id: uuid::Uuid,
        target_id: uuid::Uuid,
        syntactic_ids: Vec<uuid::Uuid>,
    ) -> Result<(), TransformError> {
        let params = BTreeMap::from([
            (
                "source_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(source_id)),
            ),
            (
                "target_id".to_string(),
                cozo::DataValue::Uuid(cozo::UuidWrapper(target_id)),
            ),
            ("relation_kind".to_string(), cozo::DataValue::from("Uses")),
            (
                "derived_from".to_string(),
                cozo::DataValue::List(
                    syntactic_ids
                        .into_iter()
                        .map(|id| cozo::DataValue::Uuid(cozo::UuidWrapper(id)))
                        .collect(),
                ),
            ),
        ]);

        db.run_script(
            &self.script_put(&params),
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }
}
