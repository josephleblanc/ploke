#![cfg(feature = "multi_embedding_schema")]

//! Multi-embedding schema definitions.
//!
//! These relations mirror the legacy embedding layout but store per-node metadata about every
//! provider/model that has been written so we can dual-write during the migration.

use crate::error::TransformError;
use cozo::{DataValue, Db, MemStorage, ScriptMutability};
use itertools::Itertools;
use std::collections::BTreeMap;

/// Raw field definition used by the multi-embedding relations.
#[derive(Copy, Clone)]
pub struct CozoField {
    st: &'static str,
    dv: &'static str,
}

impl CozoField {
    const fn new(st: &'static str, dv: &'static str) -> Self {
        Self { st, dv }
    }

    fn st(&self) -> &str {
        self.st
    }

    fn dv(&self) -> &str {
        self.dv
    }
}

/// Schema definition for a multi-embedding relation.
///
/// These helpers keep the schema creation scripts colocated with the legacy schema definitions so
/// `ploke-transform` can create the relations during ingest while `ploke-db` reuses the metadata for
/// runtime helpers.
pub struct ExperimentalRelationSchema {
    relation: &'static str,
    fields: &'static [CozoField],
}

impl ExperimentalRelationSchema {
    pub fn relation(&self) -> &'static str {
        self.relation
    }

    pub fn field_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.fields.iter().map(CozoField::st)
    }

    fn script_identity(&self) -> String {
        let fields = self.fields.iter().map(CozoField::st).collect::<Vec<_>>();
        let keys = fields
            .iter()
            .copied()
            .filter(|f| ID_KEYWORDS.contains(f))
            .join(", ");
        let vals = fields
            .iter()
            .copied()
            .filter(|f| !ID_KEYWORDS.contains(f))
            .join(", ");
        format!("{} {{ {keys}, at => {vals} }}", self.relation)
    }

    pub fn script_create(&self) -> String {
        let fields = self
            .fields
            .iter()
            .map(|field| format!("{}: {}", field.st(), field.dv()))
            .collect::<Vec<_>>();
        let keys = fields
            .iter()
            .filter(|f| ID_VAL_KEYWORDS.contains(&f.as_str()))
            .join(", ");
        let vals = fields
            .iter()
            .filter(|f| !ID_VAL_KEYWORDS.contains(&f.as_str()))
            .join(", ");
        format!(
            ":create {} {{ {}, at: Validity => {} }}",
            self.relation, keys, vals
        )
    }

    pub fn script_put(&self, params: &BTreeMap<String, DataValue>) -> String {
        let lhs_keys = params
            .keys()
            .filter(|k| ID_KEYWORDS.contains(&k.as_str()))
            .join(", ");
        let lhs_entries = params
            .keys()
            .filter(|k| !ID_KEYWORDS.contains(&k.as_str()))
            .join(", ");
        let rhs_keys = params
            .keys()
            .filter(|k| ID_KEYWORDS.contains(&k.as_str()))
            .map(|k| format!("${}", k))
            .join(", ");
        let rhs_entries = params
            .keys()
            .filter(|k| !ID_KEYWORDS.contains(&k.as_str()))
            .map(|k| format!("${}", k))
            .join(", ");

        format!(
            "?[{lhs_keys}, at, {lhs_entries}] <- [[{rhs_keys}, 'ASSERT', {rhs_entries}]] :put {}",
            self.script_identity()
        )
    }
}

macro_rules! define_relation_schema {
    ($const_name:ident {
        $relation:literal,
        $($field_name:ident: $dv:literal),+ $(,)?
    }) => {
        pub const $const_name: ExperimentalRelationSchema = ExperimentalRelationSchema {
            relation: $relation,
            fields: &[
                $(CozoField::new(stringify!($field_name), $dv)),+
            ],
        };
    };
}

define_relation_schema!(FUNCTION_MULTI_EMBEDDING_SCHEMA {
    "function_multi_embedding",
    id: "Uuid",
    name: "String",
    docstring: "String?",
    vis_kind: "String",
    vis_path: "[String]?",
    span: "[Int; 2]",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    return_type_id: "Uuid?",
    body: "String?",
    module_id: "Uuid",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(CONST_MULTI_EMBEDDING_SCHEMA {
    "const_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    ty_id: "Uuid",
    value: "String?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(ENUM_MULTI_EMBEDDING_SCHEMA {
    "enum_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    variants: "[Uuid]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(IMPL_MULTI_EMBEDDING_SCHEMA {
    "impl_multi_embedding",
    id: "Uuid",
    self_type: "Uuid",
    span: "[Int; 2]",
    trait_type: "Uuid?",
    methods: "[Uuid]?",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(IMPORT_MULTI_EMBEDDING_SCHEMA {
    "import_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String?",
    vis_path: "[String]?",
    cfgs: "[String]",
    source_path: "[String]",
    visible_name: "String",
    original_name: "String?",
    is_glob: "Bool",
    is_self_import: "Bool",
    import_kind: "String",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(MACRO_MULTI_EMBEDDING_SCHEMA {
    "macro_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    body: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    kind: "String",
    proc_kind: "String?",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(MODULE_MULTI_EMBEDDING_SCHEMA {
    "module_multi_embedding",
    id: "Uuid",
    name: "String",
    path: "[String]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    span: "[Int; 2]",
    tracking_hash: "Uuid",
    module_kind: "String",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(STATIC_MULTI_EMBEDDING_SCHEMA {
    "static_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    ty_id: "Uuid",
    is_mutable: "Bool",
    value: "String?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(STRUCT_MULTI_EMBEDDING_SCHEMA {
    "struct_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(TRAIT_MULTI_EMBEDDING_SCHEMA {
    "trait_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    methods: "[Uuid]?",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(TYPE_ALIAS_MULTI_EMBEDDING_SCHEMA {
    "type_alias_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    ty_id: "Uuid",
    embeddings: "[(String, Int)]"
});

define_relation_schema!(UNION_MULTI_EMBEDDING_SCHEMA {
    "union_multi_embedding",
    id: "Uuid",
    name: "String",
    span: "[Int; 2]",
    vis_kind: "String",
    vis_path: "[String]?",
    docstring: "String?",
    tracking_hash: "Uuid",
    cfgs: "[String]?",
    embeddings: "[(String, Int)]"
});

const ID_KEYWORDS: [&str; 9] = [
    "id",
    "function_id",
    "owner_id",
    "source_id",
    "target_id",
    "type_id",
    "node_id",
    "embedding_model",
    "provider",
];
const ID_VAL_KEYWORDS: [&str; 9] = [
    "id: Uuid",
    "function_id: Uuid",
    "owner_id: Uuid",
    "source_id: Uuid",
    "target_id: Uuid",
    "type_id: Uuid",
    "node_id: Uuid",
    "embedding_model: String",
    "provider: String",
];

const METADATA_SCHEMAS: &[ExperimentalRelationSchema] = &[
    FUNCTION_MULTI_EMBEDDING_SCHEMA,
    CONST_MULTI_EMBEDDING_SCHEMA,
    ENUM_MULTI_EMBEDDING_SCHEMA,
    IMPL_MULTI_EMBEDDING_SCHEMA,
    IMPORT_MULTI_EMBEDDING_SCHEMA,
    MACRO_MULTI_EMBEDDING_SCHEMA,
    MODULE_MULTI_EMBEDDING_SCHEMA,
    STATIC_MULTI_EMBEDDING_SCHEMA,
    STRUCT_MULTI_EMBEDDING_SCHEMA,
    TRAIT_MULTI_EMBEDDING_SCHEMA,
    TYPE_ALIAS_MULTI_EMBEDDING_SCHEMA,
    UNION_MULTI_EMBEDDING_SCHEMA,
];

/// Creates every multi-embedding metadata relation.
pub fn create_multi_embedding_relations(db: &Db<MemStorage>) -> Result<(), TransformError> {
    for schema in METADATA_SCHEMAS {
        db.run_script(
            &schema.script_create(),
            BTreeMap::new(),
            ScriptMutability::Mutable,
        )?;
    }
    Ok(())
}
