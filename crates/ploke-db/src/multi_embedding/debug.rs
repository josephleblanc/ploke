use ploke_core::embeddings::EmbeddingSet;

use crate::{
    multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt, schema::EmbeddingSetExt},
    Database, DbError,
};

use super::*;

impl DebugAll for Database {}

pub trait DebugAll: HnswExt + EmbeddingExt {
    fn is_embedding_info_all(
        &self,
        embedding_set: &EmbeddingSet,
    ) -> Result<IsEmbeddingInfo, DbError> {
        use super::hnsw_ext::HnswExt;
        let is_embedding_set_registered = self.is_embedding_set_registered()?;
        let is_embedding_set_row = self.is_embedding_set_row_present(embedding_set)?;
        let is_vector_embedding = self.is_vector_embedding_registered(embedding_set)?;
        let is_hnsw_relation = self.is_hnsw_index_registered(embedding_set)?;
        let embedding_info = IsEmbeddingInfo {
            embedding_set: embedding_set.clone(),
            is_embedding_set_registered,
            is_embedding_set_row,
            is_vector_embedding,
            is_hnsw_relation,
        };
        Ok(embedding_info)
    }
}

/// Helper struct to aggregate info on presence/absence of relations related to embedding vectors
/// in the database for a given EmbeddingSet.
pub struct IsEmbeddingInfo {
    pub embedding_set: EmbeddingSet,
    pub is_embedding_set_registered: bool,
    pub is_embedding_set_row: bool,
    pub is_vector_embedding: bool,
    pub is_hnsw_relation: bool,
}

impl IsEmbeddingInfo {
    pub fn tracing_print_all(&self, level: tracing::Level) {
        use tracing::Level;
        let msg = format!(
            "Is embedding info all:
\t{vec_name: <20} | vector relation name
\t{hnsw_name: <20} | hnsw relation name
\t{is_set: <20} | is_embedding_set_registered
\t{is_set_row: <20} | is_embedding_set_row
\t{is_vec_emb: <20} | is_vector_embedding
\t{is_hnsw_rel: <20} | is_hnsw_relation",
            vec_name = self.embedding_set.vector_relation_name(),
            hnsw_name = self.embedding_set.hnsw_rel_name(),
            is_set = self.is_embedding_set_registered,
            is_set_row = self.is_embedding_set_row,
            is_vec_emb = self.is_vector_embedding,
            is_hnsw_rel = self.is_hnsw_relation,
        );
        use tracing::{debug, error, info, trace, warn};
        match level {
            Level::TRACE => {
                trace!(%msg)
            }
            Level::DEBUG => {
                debug!(%msg)
            }
            Level::INFO => {
                info!(%msg)
            }
            Level::WARN => {
                warn!(%msg)
            }
            Level::ERROR => {
                error!(%msg)
            }
        }
    }
}
