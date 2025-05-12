use syn_parser::utils::LogStyle;
pub mod primary_nodes;

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
        $($field_name:ident: $dv:literal),+
        $(,)?
    }) => {
        pub struct $schema_name {
            $($field_name: CozoField),+
        }

        impl $schema_name {
            pub const SCHEMA: Self = Self {
                $($field_name: CozoField { st: stringify!($field_name), dv: $dv }),+
            };

            $(pub fn $field_name(&self) -> &str {
                self.$field_name.st()
            })*

        }

        impl $schema_name {
            pub fn schema_string(&self) -> String {
                let fields = vec![
                    $(format!("{}: {}", self.$field_name.st(), self.$field_name.dv())),+
                ];
                let relation_name = stringify!($schema_name).strip_suffix("NodeSchema")
                    .unwrap_or_else(|| {
                        log::error!(
                            "{} {}",
                            "Macro Error".log_error(),
                            "Name of Schema used in define_schema! must be [Name]NodeSchema"
                        );
                        panic!()
                }).to_lowercase();
                format!(":create {} {{ {} }}", relation_name, fields.join(", "))
            }
        }
    };
}
