### Overall assessment

- **Change scope**: I implemented the “next logical increment” by adding a minimal `EmbeddingSetId` type in `ploke-core` and making the indexer runtime explicitly shape-aware via `EmbeddingShape`; this stays within the runtime/indexer slice and doesn’t alter DB semantics.  
- **Safety**: `cargo check -p ploke-core -p ploke-embed` passes, and `read_lints` shows no new lints on the touched files, so the edits are compilation- and lint-clean.  
- **Behavior**: Runtime behavior is unchanged for valid configurations—batches still embed and dual-write as before—but we now have stronger invariants: if an embedder ever returns vectors with the wrong length, `process_batch` will fail fast against a canonical shape derived at `IndexerTask` construction.

### Concrete changes made

- **New core type: `EmbeddingSetId`**

  - Added a small, strongly-typed set identifier in `ploke-core`:

    ```startLine:endLine:crates/ploke-core/src/embedding_set.rs
    use serde::{Deserialize, Serialize};

    use crate::{EmbeddingModelId, EmbeddingProviderSlug, EmbeddingShape};

    /// Strongly-typed identifier for an embedding set.
    ///
    /// An embedding set represents a specific provider+model+shape combination
    /// (e.g., "openai" / "text-embedding-ada-002" / `<F32; 1536>`). This type
    /// is intended to align with the runtime-owned multi-embedding schema and
    /// will eventually correspond to rows in the `embedding_sets` relation.
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct EmbeddingSetId {
        pub provider: EmbeddingProviderSlug,
        pub model: EmbeddingModelId,
        pub shape: EmbeddingShape,
    }

    impl EmbeddingSetId {
        /// Convenience constructor from components.
        pub fn new(
            provider: EmbeddingProviderSlug,
            model: EmbeddingModelId,
            shape: EmbeddingShape,
        ) -> Self {
            Self {
                provider,
                model,
                shape,
            }
        }

        /// Returns the embedding dimension for this set.
        pub fn dimension(&self) -> u32 {
            self.shape.dimension
        }
    }
    ```

  - Re-exported from `ploke_core::lib` so all crates can use the same ID:

    ```startLine:endLine:crates/ploke-core/src/lib.rs
    pub mod embedding_types;
    pub use embedding_types::{
        EmbeddingDType, EmbeddingEncoding, EmbeddingModelId, EmbeddingProviderSlug, EmbeddingShape,
    };
    pub mod embedding_set;
    pub use embedding_set::EmbeddingSetId;
    ```

  - This is a **purely additive** API at this point; nothing depends on it yet, which keeps the risk low but sets us up to hang set/provenance metadata off multi-embedding relations later.

- **Threaded `EmbeddingShape` into the indexer**

  - **New field on `IndexerTask`**: the indexer now caches the shape (dimension, dtype, encoding) at construction, derived from the `EmbeddingProcessor`:

    ```startLine:endLine:crates/ingest/ploke-embed/src/indexer/mod.rs
    #[derive(Debug)]
    pub struct IndexerTask {
        pub db: Arc<Database>,
        pub io: IoManagerHandle,
        pub embedding_processor: Arc<EmbeddingProcessor>,
        /// Shape for all embedding vectors produced by this indexer.
        ///
        /// This is derived from the `EmbeddingProcessor` at construction time and
        /// used to enforce that every batch we write is homogeneous with respect
        /// to dimension and encoding, matching the multi-embedding DB helpers.
        pub embedding_shape: EmbeddingShape,
        pub cancellation_token: CancellationToken,
        pub batch_size: usize,
        pub bm25_tx: Option<mpsc::Sender<bm25_service::Bm25Cmd>>,
        pub cursors: Mutex<HashMap<NodeType, Uuid>>,
        pub total_processed: AtomicUsize,
    }

    impl IndexerTask {
        pub fn new(
            db: Arc<Database>,
            io: IoManagerHandle,
            embedding_processor: Arc<EmbeddingProcessor>,
            cancellation_token: CancellationToken,
            batch_size: usize,
        ) -> Self {
            let embedding_shape = embedding_processor.shape();
            Self {
                db,
                io,
                embedding_processor,
                embedding_shape,
                cancellation_token,
                batch_size,
                bm25_tx: None,
                cursors: Mutex::new(HashMap::new()),
                total_processed: AtomicUsize::new(0),
            }
        }
    }
    ```

  - **Dimension enforcement now uses the shape**: `process_batch` previously compared embedding lengths against `embedding_processor.dimensions()`. It now uses the canonical shape dimension:

    ```startLine:endLine:crates/ingest/ploke-embed/src/indexer/mod.rs
        let embeddings = self
            .embedding_processor
            .generate_embeddings(valid_snippets)
            .await?;
        tracing::trace!(
            "Processed embeddings {} with dimension {:?}",
            embeddings.len(),
            embeddings.first().map(|v| v.len())
        );

        let dims = self.embedding_shape.dimension as usize;
        for embedding in &embeddings {
            if embedding.len() != dims {
                return Err(EmbedError::DimensionMismatch {
                    expected: dims,
                    actual: embedding.len(),
                });
            }
        }
    ```

  - This makes the indexer’s invariant explicit: **one shape per indexer instance**, and all embeddings written through it must match that shape. It dovetails with the multi-embedding DB helpers, which already use a dimension-aware `VectorDimensionSpec` and enforce `<F32; dims>` at the relation level.

- **Tests and construction call sites updated**

  - All the unit tests that constructed `EmbeddingProcessor` by manually setting the `source` now call the `new` constructor, so they implicitly get the correct `shape()` behavior and don’t have to know about the internal fields:

    ```startLine:endLine:crates/ingest/ploke-embed/src/indexer/unit_tests.rs
    async fn setup_local_model_embedding_processor() -> Result<EmbeddingProcessor, ploke_error::Error> {
        let model = setup_local_model_config("fixture_nodes").await?;
        let source = EmbeddingSource::Local(model);
        Ok(EmbeddingProcessor::new(source))
    }
    ```

    And similarly for other test sites:

    ```startLine:endLine:crates/ingest/ploke-embed/src/indexer/unit_tests.rs
        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
    ```

    ```startLine:endLine:crates/ingest/ploke-embed/src/indexer/unit_tests.rs
        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = EmbeddingProcessor::new(source);
    ```

  - The `indexer_writes_embeddings_with_multi_embedding_db_enabled` test, which is part of the Slice 2 runtime verification, now builds the processor via `EmbeddingProcessor::new(...)` as well, ensuring that `IndexerTask::new` sees a consistent shape when enforcing dimensions.

### How this advances the Slice 3 plan

- **Stronger runtime invariants**: The indexer now has a **single, strongly-typed embedding shape** attached for its entire lifetime. This is a prerequisite for:
  - Refusing mixed-dimension batches, and  
  - Threading set/shape metadata into DB metadata (`embedding_sets`, `embedding_nodes`) without relying on ad-hoc length checks inside DB helpers.

- **Set-level typing groundwork**: `EmbeddingSetId` gives us a canonical way to name “embedding sets” (provider + model + shape). This aligns with the multi-embedding schema’s intent and will be the natural type to:
  - Attach to indexer configuration (`IndexerTask`/`EmbeddingProcessor`), and  
  - Persist into future `embedding_sets` tables and runtime `/embedding use` commands.

- **Next logical follow-ups (not done yet, but now unblocked)**:
  - Thread `EmbeddingSetId` (or at least `EmbeddingProviderSlug`/`EmbeddingModelId`) through the indexer config so we can associate each runtime embedding job with a concrete set ID.  
  - Extend runtime tests to assert shape/set invariants (e.g., the indexer refuses to run if the DB doesn’t support the requested dimension, or if a provider/model mismatch is detected).  
  - Eventually wire `EmbeddingSetId` into the multi-embedding metadata relations and `/embedding use` CLI so end users select sets by typed ID rather than loose strings.

If you’d like, I can take the next pass by introducing a minimal “indexer config” struct that carries an `EmbeddingSetId` alongside the existing processor, then update the indexer tests to assert that the set’s `dimension()` matches the DB vector dimension specs.
