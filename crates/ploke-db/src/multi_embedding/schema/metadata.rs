use crate::database::Database;
use crate::error::DbError;
use crate::multi_embedding::adapter::ExperimentalEmbeddingDbExt;
use cozo::ScriptMutability;
use std::collections::BTreeMap;

pub use ploke_transform::schema::multi_embedding::*;

/// Extension helpers for creating multi-embedding relations inside `ploke-db`.
pub trait ExperimentalRelationSchemaDbExt {
    fn ensure_registered(&self, db: &Database) -> Result<(), DbError>;
}

impl ExperimentalRelationSchemaDbExt for ExperimentalRelationSchema {
    fn ensure_registered(&self, db: &Database) -> Result<(), DbError> {
        match db.ensure_relation_registered(self.relation()) {
            Ok(()) => Ok(()),
            Err(DbError::ExperimentalRelationMissing { .. }) => db
                .run_script(
                    &self.script_create(),
                    BTreeMap::new(),
                    ScriptMutability::Mutable,
                )
                .map(|_| ())
                .map_err(|err| DbError::ExperimentalScriptFailure {
                    action: "schema_create",
                    relation: self.relation().to_string(),
                    details: err.to_string(),
                }),
            Err(other) => Err(other),
        }
    }
}
