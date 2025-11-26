use ploke_core::embeddings::EmbeddingSetId;

use crate::{Database, DbError};

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
    fn count_pending_embeddings(&self, embedding_set_id: EmbeddingSetId) -> Result<usize, DbError>;

    /// Helper function to specifically count unembedded non-files.
    ///
    /// Similar to `count_pending_embeddings`, it is useful when processing vector embeddings in
    /// `ploke-embed`.
    fn count_unembedded_nonfiles(&self, embedding_set_id: EmbeddingSetId)
        -> Result<usize, DbError>;

    /// Helper function to specifically count unembedded files.
    ///
    /// Similar to `count_pending_embeddings`, it is useful when processing vector embeddings in
    /// `ploke-embed`
    fn count_unembedded_files(&self, embedding_set_id: EmbeddingSetId) -> Result<usize, DbError>;
}

impl EmbeddingExt for cozo::Db<cozo::MemStorage> {
    fn count_pending_embeddings(&self, embedding_set_id: EmbeddingSetId) -> Result<usize, DbError> {
        todo!()
    }

    fn count_unembedded_nonfiles(
        &self,
        embedding_set_id: EmbeddingSetId,
    ) -> Result<usize, DbError> {
        todo!()
    }

    fn count_unembedded_files(&self, embedding_set_id: EmbeddingSetId) -> Result<usize, DbError> {
        todo!()
    }
}
