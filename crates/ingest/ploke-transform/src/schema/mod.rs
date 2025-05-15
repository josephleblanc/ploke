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

use cozo::{Db, MemStorage};
use itertools::Itertools;
use std::collections::BTreeMap;
use syn_parser::utils::LogStyle;

use crate::utils::log_db_result;

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
        }
    };
}
