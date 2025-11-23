use ploke_error::Error as PlokeError;

use crate::database::to_uuid;
use crate::{Database, DbError, NodeType, QueryResult};

#[cfg(feature = "multi_embedding")]
use crate::multi_embedding::vectors::ExperimentalVectorRelation;
#[cfg(feature = "multi_embedding")]
use cozo::ScriptMutability;
#[cfg(feature = "multi_embedding")]
use std::collections::{BTreeMap, HashSet};
#[cfg(feature = "multi_embedding")]
use uuid::Uuid;

/// An extension trait for [`Database`] that provides convenience methods
/// for common embedding-related queries.
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
}

/// Constructs the CozoScript query to check for a null embedding on a named item.
fn build_null_embed_script(name: &str, node_ty: NodeType) -> String {
    let ty = node_ty.relation_str();
    todo!("Need to update the CozoScript used here, or decide on alternative if this method does not fit into the new single-write approach to vectors");
    if cfg!(feature = "multi_embedding") {
        format!(
            "?[name, is_null_embedding] :=
            *{ty}{{name, embedding @ 'NOW' }},
            name = \"{name}\",
            is_null_embedding = is_null(embedding)
            "
        )
    } else {
        format!(
            "?[name, is_null_embedding] :=
            *{ty}{{name, embedding @ 'NOW' }},
            name = \"{name}\",
            is_null_embedding = is_null(embedding)
            "
        )
    }
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
    fn embedded_ids_for_vectors(
        &self,
        vector_relations: &[ExperimentalVectorRelation],
    ) -> Result<HashSet<Uuid>, DbError>;

    /// Returns the node IDs for the given primary node type that are still
    /// missing vectors across the provided vector relations.
    fn pending_ids_for_type(
        &self,
        node_type: NodeType,
        vector_relations: &[ExperimentalVectorRelation],
    ) -> Result<HashSet<Uuid>, DbError>;

    /// Convenience: counts pending nodes for the given type.
    fn count_pending_for_type(
        &self,
        node_type: NodeType,
        vector_relations: &[ExperimentalVectorRelation],
    ) -> Result<usize, DbError> {
        Ok(self
            .pending_ids_for_type(node_type, vector_relations)?
            .len())
    }
}

#[cfg(feature = "multi_embedding")]
impl VectorEmbeddingStatusExt for Database {
    fn embedded_ids_for_vectors(
        &self,
        vector_relations: &[ExperimentalVectorRelation],
    ) -> Result<HashSet<Uuid>, DbError> {
        let mut ids = HashSet::new();
        for relation in vector_relations {
            let script = format!(
                r#"
?[node_id] :=
    *{rel}{{ node_id @ 'NOW' }}
"#,
                rel = relation.relation_name(),
            );
            let rows = self
                .run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
                .map_err(DbError::from)?;
            for row in rows.rows {
                if let Some(value) = row.first() {
                    ids.insert(to_uuid(value)?);
                }
            }
        }
        Ok(ids)
    }

    fn pending_ids_for_type(
        &self,
        node_type: NodeType,
        vector_relations: &[ExperimentalVectorRelation],
    ) -> Result<HashSet<Uuid>, DbError> {
        let mut base_ids = HashSet::new();
        let script = format!(
            r#"
?[id] :=
    *{rel}{{ id @ 'NOW' }}
"#,
            rel = node_type.relation_str(),
        );
        let rows = self
            .run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(DbError::from)?;
        for row in rows.rows {
            if let Some(value) = row.first() {
                base_ids.insert(to_uuid(value)?);
            }
        }

        let embedded = self.embedded_ids_for_vectors(vector_relations)?;
        base_ids.retain(|id| !embedded.contains(id));
        Ok(base_ids)
    }
}

pub struct NewQueryBuilder {
    // e.g. some kind of join/select
    first_set: Vec<String>,
    // e.g. some kind of limit on # of returned items
    second_set: Vec<String>,
    // ..etc
}
