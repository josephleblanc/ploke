use std::sync::{Arc, RwLock};

use crate::cancel_token::CancellationListener;
use crate::error::EmbedError;
use crate::indexer::EmbeddingProcessor;
use ploke_core::embeddings::EmbeddingSet;
use ploke_db::multi_embedding::db_ext::EmbeddingExt;
use ploke_db::Database;

/// Shared runtime handle for the active embedding set and embedder.
///
/// This is the single source of truth for which provider/model/dimension tuple
/// is considered active at runtime. Both the database and indexing/search
/// components should observe this handle so hot-swaps remain consistent.
#[derive(Debug, Clone)]
pub struct EmbeddingRuntime {
    active_set: Arc<RwLock<EmbeddingSet>>,
    embedder: Arc<RwLock<Arc<EmbeddingProcessor>>>,
}

impl EmbeddingRuntime {
    /// Construct a new runtime with the provided embedding set and embedder.
    pub fn new(active_set: EmbeddingSet, embedder: EmbeddingProcessor) -> Self {
        Self {
            active_set: Arc::new(RwLock::new(active_set)),
            embedder: Arc::new(RwLock::new(Arc::new(embedder))),
        }
    }

    /// Construct using an existing active-set handle so the database and runtime share the same lock.
    pub fn from_shared_set(
        active_set: Arc<RwLock<EmbeddingSet>>,
        embedder: EmbeddingProcessor,
    ) -> Self {
        Self {
            active_set,
            embedder: Arc::new(RwLock::new(Arc::new(embedder))),
        }
    }

    /// Convenience constructor using the default embedding set.
    pub fn with_default_set(embedder: EmbeddingProcessor) -> Self {
        Self::new(EmbeddingSet::default(), embedder)
    }

    /// Return a clone of the active set for read-only use.
    pub fn current_active_set(&self) -> Result<EmbeddingSet, EmbedError> {
        let guard = self
            .active_set
            .read()
            .map_err(|_| EmbedError::State("active embedding set poisoned".into()))?;
        Ok(guard.clone())
    }

    /// Return a clone of the current embedder Arc.
    pub fn current_processor(&self) -> Result<Arc<EmbeddingProcessor>, EmbedError> {
        let guard = self
            .embedder
            .read()
            .map_err(|_| EmbedError::State("embedder handle poisoned".into()))?;
        Ok(Arc::clone(&*guard))
    }

    /// Expose the active-set handle so consumers (e.g., Database) can share it.
    pub fn active_set_handle(&self) -> Arc<RwLock<EmbeddingSet>> {
        Arc::clone(&self.active_set)
    }

    /// Swap both the active embedding set and embedder, updating the database schema
    /// for the new set before making it visible to other components.
    pub fn activate(
        &self,
        db: &Database,
        new_set: EmbeddingSet,
        new_embedder: EmbeddingProcessor,
    ) -> Result<(), EmbedError> {
        db.ensure_embedding_set_relation()?;
        db.ensure_vector_embedding_relation(&new_set)?;
        db.set_active_set(new_set.clone())?;

        {
            let mut set_guard = self
                .active_set
                .write()
                .map_err(|_| EmbedError::State("active embedding set poisoned".into()))?;
            *set_guard = new_set;
        }

        {
            let mut embedder_guard = self
                .embedder
                .write()
                .map_err(|_| EmbedError::State("embedder handle poisoned".into()))?;
            *embedder_guard = Arc::new(new_embedder);
        }

        Ok(())
    }

    /// Delegate embedder methods through the runtime so existing call-sites can share
    /// a single handle while allowing hot-swaps.
    pub async fn generate_embeddings(
        &self,
        snippets: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        let embedder = self.current_processor()?;
        embedder.generate_embeddings(snippets).await
    }

    pub async fn generate_embeddings_with_cancel(
        &self,
        snippets: Vec<String>,
        cancel: Option<&CancellationListener>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        let embedder = self.current_processor()?;
        embedder
            .generate_embeddings_with_cancel(snippets, cancel)
            .await
    }

    pub fn dimensions(&self) -> Result<usize, EmbedError> {
        let embedder = self.current_processor()?;
        Ok(embedder.dimensions())
    }
}
