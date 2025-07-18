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
use syn_parser::resolve::Colorize;
use syn_parser::utils::{LogStyle, LogStyleDebug};

// #[macro_export]
// macro_rules! define_edge_schema {
//     ($schema_name:ident {
//         $relation:literal,
//         $($field_name:ident: $dv:literal),+
//         $(,)?
//     }) => {
//         pub struct $schema_name {
//             pub relation: &'static str,
//             $($field_name: CozoField),+
//         }
//
//         impl $schema_name {
//             pub const SCHEMA: Self = Self {
//                 relation: $relation,
//                 $($field_name: CozoField { st: stringify!($field_name), dv: $dv }),+
//             };
//
//             $(pub fn $field_name(&self) -> &str {
//                 self.$field_name.st()
//             })*
//
//             /// Returns the source field name, which should be present in all edge schemas
//             pub fn source_field(&self) -> &str {
//                 self.source_id.st()
//             }
//
//             /// Returns the target field name, which should be present in all edge schemas
//             pub fn target_field(&self) -> &str {
//                 self.target_id.st()
//             }
//         }
//
//         impl $schema_name {
//             pub fn script_create(&self) -> String {
//                 let fields = vec![
//                     $(format!("{}: {}", self.$field_name.st(), self.$field_name.dv())),+
//                 ];
//                 format!(":create {} {{ {} }}", $relation, fields.join(", "))
//             }
//
//             pub fn script_put(&self, params: &BTreeMap<String, cozo::DataValue>) -> String {
//                 let entry_names = params.keys().join(", ");
//                 let param_names = params.keys().map(|k| format!("${}", k)).join(", ");
//                 format!(
//                     "?[{}] <- [[{}]] :put {}",
//                     entry_names, param_names, self.relation
//                 )
//             }
//
//             pub fn log_create_script(&self) {
//                 tracing::info!(target: "db",
//                     "{} {}: {:?}",
//                     "Printing edge schema".log_header(),
//                     $relation.log_name(),
//                     self.script_create()
//                 );
//             }
//
//             pub(crate) fn create_and_insert(
//                 &self,
//                 db: &Db<MemStorage>,
//             ) -> Result<(), Box<TransformError>> {
//                 let const_schema = Self::SCHEMA;
//                 let db_result = db.run_script(
//                     &const_schema.script_create(),
//                     BTreeMap::new(),
//                     cozo::ScriptMutability::Mutable,
//                 )?;
//                 self.log_create_script();
//                 log_db_result(db_result);
//                 Ok(())
//             }
//
//             /// NOTE: Not clear that this is valid cozo. Use with care.
//             /// Creates a traversal query that follows this edge type from a given source node
//             pub fn traverse_from(&self, source_param: &str) -> String {
//                 format!(
//                     "?[target] := *{}[source, target], source = ${} :limit 1000",
//                     self.relation, source_param
//                 )
//             }
//
//             /// NOTE: Not clear that this is valid cozo. Use with care.
//             /// Creates a traversal query that follows this edge type to a given target node
//             pub fn traverse_to(&self, target_param: &str) -> String {
//                 format!(
//                     "?[source] := *{}[source, target], target = ${} :limit 1000",
//                     self.relation, target_param
//                 )
//             }
//         }
//     };
// }

define_schema!(SyntacticRelationSchema {
    "syntax_edge",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    source_kind: "String",
    target_kind: "String"
});

// NOTE: WIP
// Example sketch
// macro_rules! relation_transformer {
// ($variant, $source_kind:ident, $target_kind:ident) => {
//     match $enum {
//         $enum::
//     }
//     SyntacticRelation::$variant { source, target } => {
//         let params = BTreeMap::from([
//             ("source_id", source.uuid().into()),
//             ("target_id", target.uuid().into()),
//             ("relation_variant", stringify!($variant).into()),
//             ("source_kind", $source_kind.as_any().to_cozo_uuid()),
//             ("target_kind", $target_kind.as_any().to_cozo_uuid()),
//         ]);
//         params
//     }
// };
// }
//
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
        };

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
