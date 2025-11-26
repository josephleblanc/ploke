use std::collections::BTreeMap;

use cozo::DataValue;
use itertools::Itertools;
use ploke_core::embeddings::{EmbRelName, EmbeddingSet, EmbeddingSetId};

use crate::{Database, DbError, NodeType};

/// Trait used to extend the database with embeddings-aware methods
pub trait EmbeddingExt {
    /// Counts the code primary node code items that have not yet been embedded.
    ///
    /// Queries the underlying database to determine which nodes have been embedded or not by the
    /// presence/absence of an associated vector for the given embedding set (identified by the
    /// embedding_set_id).
    ///
    /// In the case of nodes being processed into vector embeddings, this function can be used to
    /// determine which nodes have not yet been embedded, while some may already have been
    /// embedded.
    ///
    /// Useful in `ploke-embed` when processing vector embeddings.
    fn count_pending_embeddings(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError>;

    /// Helper function to specifically count unembedded non-files.
    ///
    /// Similar to `count_pending_embeddings`, it is useful when processing vector embeddings in
    /// `ploke-embed`.
    fn count_unembedded_nonfiles(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError>;

    /// Helper function to specifically count unembedded files.
    ///
    // Similar to `count_pending_embeddings`, it is useful when processing vector embeddings in
    // `ploke-embed`
    fn count_unembedded_files(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError>;

    /// Checks for the presence of the embedding info for a given embedding set.
    fn is_embedding_present(&self, embedding_set_id: &EmbeddingSet) -> Result<bool, DbError>;

    fn is_embedding_id_present(&self, embedding_set_id: EmbeddingSetId) -> Result<bool, DbError>;

    fn get_embedding_rel_name(
        &self,
        embedding_set_id: EmbeddingSetId,
    ) -> Result<EmbRelName, DbError>;
}

impl EmbeddingExt for cozo::Db<cozo::MemStorage> {
    fn count_pending_embeddings(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError> {
        // 1. Get rel_name from embedding_set_id
        // 2. Build the main query
        let all_node_types_unembedded_rules = NodeType::primary_nodes()
            .iter()
            .map(|node_type| {
                format!(
                    "unembedded[id] := *{node_rel}{{id}}, !*{embed_rel}{{node_id: id}}",
                    node_rel = node_type.relation_str(),
                    embed_rel = embedding_set_id.rel_name
                )
            })
            .join("\n");

        let query = format!(
            "{}\n?[count(id)] := unembedded[id]",
            all_node_types_unembedded_rules
        );

        let result = self
            .run_script(
                &query,
                Default::default(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        Database::into_usize(result)
    }

    fn count_unembedded_nonfiles(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError> {
        let nodes = self.count_pending_embeddings(embedding_set)?;
        let files = self.count_unembedded_files(embedding_set)?;
        let count = nodes.checked_sub(files).ok_or_else(|| {
            DbError::QueryExecution(
                "Invariant violated: more unembedded files than unembedded nodes".into(),
            )
        })?;
        Ok(count)
    }

    fn count_unembedded_files(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError> {
        let query = format!(
            r#"
unembedded_file_mod[id] := *module{{id}}, *file_mod{{owner_id: id}}, 
!*{embed_rel}{{node_id: id}}

?[count(id)] := unembedded_file_mod[id]
"#,
            embed_rel = embedding_set.rel_name
        );

        let result = self
            .run_script(
                &query,
                Default::default(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        Database::into_usize(result)
    }

    fn is_embedding_present(&self, embedding_set: &EmbeddingSet) -> Result<bool, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "embedding_set_id".into(),
            cozo::DataValue::from(embedding_set.hash_id.into_inner() as i64),
        );

        let get_rel_name_script =
            "?[rel_name] := *embedding_set{id: $embedding_set_id, rel_name @ 'NOW'}";
        let result = self
            .run_script(
                get_rel_name_script,
                params,
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        result
            .rows
            .first()
            .and_then(|row| row.first())
            .map(|v| v.get_str().is_some())
            .ok_or(DbError::NotFound)
    }

    fn is_embedding_id_present(&self, embedding_set_id: EmbeddingSetId) -> Result<bool, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "embedding_set_id".into(),
            cozo::DataValue::from(embedding_set_id.into_inner() as i64),
        );

        let get_rel_name_script =
            "?[rel_name] := *embedding_set{id: $embedding_set_id, rel_name @ 'NOW'}";
        let result = self
            .run_script(
                get_rel_name_script,
                params,
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        result
            .rows
            .first()
            .and_then(|row| row.first())
            .map(|v| v.get_str().is_some())
            .ok_or(DbError::NotFound)
    }

    fn get_embedding_rel_name(
        &self,
        embedding_set_id: EmbeddingSetId,
    ) -> Result<EmbRelName, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "embedding_set_id".into(),
            cozo::DataValue::from(embedding_set_id.into_inner() as i64),
        );

        let get_rel_name_script =
            "?[rel_name] := *embedding_set{id: $embedding_set_id, rel_name @ 'NOW'}";
        let result = self
            .run_script(
                get_rel_name_script,
                params,
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        result
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|v| v.get_str())
            .map(EmbRelName::new_from_str)
            .ok_or(DbError::NotFound)
    }
}
