//! Cozo Schema
//!
//! Contains the schema for all relations created by transforming parsed entities into the cozo
//! database. These are separated into categories:
//!     - primary nodes (primary_nodes): Nodes which may be direct children of a file-level or inline
//!     module.
//!     - secondary nodes (secondary_nodes): Nodes which may not be direct children of a file-level
//!     module, but must exist within a primary node's scope (e.g. field, function param)
//!     - associated nodes (assoc_nodes): Nodes which may be defined within an impl block (e.g.
//!     methods, associated constants, etc)
//!     - subnode_variants (subnode_variants): Different subnode fields that are treated as enum
//!     variants while parsed, but are split into separate relations during the transform into the
//!     database.
//!     - edges: Relations between nodes, with both syntactic (AST-based) and semantic (logical) layers
// TODO: add to docs:
// - types
pub mod assoc_nodes;
pub mod edges;
pub mod primary_nodes;
pub mod secondary_nodes;
pub mod subnode_variants;
pub mod types;

use crate::error::TransformError;
use assoc_nodes::MethodNodeSchema;
use cozo::{Db, MemStorage};
use edges::SyntacticRelationSchema;
use itertools::Itertools;
use primary_nodes::*;
use secondary_nodes::*;
use types::create_and_insert_types;
use std::collections::BTreeMap;
use subnode_variants::FileModuleNodeSchema;
use syn_parser::utils::LogStyle;

// TODO: use lazy_static and SmartString
// &str for now,
// find a better solution using lazy_static and maybe SmartString, since that is what cozo uses
// anyawys
pub struct CozoField {
    st: &'static str,
    dv: &'static str,
}
impl CozoField {
    fn schema_str(&self) -> impl Iterator<Item = char> {
        self.st.chars().chain(": ".chars()).chain(self.dv.chars())
    }
}

impl CozoField {
    pub fn st(&self) -> &str {
        self.st
    }

    pub fn dv(&self) -> &str {
        self.dv
    }
}

/// Create schema for all nodes and enter them into the database.
/// This step must only be run once to avoid errors from the database.
pub fn create_schema_all(db: &Db<MemStorage>) -> Result<(), crate::error::TransformError> {
    // ---- primary nodes ----
    ConstNodeSchema::create_and_insert_schema(db)?;
    EnumNodeSchema::create_and_insert_schema(db)?;
    FunctionNodeSchema::create_and_insert_schema(db)?;
    ImplNodeSchema::create_and_insert_schema(db)?;
    ImportNodeSchema::create_and_insert_schema(db)?;
    MacroNodeSchema::create_and_insert_schema(db)?;
    ModuleNodeSchema::create_and_insert_schema(db)?;
    StaticNodeSchema::create_and_insert_schema(db)?;
    StructNodeSchema::create_and_insert_schema(db)?;
    TraitNodeSchema::create_and_insert_schema(db)?;
    TypeAliasNodeSchema::create_and_insert_schema(db)?;
    UnionNodeSchema::create_and_insert_schema(db)?;
    // -- special handling --
    FileModuleNodeSchema::create_and_insert_schema(db)?;


    // -- secondary nodes --
    AttributeNodeSchema::create_and_insert_schema(db)?;

    create_and_insert_generic_schema(db)?;

    VariantNodeSchema::create_and_insert_schema(db)?;
    FieldNodeSchema::create_and_insert_schema(db)?;


    // -- associated nodes --
    MethodNodeSchema::create_and_insert_schema(db)?;

    // -- type_nodes --
    create_and_insert_types(db)?;

    // -- edges --
    SyntacticRelationSchema::create_and_insert_schema(db)?;
    Ok(())
}

/// Example
/// define_schema!(FunctionNodeSchema {
///     id: "Uuid",
///     name: "String",
///     docstring: "String?",
///     tracking_hash: "Uuid"
/// });
#[macro_export]
macro_rules! define_schema {
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

            pub const SCHEMA_FIELDS: &'static [&'static str] = &[
                $( stringify!($field_name) ),+
            ];
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
                // Should come out looking like:
                // "?[owner_id, param_index, kind, name, type_id] <- [[$owner_id, $param_index, $kind, $name, $type_id]] :put generic_params",
                let script = format!(
                    "?[{}] <- [[{}]] :put {}",
                    entry_names, param_names, self.relation
                );
                script
            }

            pub fn log_create_script(&self) {
                log::info!(target: "db",
                    "{} {}: {:?}",
                    "Printing schema".log_header(),
                    $relation.log_name(),
                    self.script_create()
                );
            }

            pub(crate) fn create_and_insert(
                &self,
                db: &Db<MemStorage>,
            ) -> Result<(), TransformError> {
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
            
            pub(crate) fn create_and_insert_schema(db: &Db<MemStorage>) -> Result<(), TransformError> {
                let schema = &Self::SCHEMA;
                let script_create = &schema.script_create();
                schema.log_create_script();
                let db_result = db.run_script(
                    script_create,
                    BTreeMap::new(),
                    cozo::ScriptMutability::Mutable,
                )?;
                log_db_result(db_result);
                Ok(())
            }
        }
    };
}

/// Helper function to create and insert the three types of generics at once.
pub(crate) fn create_and_insert_generic_schema(db: &Db<MemStorage>) -> Result<(), TransformError> {
    GenericTypeNodeSchema::create_and_insert_schema(db)?;
    GenericConstNodeSchema::create_and_insert_schema(db)?;
    GenericLifetimeNodeSchema::create_and_insert_schema(db)?;
    Ok(())
}

pub(crate) fn log_db_result(db_result: cozo::NamedRows) {
    log::info!(target: "db",
        "{} {:?}",
        "  Db return: ".log_step(),
        db_result,
    );
}




