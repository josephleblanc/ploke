use std::collections::BTreeMap;

use cozo::DataValue;
use itertools::Itertools;
use ploke_core::embeddings::{EmbRelName, EmbeddingSet, EmbeddingSetId};
use tracing::info;

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
    fn count_pending_embeddings(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError> {
        let rel_name = embedding_set.rel_name.to_string().replace('-', "_");
        let conditions = NodeType::primary_nodes()
            .iter()
            .map(|node_type| {
                format!(
                    "(*{node_rel}{{id}}, not *{embed_rel}{{node_id: id}})",
                    node_rel = node_type.relation_str(),
                    embed_rel = &rel_name
                )
            })
            .join(" or ");

        let query = format!("?[count(id)] := {}", conditions);

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
            embed_rel = embedding_set.rel_name.to_string().replace('-', "_")
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

#[cfg(test)]
mod tests {
    // #[cfg(feature = "multi_embedding_db")]
    // fn setup_db() -> Result<(), ploke_error::Error> {
    //     ploke_test_utils::setup_db_full("fixture_nodes")?;
    // }

    use std::collections::BTreeMap;

    use super::*;
    use cozo::{MemStorage, ScriptMutability};
    use ploke_core::embeddings::{
        EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
    };
    use tracing::Level;

    use crate::{
        multi_embedding::{
            db_ext::EmbeddingExt,
            schema::{EmbeddingSetExt, EmbeddingVector},
        },
        Database,
    };
    fn setup_db() -> Result<cozo::Db<MemStorage>, ploke_error::Error> {
        ploke_test_utils::setup_db_full("fixture_nodes")
    }
    use crate::multi_embedding::schema::CozoEmbeddingSetExt;

    // #[cfg(feature = "multi_embedding_db")]
    #[test]
    fn multi_pending_embeddings_count_basic() -> Result<(), ploke_error::Error> {
        ploke_test_utils::init_test_tracing(Level::INFO);
        // let db = Database::new(ploke_test_utils::setup_db_full("fixture_nodes")?);
        // let db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")?;
        let db = setup_db()?;

        let embedding_set = EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("test_provider"),
            EmbeddingModelId::new_from_str("test_model"),
            EmbeddingShape::new_dims_default(123),
        );
        let create_rel_script = embedding_set.script_create();

        tracing::info!("Running script:\n{create_rel_script}");
        let db_result = db
            .run_script(
                &create_rel_script,
                BTreeMap::new(),
                ScriptMutability::Mutable,
            )
            .map_err(DbError::from)?;
        tracing::info!("db_result: {:?}", db_result.rows);

        let put_rel_script = embedding_set.script_put();
        tracing::info!("Running script:\n{put_rel_script}");
        let db_result = db
            .run_script(&put_rel_script, BTreeMap::new(), ScriptMutability::Mutable)
            .map_err(DbError::from)?;
        tracing::info!("db_result: {:?}", db_result.rows);

        let create_vector_script = EmbeddingVector::script_create_from_set(&embedding_set);
        tracing::info!("Running script:\n{create_vector_script}");
        let db_result = db
            .run_script(
                &create_vector_script,
                BTreeMap::new(),
                ScriptMutability::Mutable,
            )
            .map_err(DbError::from)?;
        tracing::info!("db_result: {:?}", db_result.rows);

        tracing::info!("counting unembedded vectors: {:?}", db_result.rows);
        let count =
            <cozo::Db<MemStorage> as EmbeddingExt>::count_pending_embeddings(&db, &embedding_set)?;
        tracing::info!("Total nodes found without embeddings using new method:\n\t{count}");

        panic!();

        Ok(())
    }
}
