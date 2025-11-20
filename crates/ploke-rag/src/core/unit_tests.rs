#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use itertools::Itertools;
    use lazy_static::lazy_static;
    use ploke_core::EmbeddingData;
    use ploke_db::{create_index_primary, Database};
    use ploke_embed::{
        indexer::{EmbeddingProcessor, EmbeddingSource},
        local::{EmbeddingConfig, LocalEmbedder},
    };
    use ploke_error::Error;
    use ploke_io::IoManagerHandle;
    use ploke_test_utils::workspace_root;
    use tokio::time::{sleep, Duration};
    use tracing::Level;
    use uuid::Uuid;

    use crate::{RagError, RagService};
    use std::sync::Once;
    static TEST_TRACING: Once = Once::new();
    fn init_tracing_once() {
        TEST_TRACING.call_once(|| {
            ploke_test_utils::init_test_tracing(tracing::Level::ERROR);
        });
    }

    lazy_static! {
        /// This test db is restored from the backup of an earlier parse of the `fixture_nodes`
        /// crate located in `tests/fixture_crates/fixture_nodes`, and has a decent sampling of all
        /// rust code items. It provides a good target for other tests because it has already been
        /// extensively tested in `syn_parser`, with each item individually verified to have all
        /// fields correctly parsed for expected values.
        ///
        /// One "gotcha" of laoding the Cozo database is that the hnsw items are not retained
        /// between backups, so they must be recalculated each time. However, by restoring the
        /// backup database we do retain the dense vector embeddings, allowing our tests to be
        /// significantly sped up by using a lazy loader here and making calls to the same backup.
        ///
        /// If needed, other tests can re-implement the load from this file, which may become a
        /// factor for some tests that need to alter the database, but as long as things are
        /// cleaned up afterwards it should be OK.
        // TODO: Add a mutex guard to avoid cross-contamination of tests.
        pub static ref TEST_DB_NODES: Result<Arc< Database >, Error> = {
            let db = Database::init_with_schema()?;

            let mut target_file = workspace_root();
            target_file.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
            let prior_rels_vec = db.relations_vec()?;
            db.import_from_backup(&target_file, &prior_rels_vec)
                .map_err(ploke_db::DbError::from)
                .map_err(ploke_error::Error::from)?;
            create_index_primary(&db)?;
            Ok(Arc::new( db ))
        };
    }

    async fn fetch_snippet_containing(
        db: &Arc<Database>,
        ordered_node_ids: Vec<Uuid>,
        search_term: &str,
    ) -> Result<String, Error> {
        let node_info: Vec<EmbeddingData> = db.get_nodes_ordered(ordered_node_ids)?;
        let io_handle = IoManagerHandle::new();

        let snippet_find: Vec<String> = io_handle
            .get_snippets_batch(node_info)
            .await
            .expect("Problem receiving")
            .into_iter()
            .try_collect()?;

        snippet_find
            .into_iter()
            .find(|snip| snip.contains(search_term))
            .ok_or_else(|| {
                RagError::Search(format!("No snippet found for term {search_term}")).into()
            })
    }

    async fn fetch_and_assert_snippet(
        db: &Arc<Database>,
        ordered_node_ids: Vec<Uuid>,
        search_term: &str,
    ) -> Result<(), Error> {
        let node_info: Vec<EmbeddingData> = db.get_nodes_ordered(ordered_node_ids)?;
        let io_handle = IoManagerHandle::new();

        let snippet = io_handle
            .get_snippets_batch(node_info)
            .await
            .expect("Problem receiving")
            .into_iter()
            .inspect(|snip| eprintln!("Search result: {:?}", snip))
            .find(|snip| snip.as_ref().is_ok_and(|s| s.contains(search_term)));

        assert!(
            snippet.is_some(),
            "No snippet found containing '{}'",
            search_term
        );
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_search() -> Result<(), Error> {
        // Initialize tracing for the test
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "use_all_const_static";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 15).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_bm25_rebuild() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        // Should not error
        rag.bm25_rebuild().await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_bm25_search_basic() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "use_all_const_static";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        // Trigger a rebuild to ensure index is fresh, then retry a few times in case it's async.
        rag.bm25_rebuild().await?;

        let mut bm25_res: Vec<(Uuid, f32)> = Vec::new();
        for _ in 0..10 {
            bm25_res = rag.search_bm25(search_term, 15).await?;
            if !bm25_res.is_empty() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert!(
            !bm25_res.is_empty(),
            "BM25 search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = bm25_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_hybrid_search() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "use_all_const_static";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;
        // Rebuild BM25 index so hybrid search uses real sparse scores rather than dense fallback.
        rag.bm25_rebuild().await?;
        let fused: Vec<(Uuid, f32)> = rag.hybrid_search(search_term, 15).await?;
        assert!(
            !fused.is_empty(),
            "Hybrid search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = fused.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_bm25_search_fallback() -> Result<(), Error> {
        // Initialize tracing for the test
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "use_all_const_static";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        // Intentionally do not call bm25_rebuild or index anything; fallback should kick in.
        let results: Vec<(Uuid, f32)> = rag.search_bm25(search_term, 15).await?;
        assert!(
            !results.is_empty(),
            "BM25 fallback returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = results.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_search_structs() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "DocumentedStruct";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 10).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_search_enums() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "GenericEnum";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 10).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_search_traits() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "ComplexGenericTrait";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 10).await?;

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _)| *id).collect();
        let snippet_found = fetch_snippet_containing(db, ordered_node_ids, search_term).await;

        // This assertion documents that dense search does reliably
        // retrieve items whose identifier appears only once in the source.
        assert!(
            snippet_found.is_err(),
            "Dense search unexpectedly found the trait '{search_term}'. \
          This indicates either the test fixture or the model changed."
        );

        // Ensure sparse index is populated so we test BM25 behavior (not dense fallback).
        rag.bm25_rebuild().await?;
        let mut results: Vec<(Uuid, f32)> = Vec::new();
        for _ in 0..10 {
            results = rag.search_bm25(search_term, 15).await?;
            if !results.is_empty() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
        assert!(
            !results.is_empty(),
            "BM25 search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = results.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_search_unions() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "GenericUnion";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 10).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_search_macros() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "documented_macro";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 10).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_search_type_aliases() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "DisplayableContainer";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 10).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_search_constants() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "TOP_LEVEL_BOOL";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 10).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_search_statics() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "TOP_LEVEL_COUNTER";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 10).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_hybrid_search_generic_trait() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "GenericSuperTrait";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;
        // Rebuild BM25 index so hybrid search uses real sparse scores rather than dense fallback.
        rag.bm25_rebuild().await?;
        let fused: Vec<(Uuid, f32)> = rag.hybrid_search(search_term, 15).await?;
        assert!(
            !fused.is_empty(),
            "Hybrid search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = fused.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_bm25_search_complex_enum() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "EnumWithMixedVariants";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        // Ensure BM25 index is populated
        rag.bm25_rebuild().await?;

        let mut bm25_res: Vec<(Uuid, f32)> = Vec::new();
        for _ in 0..10 {
            bm25_res = rag.search_bm25(search_term, 15).await?;
            if !bm25_res.is_empty() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        assert!(
            !bm25_res.is_empty(),
            "BM25 search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = bm25_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_search_function_definitions() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");

        let search_term = "use_all_const_static";

        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 10).await?;
        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[cfg(feature = "multi_embedding_db")]
    #[tokio::test]
    async fn search_for_set_returns_results_for_seeded_set() -> Result<(), Error> {
        use ploke_core::{EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSetId, EmbeddingShape};
        use ploke_db::multi_embedding::schema::vector_dims::vector_dimension_specs;
        use ploke_db::{EmbeddingInsert, MultiEmbeddingRuntimeConfig};
        use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
        use ploke_embed::local::{EmbeddingConfig, LocalEmbedder};

        init_tracing_once();

        // DB with multi-embedding fixtures and runtime config enabled
        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Arc::new(ploke_db::Database::with_multi_embedding_config(
            raw_db, config,
        ));

        // Pick a pending node and dimension spec so we can seed a runtime embedding
        // that lives in the same vector space as our query.
        let batches = db.get_unembedded_node_data(1, 0)?;
        let (_node_type, node) = batches
            .into_iter()
            .find_map(|typed| typed.v.into_iter().next().map(|entry| (typed.ty, entry)))
            .expect("at least one pending node");

        let dim_spec = vector_dimension_specs()
            .first()
            .expect("at least one vector dimension spec");

        // Embedding processor: use the default local embedder (384 dims) so both the
        // stored vector and query vector come from the same model/shape.
        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));

        let rag = RagService::new(db.clone(), embedding_processor.clone())?;

        // Build an EmbeddingSetId that matches the seeded dimension spec and the
        // embedder we are using for this test.
        let shape = EmbeddingShape::f32_raw(dim_spec.dims() as u32);
        let set_id = EmbeddingSetId::new(
            EmbeddingProviderSlug(dim_spec.provider().to_string()),
            EmbeddingModelId(dim_spec.embedding_model().to_string()),
            shape,
        );

        // Generate an embedding for a concrete query string and persist it for the
        // selected node using the set-aware helper.
        let query = "set_aware_roundtrip_query";
        let vectors = embedding_processor
            .generate_embeddings(vec![query.to_string()])
            .await?;
        let vector = vectors
            .into_iter()
            .next()
            .expect("expected one embedding from generate_embeddings");

        db.update_embeddings_batch_for_set(vec![EmbeddingInsert {
            node_id: node.id,
            set_id: set_id.clone(),
            vector: vector.clone(),
        }])
        .await?;

        // Ensure HNSW indexes exist for this type (including multi-embedding indexes).
        create_index_primary(&db)?;

        // Use the same query text and embedding set when performing dense search; the
        // seeded node should appear among the top results.
        let hits: Vec<(Uuid, f32)> = rag.search_for_set(query, 10, &set_id).await?;
        assert!(
            hits.iter().any(|(id, _)| *id == node.id),
            "expected set-aware dense search to return the seeded node for matching embedding/query"
        );

        Ok(())
    }

    #[cfg(feature = "multi_embedding_db")]
    #[tokio::test]
    async fn search_for_set_falls_back_when_multi_embedding_disabled() -> Result<(), Error> {
        use ploke_core::{EmbeddingModelId, EmbeddingProviderSlug};
        use ploke_core::{EmbeddingSetId, EmbeddingShape};
        use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
        use ploke_embed::local::{EmbeddingConfig, LocalEmbedder};

        init_tracing_once();

        // Legacy-style DB without multi_embedding_db enabled.
        let db = Arc::new(ploke_db::Database::init_with_schema()?);

        // Use the existing dense search test fixture DB for embeddings.
        let db_nodes = TEST_DB_NODES
            .as_ref()
            .expect("TEST_DB_NODES must initialize")
            .clone();

        // Swap in the pre-populated DB handle so we have real embeddings/HNSW state.
        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db_nodes.clone(), embedding_processor)?;

        // Build a dummy EmbeddingSetId; because multi_embedding_db is disabled on the
        // Database, search_for_set should transparently fall back to legacy search.
        let shape = EmbeddingShape::f32_raw(384);
        let dummy_set = EmbeddingSetId::new(
            EmbeddingProviderSlug("local-transformers".to_string()),
            EmbeddingModelId("sentence-transformers/all-MiniLM-L6-v2".to_string()),
            shape,
        );

        let search_term = "use_all_const_static";
        let hits: Vec<(Uuid, f32)> = rag.search_for_set(search_term, 15, &dummy_set).await?;
        assert!(
            !hits.is_empty(),
            "expected search_for_set to fall back and return results when multi_embedding_db is disabled"
        );

        Ok(())
    }

    /// Shape test: dense search should only be expected to return results for
    /// symbols that actually have non-null legacy embeddings in the fixture
    /// database. This avoids over-constraining tests on specific symbols.
    #[tokio::test]
    async fn rag_dense_search_matches_embed_presence() -> Result<(), Error> {
        init_tracing_once();

        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.")
            .clone();

        // Symbols exercised by existing RAG tests along with their relations.
        let symbols = &[
            ("use_all_const_static", "function"),
            ("TOP_LEVEL_BOOL", "const"),
            ("TOP_LEVEL_COUNTER", "static"),
        ];

        // Check which symbols actually have non-null legacy embeddings.
        let mut embedded_symbols = Vec::new();
        for (name, relation) in symbols {
            let script = format!(
                r#"
?[name, has_embedding] :=
    *{rel}{{ name, embedding @ 'NOW' }},
    name = {name_lit},
    has_embedding = !is_null(embedding)
"#,
                rel = relation,
                name_lit = format!("{name:?}"),
            );
            let rows = db.raw_query(&script).map_err(ploke_error::Error::from)?;
            let has_embedding = rows
                .rows
                .first()
                .and_then(|row| row.get(1))
                .and_then(|v| v.get_bool())
                .unwrap_or(false);
            if has_embedding {
                embedded_symbols.push(*name);
            }
        }

        assert!(
            !embedded_symbols.is_empty(),
            "expected at least one test symbol to have a non-null legacy embedding"
        );

        // Build a dense embedder + RAG service for the fallback path.
        let model = LocalEmbedder::new(EmbeddingConfig::default())?;
        let source = EmbeddingSource::Local(model);
        let embedding_processor = Arc::new(EmbeddingProcessor::new(source));
        let rag = RagService::new(db.clone(), embedding_processor)?;

        // For every symbol with an embedding present, dense search should
        // produce at least one hit.
        for symbol in embedded_symbols {
            let hits: Vec<(Uuid, f32)> = rag.search(symbol, 10).await?;
            assert!(
                !hits.is_empty(),
                "expected dense search to return hits for symbol '{symbol}' with non-null embedding"
            );
        }

        Ok(())
    }
}
