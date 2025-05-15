//! Edge schema definitions for the Cozo database
//!
//! This module contains schemas for edge relations in the graph, with a two-tiered approach:
//! 1. Syntactic relations - directly representing AST relationships
//! 2. Semantic relations - higher-level logical relationships derived from syntactic ones

use super::*;
use cozo::{Db, MemStorage};
use itertools::Itertools;
use std::collections::BTreeMap;
use syn_parser::utils::LogStyle;
use syn_parser::parser::relations::SyntacticRelation;

/// Macro for defining edge schemas in the Cozo database
///
/// Similar to `define_schema!` but specialized for edge relations with source and target fields.
/// Provides methods for creating and querying edge relations.
#[macro_export]
macro_rules! define_edge_schema {
    ($schema_name:ident {
        $relation:literal,
        $($field_name:ident: $dv:literal),+
        $(,)?
    }) => {
        pub struct $schema_name {
            pub relation: &'static str,
            $($field_name: CozoField),+
        }

        impl $schema_name {
            pub const SCHEMA: Self = Self {
                relation: $relation,
                $($field_name: CozoField { st: stringify!($field_name), dv: $dv }),+
            };

            $(pub fn $field_name(&self) -> &str {
                self.$field_name.st()
            })*

            /// Returns the source field name, which should be present in all edge schemas
            pub fn source_field(&self) -> &str {
                self.source_id.st()
            }

            /// Returns the target field name, which should be present in all edge schemas
            pub fn target_field(&self) -> &str {
                self.target_id.st()
            }
        }

        impl $schema_name {
            pub fn script_create(&self) -> String {
                let fields = vec![
                    $(format!("{}: {}", self.$field_name.st(), self.$field_name.dv())),+
                ];
                format!(":create {} {{ {} }}", $relation, fields.join(", "))
            }

            pub fn script_put(&self, params: &BTreeMap<String, cozo::DataValue>) -> String {
                let entry_names = params.keys().join(", ");
                let param_names = params.keys().map(|k| format!("${}", k)).join(", ");
                format!(
                    "?[{}] <- [[{}]] :put {}",
                    entry_names, param_names, self.relation
                )
            }

            pub fn log_create_script(&self) {
                log::info!(target: "db",
                    "{} {}: {:?}",
                    "Printing edge schema".log_header(),
                    $relation.log_name(),
                    self.script_create()
                );
            }

            pub(crate) fn create_and_insert(
                &self,
                db: &Db<MemStorage>,
            ) -> Result<(), Box<cozo::Error>> {
                let const_schema = Self::SCHEMA;
                let db_result = db.run_script(
                    &const_schema.script_create(),
                    BTreeMap::new(),
                    cozo::ScriptMutability::Mutable,
                )?;
                self.log_create_script();
                log_db_result(db_result);
                Ok(())
            }
            
            /// Creates a traversal query that follows this edge type from a given source node
            pub fn traverse_from(&self, source_param: &str) -> String {
                format!(
                    "?[target] := *{}[source, target], source = ${} :limit 1000",
                    self.relation, source_param
                )
            }
            
            /// Creates a traversal query that follows this edge type to a given target node
            pub fn traverse_to(&self, target_param: &str) -> String {
                format!(
                    "?[source] := *{}[source, target], target = ${} :limit 1000",
                    self.relation, target_param
                )
            }
        }
    };
}

// Define the syntactic relation schema (base layer)
define_edge_schema!(SyntacticRelationSchema {
    "syntactic_relation",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    source_kind: "String",
    target_kind: "String"
});

// Define the semantic relation schema (higher level)
define_edge_schema!(SemanticRelationSchema {
    "semantic_relation",
    source_id: "Uuid",
    target_id: "Uuid",
    relation_kind: "String",
    derived_from: "[Uuid]?"
});

impl SyntacticRelationSchema {
    /// Transforms a SyntacticRelation into parameters for database insertion
    pub fn relation_to_params(&self, relation: &SyntacticRelation) -> BTreeMap<String, cozo::DataValue> {
        let source_id = relation.source().uuid();
        let target_id = relation.target().uuid();
        let relation_kind = format!("{:?}", relation);
        
        // Extract the node type names from the relation variant
        let source_kind = match relation {
            SyntacticRelation::Contains { source, .. } => "Module",
            SyntacticRelation::ResolvesToDefinition { source, .. } => "Module",
            SyntacticRelation::CustomPath { source, .. } => "Module",
            SyntacticRelation::Sibling { source, .. } => "Module",
            SyntacticRelation::ModuleImports { source, .. } => "Module",
            SyntacticRelation::ReExports { source, .. } => "Import",
            SyntacticRelation::StructField { source, .. } => "Struct",
            SyntacticRelation::UnionField { source, .. } => "Union",
            SyntacticRelation::VariantField { source, .. } => "Variant",
            SyntacticRelation::EnumVariant { source, .. } => "Enum",
            SyntacticRelation::ImplAssociatedItem { source, .. } => "Impl",
            SyntacticRelation::TraitAssociatedItem { source, .. } => "Trait",
        };
        
        let target_kind = match relation {
            SyntacticRelation::Contains { target, .. } => "PrimaryNode",
            SyntacticRelation::ResolvesToDefinition { target, .. } => "Module",
            SyntacticRelation::CustomPath { target, .. } => "Module",
            SyntacticRelation::Sibling { target, .. } => "Module",
            SyntacticRelation::ModuleImports { target, .. } => "Import",
            SyntacticRelation::ReExports { target, .. } => "PrimaryNode",
            SyntacticRelation::StructField { target, .. } => "Field",
            SyntacticRelation::UnionField { target, .. } => "Field",
            SyntacticRelation::VariantField { target, .. } => "Field",
            SyntacticRelation::EnumVariant { target, .. } => "Variant",
            SyntacticRelation::ImplAssociatedItem { target, .. } => "AssociatedItem",
            SyntacticRelation::TraitAssociatedItem { target, .. } => "AssociatedItem",
        };
        
        BTreeMap::from([
            ("source_id".to_string(), cozo::DataValue::Uuid(cozo::UuidWrapper(source_id))),
            ("target_id".to_string(), cozo::DataValue::Uuid(cozo::UuidWrapper(target_id))),
            ("relation_kind".to_string(), cozo::DataValue::from(relation_kind)),
            ("source_kind".to_string(), cozo::DataValue::from(source_kind)),
            ("target_kind".to_string(), cozo::DataValue::from(target_kind)),
        ])
    }
    
    /// Inserts a syntactic relation into the database
    pub fn insert_relation(
        &self,
        db: &Db<MemStorage>,
        relation: &SyntacticRelation,
    ) -> Result<(), cozo::Error> {
        let params = self.relation_to_params(relation);
        db.run_script(
            &self.script_put(&params),
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }
}

impl SemanticRelationSchema {
    /// Derives a semantic "contains" relation from syntactic relations
    pub fn derive_contains_relation(
        &self,
        db: &Db<MemStorage>,
        source_id: uuid::Uuid,
        target_id: uuid::Uuid,
        syntactic_ids: Vec<uuid::Uuid>,
    ) -> Result<(), cozo::Error> {
        let params = BTreeMap::from([
            ("source_id".to_string(), cozo::DataValue::Uuid(cozo::UuidWrapper(source_id))),
            ("target_id".to_string(), cozo::DataValue::Uuid(cozo::UuidWrapper(target_id))),
            ("relation_kind".to_string(), cozo::DataValue::from("Contains")),
            ("derived_from".to_string(), cozo::DataValue::List(
                syntactic_ids.into_iter()
                    .map(|id| cozo::DataValue::Uuid(cozo::UuidWrapper(id)))
                    .collect()
            )),
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
    ) -> Result<(), cozo::Error> {
        let params = BTreeMap::from([
            ("source_id".to_string(), cozo::DataValue::Uuid(cozo::UuidWrapper(source_id))),
            ("target_id".to_string(), cozo::DataValue::Uuid(cozo::UuidWrapper(target_id))),
            ("relation_kind".to_string(), cozo::DataValue::from("Defines")),
            ("derived_from".to_string(), cozo::DataValue::List(
                syntactic_ids.into_iter()
                    .map(|id| cozo::DataValue::Uuid(cozo::UuidWrapper(id)))
                    .collect()
            )),
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
    ) -> Result<(), cozo::Error> {
        let params = BTreeMap::from([
            ("source_id".to_string(), cozo::DataValue::Uuid(cozo::UuidWrapper(source_id))),
            ("target_id".to_string(), cozo::DataValue::Uuid(cozo::UuidWrapper(target_id))),
            ("relation_kind".to_string(), cozo::DataValue::from("Uses")),
            ("derived_from".to_string(), cozo::DataValue::List(
                syntactic_ids.into_iter()
                    .map(|id| cozo::DataValue::Uuid(cozo::UuidWrapper(id)))
                    .collect()
            )),
        ]);
        
        db.run_script(
            &self.script_put(&params),
            params,
            cozo::ScriptMutability::Mutable,
        )?;
        Ok(())
    }
}

/// Transforms syntactic relations into the database
pub fn transform_relations(
    db: &Db<MemStorage>,
    relations: Vec<SyntacticRelation>,
) -> Result<(), cozo::Error> {
    let syntactic_schema = SyntacticRelationSchema::SCHEMA;
    
    // Create the schema if it doesn't exist
    syntactic_schema.create_and_insert(db)?;
    
    // Create the semantic schema
    SemanticRelationSchema::SCHEMA.create_and_insert(db)?;
    
    // Insert all syntactic relations
    for relation in &relations {
        syntactic_schema.insert_relation(db, relation)?;
    }
    
    // Here we would derive semantic relations from syntactic ones
    // This is a placeholder for future implementation
    
    Ok(())
}
