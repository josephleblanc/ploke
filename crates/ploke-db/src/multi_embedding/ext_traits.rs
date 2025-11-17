use super::*;

use std::collections::{BTreeMap, HashSet};

use crate::{database::Database, multi_embedding::parse_embedding_metadata};
use crate::error::DbError;
use crate::NodeType;
use cozo::{self, DataValue, Db, MemStorage, NamedRows, Num, ScriptMutability, UuidWrapper};
use itertools::Itertools;
use lazy_static::lazy_static;
use std::ops::Deref;
use uuid::Uuid;

pub trait ExperimentalEmbeddingDbExt {
    fn ensure_relation_registered(&self, relation_name: &str) -> Result<(), DbError>;
    fn assert_vector_column_layout(&self, relation_name: &str, dims: i64) -> Result<(), DbError>;
    fn enumerate_metadata_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError>;
    fn enumerate_vector_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError>;
}

impl ExperimentalEmbeddingDbExt for Db<MemStorage> {
    fn ensure_relation_registered(&self, relation_name: &str) -> Result<(), DbError> {
        let rows = self
            .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "relations_lookup",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })?;
        let found = rows.rows.iter().any(|row| {
            row.iter().any(|value| {
                value
                    .get_str()
                    .map(|name| name == relation_name)
                    .unwrap_or(false)
            })
        });
        if found {
            Ok(())
        } else {
            Err(DbError::ExperimentalRelationMissing {
                relation: relation_name.to_string(),
            })
        }
    }

    fn assert_vector_column_layout(&self, relation_name: &str, dims: i64) -> Result<(), DbError> {
        let script = format!("::columns {}", relation_name);
        let rows = self
            .run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "columns_lookup",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })?;
        let mut matches = 0;
        for row in &rows.rows {
            let column_name = row
                .get(0)
                .and_then(DataValue::get_str)
                .map(|s| s == "vector")
                .unwrap_or(false);
            let column_type = row
                .get(3)
                .and_then(DataValue::get_str)
                .map(|s| s == format!("<F32;{dims}>"))
                .unwrap_or(false);
            if column_name && column_type {
                matches += 1;
            }
        }
        if matches == 1 {
            Ok(())
        } else {
            Err(DbError::ExperimentalVectorLayoutMismatch {
                relation: relation_name.to_string(),
                dims,
            })
        }
    }

    fn enumerate_metadata_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError> {
        let query = format!(
            r#"
?[embeddings] :=
    *{rel}{{ embeddings @ 'NOW' }}
"#,
            rel = relation_name,
        );
        let rows = self
            .run_script(&query, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "metadata_query",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })?;
        let mut values = HashSet::new();
        for row in &rows.rows {
            for entry in parse_embedding_metadata(&row[0])? {
                values.insert(entry);
            }
        }
        Ok(values)
    }

    fn enumerate_vector_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError> {
        let query = format!(
            r#"
?[embedding_model, embedding_dims] :=
    *{rel}{{ embedding_model, embedding_dims @ 'NOW' }}
"#,
            rel = relation_name,
        );
        let rows = self
            .run_script(&query, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "vector_query",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })?;
        let mut entries = HashSet::new();
        for row in &rows.rows {
            let model = row[0]
                .get_str()
                .ok_or_else(|| DbError::ExperimentalMetadataParse {
                    reason: format!(
                        "embedding_model should be string for relation {relation_name}"
                    ),
                })?
                .to_string();
            let dims = match &row[1] {
                DataValue::Num(Num::Int(val)) => *val,
                other => {
                    return Err(DbError::ExperimentalMetadataParse {
                        reason: format!(
                            "embedding_dims must be integer for relation {relation_name}, got {other:?}"
                        ),
                    })
                }
            };
            entries.insert((model, dims));
        }
        Ok(entries)
    }
}

impl ExperimentalEmbeddingDbExt for Database {
    fn ensure_relation_registered(&self, relation_name: &str) -> Result<(), DbError> {
        <Db<MemStorage> as ExperimentalEmbeddingDbExt>::ensure_relation_registered(
            self.deref(),
            relation_name,
        )
    }

    fn assert_vector_column_layout(&self, relation_name: &str, dims: i64) -> Result<(), DbError> {
        <Db<MemStorage> as ExperimentalEmbeddingDbExt>::assert_vector_column_layout(
            self.deref(),
            relation_name,
            dims,
        )
    }

    fn enumerate_metadata_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError> {
        <Db<MemStorage> as ExperimentalEmbeddingDbExt>::enumerate_metadata_models(
            self.deref(),
            relation_name,
        )
    }

    fn enumerate_vector_models(
        &self,
        relation_name: &str,
    ) -> Result<HashSet<(String, i64)>, DbError> {
        <Db<MemStorage> as ExperimentalEmbeddingDbExt>::enumerate_vector_models(
            self.deref(),
            relation_name,
        )
    }
}

pub trait ExperimentalEmbeddingDatabaseExt: ExperimentalEmbeddingDbExt {
    fn create_idx(
        &self,
        relation_name: &str,
        dims: i64,
        m: i64,
        ef_construction: i64,
        distance: HnswDistance,
    ) -> Result<(), DbError>;
    fn search_embeddings_hnsw(
        &self,
        relation_name: &str,
        query_literal: &str,
        k: i64,
        ef: i64,
    ) -> Result<NamedRows, DbError>;
    fn vector_rows(&self, relation_name: &str, node_id: Uuid) -> Result<NamedRows, DbError>;
    fn vector_metadata_rows(
        &self,
        relation_name: &str,
        node_id: Uuid,
    ) -> Result<NamedRows, DbError>;
}

impl ExperimentalEmbeddingDatabaseExt for Database {
    fn create_idx(
        &self,
        relation_name: &str,
        dims: i64,
        m: i64,
        ef_construction: i64,
        distance: HnswDistance,
    ) -> Result<(), DbError> {
        let script = format!(
            r#"
::hnsw create {rel}:vector_idx {{
    fields: [vector],
    dim: {dims},
    dtype: F32,
    m: {m},
    ef_construction: {ef_construction},
    distance: {distance}
}}
"#,
            rel = relation_name,
            dims = dims,
            m = m,
            ef_construction = ef_construction,
            distance = distance.as_str(),
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "create_idx",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })
    }

    fn search_embeddings_hnsw(
        &self,
        relation_name: &str,
        query_literal: &str,
        k: i64,
        ef: i64,
    ) -> Result<NamedRows, DbError> {
        let script = format!(
            r#"
?[node_id, distance] :=
    ~{rel}:vector_idx{{ node_id |
        query: {query},
        k: {k},
        ef: {ef},
        bind_distance: distance
    }}
"#,
            rel = relation_name,
            query = query_literal,
            k = k,
            ef = ef,
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "search_embeddings_hnsw",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })
    }

    fn vector_rows(&self, relation_name: &str, node_id: Uuid) -> Result<NamedRows, DbError> {
        let script = format!(
            r#"
?[embedding_model, provider, embedding_dims, vector] :=
    *{rel}{{ node_id, embedding_model, provider, embedding_dims, vector @ 'NOW' }},
    node_id = to_uuid("{node_id}")
"#,
            rel = relation_name,
            node_id = node_id,
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "vector_rows",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })
    }

    fn vector_metadata_rows(
        &self,
        relation_name: &str,
        node_id: Uuid,
    ) -> Result<NamedRows, DbError> {
        let script = format!(
            r#"
?[embeddings] :=
    *{rel}{{ id, embeddings @ 'NOW' }},
    id = to_uuid("{node_id}")
"#,
            rel = relation_name,
            node_id = node_id,
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "vector_metadata_rows",
                relation: relation_name.to_string(),
                details: err.to_string(),
            })
    }
}
