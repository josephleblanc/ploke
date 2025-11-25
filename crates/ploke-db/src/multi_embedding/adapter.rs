use super::HnswDistance;
use std::collections::{BTreeMap, HashSet};

use crate::multi_embedding::HnswEmbedInfo;
use crate::{database::Database, multi_embedding::schema::vector_dims::HnswRelName};
use crate::error::DbError;
use cozo::{DataValue, Db, MemStorage, NamedRows, Num, ScriptMutability};
use ploke_core::{embedding_set::EmbRelName, EmbeddingDType, EmbeddingSetId};
use std::ops::Deref;
use uuid::Uuid;

pub trait EmbeddingDbExt {
    fn ensure_relation_registered(&self, relation_name: EmbRelName) -> Result<(), DbError>;

    /// verifies vector dims are of expected length
    fn assert_vector_column_layout(&self, relation_name: EmbRelName, dims: i64) -> Result<(), DbError>;

    /// Lists the models that have been used to generate vector embeddings currently present in the
    /// database, along with their dimensions.
    fn enumerate_vector_models(
        &self,
        relation_name: EmbRelName,
    ) -> Result<HashSet<(String, i64)>, DbError>;

    /// Gets the vector embedding rows for the given node_id.
    fn vector_rows_from_id(&self, relation_name: EmbRelName, node_id: Uuid)
        -> Result<NamedRows, DbError>;
}

impl EmbeddingDbExt for Db<MemStorage> {
    fn ensure_relation_registered(&self, relation_name: EmbRelName) -> Result<(), DbError> {
        let rows = self
            .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::EmbeddingScriptFailure {
                action: "relations_lookup",
                relation: relation_name.clone(),
                details: err.to_string(),
            })?;
        let found = rows.rows.iter().any(|row| {
            row.iter().any(|value| {
                value
                    .get_str()
                    .map(|name| name == relation_name.as_ref())
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

    fn assert_vector_column_layout(&self, relation_name: EmbRelName, dims: i64) -> Result<(), DbError> {
        let script = format!("::columns {}", relation_name);
        let rows = self
            .run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::EmbeddingScriptFailure {
                action: "columns_lookup",
                relation: relation_name.clone(),
                details: err.to_string(),
            })?;
        let mut matches = 0;
        for row in &rows.rows {
            let column_name = row
                .first()
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
                relation: relation_name,
                dims: dims as u32,
            })
        }
    }

    fn enumerate_vector_models(
        &self,
        relation_name: EmbRelName,
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
            .map_err(|err| DbError::EmbeddingScriptFailure {
                action: "vector_query",
                relation: relation_name.clone(),
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

    fn vector_rows_from_id(
        &self,
        relation_name: EmbRelName,
        node_id: Uuid,
    ) -> Result<NamedRows, DbError> {
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
            .map_err(|err| DbError::EmbeddingScriptFailure {
                action: "vector_rows",
                relation: relation_name,
                details: err.to_string(),
            })
    }
}

impl EmbeddingDbExt for Database {
    fn ensure_relation_registered(&self, relation_name: EmbRelName) -> Result<(), DbError> {
        <Db<MemStorage> as EmbeddingDbExt>::ensure_relation_registered(self.deref(), relation_name)
    }

    fn assert_vector_column_layout(&self, relation_name: EmbRelName, dims: i64) -> Result<(), DbError> {
        <Db<MemStorage> as EmbeddingDbExt>::assert_vector_column_layout(
            self.deref(),
            relation_name,
            dims,
        )
    }

    fn enumerate_vector_models(
        &self,
        relation_name: EmbRelName,
    ) -> Result<HashSet<(String, i64)>, DbError> {
        <Db<MemStorage> as EmbeddingDbExt>::enumerate_vector_models(self.deref(), relation_name)
    }

    fn vector_rows_from_id(
        &self,
        relation_name: EmbRelName,
        node_id: Uuid,
    ) -> Result<NamedRows, DbError> {
        <Db<MemStorage> as EmbeddingDbExt>::vector_rows_from_id(self.deref(), relation_name, node_id)
    }
}

pub trait IndexDbExt: EmbeddingDbExt {
    fn create_idx(
        &self,
        hnsw_info: HnswEmbedInfo,
    ) -> Result<(), DbError>;
    fn search_embeddings_hnsw(
        &self,
        hnsw_rel_name: HnswRelName,
        query_literal: &str,
        k: i64,
        ef: i64,
    ) -> Result<NamedRows, DbError>;
    fn vector_metadata_rows(
        &self,
        relation_name: EmbRelName,
        node_id: Uuid,
    ) -> Result<NamedRows, DbError>;
}

pub(crate) fn parse_embedding_metadata(value: &DataValue) -> Result<Vec<(String, i64)>, DbError> {
    let entries = value
        .get_slice()
        .ok_or_else(|| DbError::ExperimentalMetadataParse {
            reason: "embeddings column should contain a list".into(),
        })?;
    let mut parsed = Vec::new();
    for entry in entries {
        let tuple = entry
            .get_slice()
            .ok_or_else(|| DbError::ExperimentalMetadataParse {
                reason: "embedding metadata tuple should be a list".into(),
            })?;
        if tuple.len() != 2 {
            return Err(DbError::ExperimentalMetadataParse {
                reason: "embedding metadata tuples must be (model, dims)".into(),
            });
        }
        let model = tuple[0]
            .get_str()
            .ok_or_else(|| DbError::ExperimentalMetadataParse {
                reason: "tuple[0] should be embedding model string".into(),
            })?;
        let dims = match &tuple[1] {
            DataValue::Num(Num::Int(val)) => *val,
            other => {
                return Err(DbError::ExperimentalMetadataParse {
                    reason: format!("tuple[1] must be integer dimensions, got {other:?}"),
                })
            }
        };
        parsed.push((model.to_string(), dims));
    }
    Ok(parsed)
}

impl IndexDbExt for Database {
    fn create_idx(
        &self,
        hnsw_set: HnswEmbedInfo
    ) -> Result<(), DbError> {
        // NOTE: The [vector] part of the script below indicates that this query expects that the
        // target relation will have a field called "vector", which will contain the vector of the
        // expected dim and dtype.
        let script = format!(
            r#"
::hnsw create {hnsw_rel_name} {{
    fields: [vector],
    dim: {dims},
    dtype: F32,
    m: {m},
    ef_construction: {ef_construction},
    distance: {distance}
}}
"#,
            hnsw_rel_name = hnsw_set.hnsw_rel_name(),
            dims = hnsw_set.dims(),
            m = hnsw_set.hnsw_m(),
            ef_construction = hnsw_set.hnsw_ef_construction(),
            distance = hnsw_set.distance(),
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|err| DbError::HnswEmbeddingScriptFailure {
                action: "create_idx",
                relation: hnsw_set.hnsw_rel_name().clone(),
                details: err.to_string(),
            })
    }

    fn search_embeddings_hnsw(
        &self,
        hnsw_rel_name: HnswRelName,
        query_literal: &str,
        k: i64,
        ef: i64,
    ) -> Result<NamedRows, DbError> {
        let script = format!(
            r#"
?[node_id, distance] :=
    ~{hnsw_rel_name} {{ node_id |
        query: {query},
        k: {k},
        ef: {ef},
        bind_distance: distance
    }}
"#,
            query = query_literal,
            k = k,
            ef = ef,
        );
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::HnswEmbeddingScriptFailure { 
                action: "search_embeddings_hnsw",
                relation: hnsw_rel_name,
                details: err.to_string(),
            })
    }

    fn vector_metadata_rows(
        &self,
        relation_name: EmbRelName,
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
            .map_err(|err| DbError::EmbeddingScriptFailure {
                action: "vector_metadata_rows",
                relation: relation_name,
                details: err.to_string(),
            })
    }
}
