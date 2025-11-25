use crate::database::Database;
use crate::error::DbError;
use crate::multi_embedding::adapter::EmbeddingDbExt;
use crate::NodeType;
use cozo::ScriptMutability;
use std::collections::BTreeMap;

pub use ploke_transform::schema::multi_embedding::*;

/// Extension helpers for creating multi-embedding relations inside `ploke-db`.
pub trait ExperimentalRelationSchemaDbExt {
    fn ensure_registered(&self, db: &Database) -> Result<(), DbError>;
}

impl ExperimentalRelationSchemaDbExt for ExperimentalRelationSchema {
    fn ensure_registered(&self, db: &Database) -> Result<(), DbError> {
        match db.ensure_relation_registered(self.typed_relation()) {
            Ok(()) => Ok(()),
            Err(DbError::ExperimentalRelationMissing { .. }) => db
                .run_script(
                    &self.script_create(),
                    BTreeMap::new(),
                    ScriptMutability::Mutable,
                )
                .map(|_| ())
                .map_err(|err| DbError::EmbeddingScriptFailure {
                    action: "schema_create",
                    relation: self.typed_relation(),
                    details: err.to_string(),
                }),
            Err(other) => Err(other),
        }
    }
}

pub fn experimental_spec_for_node(ty: NodeType) -> std::option::Option<ploke_transform::schema::ExperimentalRelationSchema> {
        use NodeType::*;
    match ty {
        Function => Some(FUNCTION_MULTI_EMBEDDING_SCHEMA),
        Const => Some(CONST_MULTI_EMBEDDING_SCHEMA),
        Enum => Some(ENUM_MULTI_EMBEDDING_SCHEMA),
        Struct => Some(STRUCT_MULTI_EMBEDDING_SCHEMA),
        Trait => Some(TRAIT_MULTI_EMBEDDING_SCHEMA),
        Module => Some(MODULE_MULTI_EMBEDDING_SCHEMA),
        Macro => Some(MACRO_MULTI_EMBEDDING_SCHEMA),
        Static => Some(STATIC_MULTI_EMBEDDING_SCHEMA),
        TypeAlias => Some(TYPE_ALIAS_MULTI_EMBEDDING_SCHEMA),
        Union => Some(UNION_MULTI_EMBEDDING_SCHEMA),
        _ => None,
    }
}
