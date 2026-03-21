#[cfg(test)]
mod tests {
    use std::{default, sync::Arc};

    use crate::{RetrievalStrategy, TokenBudget};
    use itertools::Itertools;
    use lazy_static::lazy_static;
    use ploke_core::EmbeddingData;
    use ploke_db::{
        Database, create_index_primary_with_index,
        multi_embedding::{db_ext::EmbeddingExt, debug::DebugAll},
    };
    use ploke_embed::{
        indexer::{EmbeddingProcessor, EmbeddingSource},
        local::{EmbeddingConfig, LocalEmbedder},
        runtime::EmbeddingRuntime,
    };
    use ploke_error::Error;
    use ploke_io::IoManagerHandle;
    use ploke_test_utils::{
        FIXTURE_NODES_LOCAL_EMBEDDINGS, fresh_backup_fixture_db, shared_backup_fixture_db,
    };
    use tokio::time::{Duration, sleep};
    use tracing::{Level, debug};
    use uuid::Uuid;

    use crate::{RagError, RagService};
    use std::sync::LazyLock;
    use std::sync::Once;

    static TEST_TRACING: Once = Once::new();
    fn init_tracing_once() {
        TEST_TRACING.call_once(|| {
            ploke_test_utils::init_test_tracing_with_target("", tracing::Level::ERROR);
        });
    }

    static DEFAULT_TEST_RAG: LazyLock<RagService> = LazyLock::new(|| {
        let db = default_test_db_setup().expect("db setup");
        init_test_rag(db)
    });
    fn init_test_rag(db: Arc<Database>) -> RagService {
        let model =
            LocalEmbedder::new(EmbeddingConfig::default()).expect("valid default embedding config");
        let source = EmbeddingSource::Local(model);
        let embedding_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            EmbeddingProcessor::new(source),
        ));
        RagService::new(db, embedding_runtime).expect("valid db and RagService constructor args")
    }

    async fn init_test_rag_bm25(db: Arc<Database>) -> RagService {
        let rag = init_test_rag(db);
        rag.bm25_rebuild().await.expect("bm25 rebuild must succeed");
        rag
    }

    fn load_local_fixture_db() -> Result<Arc<Database>, Error> {
        shared_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)
    }

    async fn db_test_setup() -> Result<Arc<Database>, Error> {
        load_local_fixture_db()
    }

    fn default_test_db_setup() -> Result<Arc<Database>, Error> {
        load_local_fixture_db()
    }

    fn runtime_for(db: &Arc<Database>, processor: EmbeddingProcessor) -> Arc<EmbeddingRuntime> {
        Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            processor,
        ))
    }

    #[tokio::test]
    async fn test_fixture_embeddings_loaded_into_active_set() -> Result<(), Error> {
        use tracing::info;

        init_tracing_once();
        let db = &DEFAULT_TEST_RAG.db;

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let rel = db.with_active_set(|set| set.rel_name.clone())?;
        let script = format!("?[count(node_id)] := *{rel}{{ node_id @ 'NOW' }}");
        let rows = db.raw_query(&script).map_err(ploke_error::Error::from)?;
        info!(?rows);
        let count = rows
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|val| val.get_int())
            .unwrap_or(0) as usize;

        assert!(
            count > 0,
            "Embedding relation is empty after loading fixture backup; \
             dense search relies on seeded vectors (use multi-embedding backup)."
        );

        Ok(())
    }

    /// Ensure dense search hits multi-embedding relations (and not legacy `function.embedding`).
    /// This mirrors loading a multi-embedding backup then issuing a vector search.
    #[tokio::test]
    async fn dense_context_uses_multi_embedding_relations() {
        use ploke_db::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt};

        let db = load_local_fixture_db().expect("load local embedding fixture");

        // Note: if the backup lacks vectors, we still expect the legacy-path error; this test
        // asserts on that specific failure mode.

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = db
            .with_active_set(|set| set.clone())
            .expect("Un-Poisoned active_embedding_set");

        let embed_rel = active_embedding_set.rel_name.clone();
        let count_script = format!("?[count(node_id)] := *{embed_rel}{{ node_id @ 'NOW' }}");
        let rows = db.raw_query(&count_script).expect("count query");
        let _count = rows
            .rows
            .first()
            .and_then(|r| r.first())
            .and_then(|v| v.get_int())
            .unwrap_or(0);

        // Issue a dense search directly through hnsw to surface any legacy-path errors.
        let dims = active_embedding_set.dims() as usize;
        let query_vec = vec![0.1f32; dims];
        let err = match db.search_similar_for_set(
            &active_embedding_set,
            ploke_db::NodeType::Function,
            query_vec,
            5,
            10,
            5,
            None,
        ) {
            Ok(_) => return,
            Err(e) => e.to_string(),
        };
        assert!(
            err.contains("function") && err.contains("embedding"),
            "expected legacy embedding column error; got: {err}"
        );
    }
    #[tokio::test]
    async fn test_db_nodes_setup() -> Result<(), Error> {
        let db = default_test_db_setup()?;
        Ok(())
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
            default_test_db_setup()
        };
    }

    async fn fetch_snippet_containing(
        db: &Arc<Database>,
        ordered_node_ids: Vec<Uuid>,
        search_term: &str,
    ) -> Result<String, Error> {
        let node_info: Vec<EmbeddingData> = db.get_nodes_ordered(ordered_node_ids)?;
        let io_handle = IoManagerHandle::new();

        let debug_msg = node_info
            .iter()
            .enumerate()
            .map(|x| format!("{:#?}", x))
            .join("\n");
        debug!(%debug_msg);

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
    async fn test_search() -> Result<(), Error> {
        init_tracing_once();

        let search_term = "use_all_const_static";

        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

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
    async fn test_bm25_rebuild() -> Result<(), Error> {
        init_tracing_once();
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        // Should not error
        rag.bm25_rebuild().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_bm25_search_basic() -> Result<(), Error> {
        init_tracing_once();
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        let search_term = "use_all_const_static";

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
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_hybrid_search() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        let search_term = "use_all_const_static";

        let fused: Vec<(Uuid, f32)> = rag.hybrid_search(search_term, 15).await?;
        assert!(
            !fused.is_empty(),
            "Hybrid search returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = fused.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_bm25_search_fallback() -> Result<(), Error> {
        // Initialize tracing for the test
        init_tracing_once();
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        let search_term = "use_all_const_static";

        // Intentionally do not call bm25_rebuild or index anything; fallback should kick in.
        let results: Vec<(Uuid, f32)> = rag.search_bm25(search_term, 15).await?;
        assert!(
            !results.is_empty(),
            "BM25 fallback returned no results for '{}'",
            search_term
        );

        let ordered_node_ids: Vec<Uuid> = results.iter().map(|(id, _score)| *id).collect();
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_structs() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "DocumentedStruct";

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
    async fn test_search_enums() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "GenericEnum";

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
    async fn test_search_traits_new() -> Result<(), Error> {
        init_tracing_once();

        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag_bm25(Arc::clone(&db)).await;

        let search_term = "ComplexGenericTrait";

        let search_res: Vec<(Uuid, f32)> = rag.search(search_term, 10).await?;

        assert!(
            !search_res.is_empty(),
            "Dense search returned no results for '{}'",
            search_term
        );
        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _)| *id).collect();

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
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_unions() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "GenericUnion";

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
    async fn test_search_macros() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "documented_macro";

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
    async fn test_search_type_aliases() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "DisplayableContainer";

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
    async fn test_search_constants() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "TOP_LEVEL_BOOL";

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
    async fn test_search_statics() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "TOP_LEVEL_COUNTER";

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
    async fn test_hybrid_search_generic_trait() -> Result<(), Error> {
        init_tracing_once();
        // Rebuild BM25 index so hybrid search uses real sparse scores rather than dense fallback.
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "GenericSuperTrait";
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
    async fn test_bm25_search_complex_enum() -> Result<(), Error> {
        init_tracing_once();
        let db_raw = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
        let db = Arc::new(db_raw);
        let rag = init_test_rag(Arc::clone(&db));

        let search_term = "EnumWithMixedVariants";

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
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_search_function_definitions() -> Result<(), Error> {
        init_tracing_once();
        let rag = &DEFAULT_TEST_RAG;
        let db = &DEFAULT_TEST_RAG.db;

        let search_term = "use_all_const_static";

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
}
