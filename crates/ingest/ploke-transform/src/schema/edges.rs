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
#[cfg(feature = "typed_type_graph")]
use syn_parser::parser::relations::TypeRelation;
use syn_parser::resolve::Colorize;
#[cfg(not(feature = "typed_type_graph"))]
use syn_parser::resolve::type_resolution::TypeUseResolution;
use syn_parser::utils::{LogStyle, LogStyleDebug};

define_schema!(SyntacticRelationSchema {
    "syntax_edge",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    source_kind: "String",
    target_kind: "String"
});

#[cfg(not(feature = "typed_type_graph"))]
define_schema!(ResolvedTypeUseSchema {
    "resolved_type_use",
    owner_id: "Uuid",
    type_id: "Uuid",
    target_id: "Uuid",
    resolved_type_id: "Uuid?",
    role: "String",
    target_kind: "String"
});

#[cfg(feature = "typed_type_graph")]
define_schema!(TypeRelationSchema {
    "type_relation",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    source_kind: "String",
    target_kind: "String"
});

#[cfg(feature = "typed_type_graph")]
pub struct TypeUseSchema;

#[cfg(feature = "typed_type_graph")]
impl TypeUseSchema {
    pub const RELATION: &'static str = "type_use";

    pub fn create_and_insert_schema(db: &Db<MemStorage>) -> Result<(), TransformError> {
        db.run_script(
            r#":create type_use {
                owner_id: Uuid,
                root_type_id: Uuid,
                role: String,
                slot_index: Int,
                at: Validity
            }"#,
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }

    pub fn insert_relation(
        db: &Db<MemStorage>,
        owner_id: cozo::DataValue,
        root_type_id: cozo::DataValue,
        role: &'static str,
        slot_index: Option<usize>,
    ) -> Result<(), TransformError> {
        let mut params = BTreeMap::new();
        params.insert("owner_id".to_string(), owner_id);
        params.insert("root_type_id".to_string(), root_type_id);
        params.insert("role".to_string(), cozo::DataValue::from(role));
        params.insert(
            "slot_index".to_string(),
            cozo::DataValue::from(slot_index.map_or(-1_i64, |idx| idx as i64)),
        );

        db.run_script(
            r#"?[owner_id, root_type_id, role, slot_index, at] :=
                owner_id = $owner_id,
                root_type_id = $root_type_id,
                role = $role,
                slot_index = $slot_index,
                at = 'ASSERT'
            :put type_use { owner_id, root_type_id, role, slot_index, at }"#,
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }
}

#[cfg(feature = "typed_type_graph")]
pub struct TypeContainsSchema;

#[cfg(feature = "typed_type_graph")]
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

#[cfg(feature = "typed_type_graph")]
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

#[cfg(not(feature = "typed_type_graph"))]
impl ResolvedTypeUseSchema {
    pub fn insert_resolution(
        &self,
        db: &Db<MemStorage>,
        resolution: &TypeUseResolution,
    ) -> Result<(), TransformError> {
        let Some(target_id) = resolution.item_target() else {
            return Ok(());
        };

        let schema = &ResolvedTypeUseSchema::SCHEMA;
        let params = BTreeMap::from([
            (
                schema.owner_id().to_string(),
                resolution.owner.to_cozo_uuid(),
            ),
            (
                schema.type_id().to_string(),
                resolution.source_type_id.to_cozo_uuid(),
            ),
            (schema.target_id().to_string(), target_id.to_cozo_uuid()),
            (
                schema.resolved_type_id().to_string(),
                resolution
                    .resolved_type_id
                    .map(|type_id| type_id.to_cozo_uuid())
                    .unwrap_or(cozo::DataValue::Null),
            ),
            (
                schema.role().to_string(),
                cozo::DataValue::from(resolution.role.as_str()),
            ),
            (
                schema.target_kind().to_string(),
                cozo::DataValue::from(
                    resolution
                        .resolved_ref
                        .resolved_target_kind()
                        .map(|kind| kind.as_str())
                        .unwrap_or("unknown"),
                ),
            ),
        ]);
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
