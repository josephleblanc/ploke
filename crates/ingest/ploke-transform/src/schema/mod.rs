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
pub mod crate_node;

use crate::error::TransformError;
use assoc_nodes::MethodNodeSchema;
use cozo::{Db, MemStorage};
use crate_node::CrateContextSchema;
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
    // -- secondary nodes --
    ParamNodeSchema::create_and_insert_schema(db)?;
    AttributeNodeSchema::create_and_insert_schema(db)?;
    VariantNodeSchema::create_and_insert_schema(db)?;
    FieldNodeSchema::create_and_insert_schema(db)?;
    create_and_insert_generic_schema(db)?;
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



    // -- associated nodes --
    MethodNodeSchema::create_and_insert_schema(db)?;

    // -- type_nodes --
    create_and_insert_types(db)?;

    // -- edges --
    SyntacticRelationSchema::create_and_insert_schema(db)?;

    // -- crate_context --
    CrateContextSchema::create_and_insert_schema(db)?;
    Ok(())
}

pub const ID_KEYWORDS: [&str; 6] = [ 
    "id", 
    "function_id", 
    "owner_id", 
    "source_id", 
    "target_id",
    "type_id"
];
pub const ID_VAL_KEYWORDS: [&str; 6] = [
    "id: Uuid", 
    "function_id: Uuid", 
    "owner_id: Uuid",
    "source_id: Uuid",
    "target_id: Uuid",
    "type_id: Uuid"
];

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
            /// An iterator over the keys for a given schema (the fields for the scheme on the left
            /// of the => symbol in cozo)
            /// e.g. `id`, `owner_id`, etc, not including `at`
            pub fn keys(&self) -> impl Iterator<Item = &&'static str> {
                Self::SCHEMA_FIELDS.iter().filter(|f| ID_KEYWORDS.contains(f))
            }

            /// An iterator over the values for a given schema (the fields for the scheme on the right
            /// of the => symbol in cozo)
            /// e.g. `name`, `items`, etc
            pub fn vals(&self) -> impl Iterator<Item = &&'static str> {
                Self::SCHEMA_FIELDS.iter().filter(|f| !ID_KEYWORDS.contains(f))
            }

            pub fn script_identity(&self) -> String {
                let fields = vec![
                    $(
                            format!("{}", self.$field_name.st())
                    ),+
                ];
                let keys = fields.iter().filter(|f| ID_KEYWORDS.contains(&f.as_str())).join(", ");
                let vals = fields.iter().filter(|f| !ID_KEYWORDS.contains(&f.as_str())).join(", ");
                    format!("{} {{ {keys}, at => {vals} }}", $relation)
            }

            pub fn script_create(&self) -> String {
                let fields = vec![
                    $(
                            format!("{}: {}", self.$field_name.st(), self.$field_name.dv())
                    ),+
                ];
                let keys = fields.iter().filter(|f| ID_VAL_KEYWORDS.contains(&f.as_str())).join(", ");
                let vals = fields.iter().filter(|f| !ID_VAL_KEYWORDS.contains(&f.as_str())).join(", ");
                format!(":create {} {{ {}, at: Validity => {} }}", $relation, keys, vals)
            }

            pub fn script_put(&self, params: &BTreeMap<String, cozo::DataValue>) -> String {
                let lhs_keys = params.keys()
                        .filter(|k| ID_KEYWORDS.contains(&k.as_str()))
                        .join(", ");
                let lhs_entries = params.keys()
                        .filter(|k| !ID_KEYWORDS.contains(&k.as_str()))
                        .join(", ");
                let rhs_keys = params.keys()
                        .filter(|k| ID_KEYWORDS.contains(&k.as_str()))
                        .map(|k| format!("${}", k))
                        .join(", ");
                let rhs_entries = params.keys()
                        .filter(|k| !ID_KEYWORDS.contains(&k.as_str()))
                        .map(|k| format!("${}", k))
                        .join(", ");
                let script = format!(
                    "?[{}, at, {}] <- [[{}, 'ASSERT', {}]] :put {}",
                    lhs_keys, lhs_entries, rhs_keys, rhs_entries,
                    self.script_identity()
                );
                script
            }

            pub fn log_create_script(&self) {
                tracing::trace!(target: "db",
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
    tracing::trace!(target: "db",
        "{} {:?}",
        "  Db return: ".log_step(),
        db_result,
    );
}




