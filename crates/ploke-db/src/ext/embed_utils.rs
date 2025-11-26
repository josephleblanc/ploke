use cozo::DataValue;
use itertools::Itertools;
use ploke_core::EmbeddingSetId;
use ploke_error::Error as PlokeError;

use crate::database::to_uuid;
use crate::multi_embedding::vectors::CozoVectorExt;
use crate::{Database, DbError, NodeType, QueryResult};

#[cfg(feature = "multi_embedding")]
use cozo::ScriptMutability;
#[cfg(feature = "multi_embedding")]
use std::collections::{BTreeMap, HashSet};
#[cfg(feature = "multi_embedding")]
use uuid::Uuid;

/// An extension trait for [`Database`] that provides convenience methods
/// for common embedding-related queries and operations.
pub trait DbEmbedUtils {
    /// Gets rows from a specific node relation where the embedding is null,
    /// filtered by the node's `name`.
    ///
    /// This is a helper to quickly check the embedding status of a specific item
    /// without writing a full CozoScript query.
    fn get_null_embedding_rows(
        &self,
        name: &str,
        node_ty: NodeType,
    ) -> Result<QueryResult, PlokeError>;

    fn upsert_vector_values(
        &self,
        embedding_data: EmbeddingInsert
    ) -> Result<(), DbError>;
}

#[derive(Debug, Clone)]
pub struct EmbeddingInsert {
    pub node_id: Uuid,
    pub emb_set: EmbeddingSetId,
    pub vector: Vec< f32 >,
}

impl DbEmbedUtils for Database {
    fn get_null_embedding_rows(
        &self,
        name: &str,
        node_ty: NodeType,
    ) -> std::result::Result<QueryResult, ploke_error::Error> {
        let script = build_null_embed_script(name, node_ty);

        let rows = self.raw_query(&script).map_err(PlokeError::from)?;
        Ok(rows)
    }

    /// Either creates a row or updates a row of vector embedding values linked to a given node_id
    /// with the provided vector value, at the relation derived from the `EmbeddingSetId`
    fn upsert_vector_values(
        &self,
        embedding_data: EmbeddingInsert
    ) -> Result<(), DbError> {
        let EmbeddingInsert {
            node_id,
            emb_set,
            vector,
        } = embedding_data;
        if vector.len() != emb_set.dims() as usize {
            return Err(DbError::ExperimentalVectorLengthMismatch {
                expected: emb_set.dims() as usize,
                actual: vector.len(),
            });
        }
        let to_cozo_uuid = |u: Uuid| -> DataValue { DataValue::Uuid(cozo::UuidWrapper(u)) };
        let vec_dv = vector
            .iter()
            .map(|v| DataValue::from(*v as f64))
            .collect_vec();
        let params = BTreeMap::from([
            ("node_id".to_string(), to_cozo_uuid(node_id)),
            ("embedding_model".to_string(), emb_set.model.as_ref().into()),
            ("provider".to_string(), emb_set.provider.as_ref().into()),
            ("vector".to_string(), DataValue::List(vec_dv)),
            (
                "embedding_dims".to_string(),
                DataValue::from(emb_set.dims() as i64),
            ),
        ]);
        let identity = emb_set.script_identity();
        let script = format!(
            r#"
?[node_id, embedding_model, provider, at, embedding_dims, vector] <- [[
    $node_id,
    $embedding_model,
    $provider,
    'ASSERT',
    $embedding_dims,
    $vector
]] :put {identity}
"#
        );
        self.run_script(&script, params, ScriptMutability::Mutable)
            .map_err(|err| DbError::EmbeddingScriptFailure {
                action: "insert_vector_row",
                relation: emb_set.relation_name().clone(),
                details: err.to_string(),
            })?;
        Ok(())
    }
}

/// Constructs the CozoScript query to check for a null embedding on a named item.
fn build_null_embed_script(name: &str, node_ty: NodeType) -> String {
    let ty = node_ty.relation_str();
    format!(
        "?[name, is_null_embedding] :=
        *{ty}{{name, embedding @ 'NOW' }},
        name = \"{name}\",
        is_null_embedding = is_null(embedding)
        "
    )
}

/// An extension trait for [`QueryResult`] that provides helper methods for
/// interpreting results from embedding-related queries.
pub trait HasNullEmbeds {
    /// Checks if the query result indicates a null embedding.
    ///
    /// This is designed to work with the specific output of `get_null_embedding_rows`,
    /// which returns a boolean in the second column of the first row.
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the embedding is null, `Ok(false)` if it is not.
    ///
    /// # Errors
    ///
    /// Returns a `ploke_error::Error` wrapping `DbError::NotFound` if the query
    /// result is empty or does not have the expected shape.
    fn is_null_embedding(&self) -> Result<bool, PlokeError>;
}

impl HasNullEmbeds for QueryResult {
    fn is_null_embedding(&self) -> Result<bool, PlokeError> {
        let is_null_embed = self
            .rows
            .first() // Use .first() to avoid panicking on empty rows
            .and_then(|row| row.get(1))
            .and_then(|v| v.get_bool())
            .ok_or(DbError::NotFound)?;
        Ok(is_null_embed)
    }
}

/// Helpers for computing embedded/pending state when embeddings live only
/// in vector relations (no metadata dual-write).
#[cfg(feature = "multi_embedding")]
pub trait VectorEmbeddingStatusExt {
    /// Returns the set of node IDs that have at least one vector row in any of
    /// the provided vector relations.
    fn embedded_ids_for_vector(&self, emb_set: EmbeddingSetId) -> Result<HashSet<Uuid>, DbError>;

    /// Convenience: counts pending nodes for all types of primary nodes for a given embedding.
    fn count_embedded_for_vector(&self, emb_set: EmbeddingSetId) -> Result<usize, DbError> {
        Ok(self.embedded_ids_for_vector(emb_set)?.len())
    }

    /// Returns the node IDs for the given primary node type that are still
    /// missing vectors across the provided vector relations.
    fn pending_ids_for_type(
        &self,
        node_type: NodeType,
        emb_set: EmbeddingSetId,
    ) -> Result<HashSet<Uuid>, DbError>;

    /// Convenience: counts pending nodes for the given type.
    fn count_pending_for_type(
        &self,
        node_type: NodeType,
        emb_set: EmbeddingSetId,
    ) -> Result<usize, DbError> {
        Ok(self.pending_ids_for_type(node_type, emb_set)?.len())
    }
}

#[cfg(feature = "multi_embedding")]
impl VectorEmbeddingStatusExt for Database {
    fn embedded_ids_for_vector(&self, emb_set: EmbeddingSetId) -> Result<HashSet<Uuid>, DbError> {
        let mut ids = HashSet::new();
        let script = format!(
            r#"
?[node_id] :=
    *{rel}{{ node_id @ 'NOW' }}
"#,
            rel = emb_set.relation_name(),
        );
        let rows = self
            .run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(DbError::from)?;
        for row in rows.rows {
            if let Some(value) = row.first() {
                ids.insert(to_uuid(value)?);
            }
        }
        Ok(ids)
    }

    fn pending_ids_for_type(
        &self,
        node_type: NodeType,
        emb_set: EmbeddingSetId,
    ) -> Result<HashSet<Uuid>, DbError> {
        let mut pending_ids = HashSet::new();
        let emb_rel = &emb_set.rel_name;
        let script = format!(
            r#"
?[node_id] :=
    *{rel}{{ node_id @ 'NOW' }},
    *{emb_rel}{{ node_id @ 'NOW' }}
"#,
            rel = node_type.relation_str(),
            emb_rel = node_type.relation_str(),
        );
        let rows = self
            .run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(DbError::from)?;

        let pending_iter = rows
            .into_iter()
            .filter_map(|r| r.first().and_then(|id| to_uuid(id).ok()));
        pending_ids.extend(pending_iter);

        Ok(pending_ids)
    }
}
