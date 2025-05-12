pub mod assoc_nodes;
pub mod primary_nodes;
pub mod secondary_nodes;

use itertools::Itertools;
use std::collections::BTreeMap;
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

// define_schema!(FunctionNodeSchemaV2 {
//     id: "id" => "Uuid",
//     name: "name" => "String",
//     docstring: "docstring" => "String?",
//     tracking_hash: "tracking_hash" => "Uuid"
// });
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
                log::info!(target: "transform_function",
                    "{} {}: {:?}",
                    "Printing schema".log_step(),
                    $relation.log_name(),
                    self.script_create()
                );
            }
        }
    };
}
