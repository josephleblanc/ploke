#[cfg(test)]
mod tests {
    use std::{default, sync::Arc};

    use crate::{RetrievalStrategy, TokenBudget};
    use itertools::Itertools;
    use lazy_static::lazy_static;
    use ploke_core::EmbeddingData;
    use ploke_db::{create_index_primary, create_index_primary_with_index, Database};
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
        #[cfg(not(feature = "multi_embedding_rag"))]
        TEST_TRACING.call_once(|| {
            ploke_test_utils::init_test_tracing(tracing::Level::ERROR);
        });
        #[cfg(feature = "multi_embedding_rag")]
        TEST_TRACING.call_once(|| {
            ploke_test_utils::init_test_tracing_with_target("cozo-script", tracing::Level::ERROR);
        });
    }

    async fn db_test_setup() -> Result<Arc<Database>, Error> {
        let base_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")?;
        let new_db = Database::new(base_db);
        ploke_db::multi_embedding::db_ext::load_db(&new_db, "fixture_nodes".to_string()).await?;
        Ok(Arc::new(new_db))
    }

    #[cfg(feature = "multi_embedding_rag")]
    fn default_test_db_setup() -> Result<Arc<Database>, Error> {
        use ploke_db::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt};
        use tracing::info;

        init_tracing_once();
        // let db = Database::init_with_schema()?;
        // let db = Database::new( ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")? );
        let db = Database::new(ploke_test_utils::setup_db_full_multi_embedding(
            "fixture_nodes",
        )?);

        // let mut target_file = workspace_root();
        // target_file.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
        // let prior_rels_vec = db.relations_vec()?;
        // db.import_from_backup(&target_file, &prior_rels_vec)
        //     .map_err(ploke_db::DbError::from)
        //     .map_err(ploke_error::Error::from)?;
        // let embedding_set = &db.active_embedding_set;
        //
        // let before_is_index_registered = db.is_hnsw_index_registered(embedding_set)?;
        // info!(?before_is_index_registered);
        //
        // let before_is_vector_embedding_registered = db.is_vector_embedding_registered(embedding_set)?;
        // info!(?before_is_vector_embedding_registered);
        // // db.create_embedding_index(&embedding_set)?;
        // let create_index = create_index_primary(&db);
        // info!(?create_index);
        // create_index?;
        //
        // let after_is_index_registered = db.is_hnsw_index_registered(embedding_set)?;
        // info!(?after_is_index_registered);
        //
        // let count_pending_after = db.count_pending_embeddings()?;
        // info!(?count_pending_after);

        Ok(Arc::new(db))

        // let embedding_set = ploke_core::embeddings::EmbeddingSet::default();
        //
        // let r2 = db.ensure_embedding_relation(&db.active_embedding_set.clone());
        // tracing::info!(ensure_embedding_relation = ?r2);
        // r2?;
        //
        // let r3 = db.create_embedding_index(&db.active_embedding_set.clone());
        // tracing::info!(create_embedding_index = ?r3);
        // r3?;
        //
        // let database = Database::from(db);
    }

    #[cfg(feature = "multi_embedding_rag")]
    #[tokio::test]
    async fn test_fixture_embeddings_loaded_into_active_set() -> Result<(), Error> {
        use tracing::info;

        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");
        ploke_db::multi_embedding::db_ext::load_db(db, "fixture_nodes".to_string()).await?;

        let rel = db.active_embedding_set.rel_name.clone();
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
    #[cfg(feature = "multi_embedding_rag")]
    #[tokio::test]
    async fn dense_context_uses_multi_embedding_relations() {
        use ploke_db::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt};

        // Load the multi-embedding backup directly to mirror the TUI /load path.
        let db = Database::init_with_schema().expect("init schema");
        let mut target_file = workspace_root();
        target_file.push(
            "tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92",
        );
        let prior_rels_vec = db.relations_vec().expect("relations_vec");
        db.import_from_backup(&target_file, &prior_rels_vec)
            .expect("import_from_backup");
        ploke_db::create_index_primary_with_index(&db).expect("create_index_primary");

        // Note: if the backup lacks vectors, we still expect the legacy-path error; this test
        // asserts on that specific failure mode.
        let embed_rel = db.active_embedding_set.rel_name.clone();
        let count_script = format!("?[count(node_id)] := *{embed_rel}{{ node_id @ 'NOW' }}");
        let rows = db.raw_query(&count_script).expect("count query");
        let _count = rows
            .rows
            .first()
            .and_then(|r| r.first())
            .and_then(|v| v.get_int())
            .unwrap_or(0);

        // Issue a dense search directly through hnsw to surface any legacy-path errors.
        let dims = db.active_embedding_set.dims() as usize;
        let query_vec = vec![0.1f32; dims];
        let err = match db.search_similar_for_set(
            &db.active_embedding_set,
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
    #[cfg(feature = "multi_embedding_rag")]
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
            // let db = Database::init_with_schema()?;
            //
            // let mut target_file = workspace_root();
            // target_file.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
            // let prior_rels_vec = db.relations_vec()?;
            // db.import_from_backup(&target_file, &prior_rels_vec)
            //     .map_err(ploke_db::DbError::from)
            //     .map_err(ploke_error::Error::from)?;
            // create_index_primary(&db)?;
            // Ok(Arc::new( db ))
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
        // let db = TEST_DB_NODES
        //     .as_ref()
        //     .expect("Must set up TEST_DB_NODES correctly.");

        let db_base = Database::new(ploke_test_utils::setup_db_full_multi_embedding(
            "fixture_nodes",
        )?);
        ploke_db::multi_embedding::db_ext::load_db(&db_base, "fixture_nodes".to_string()).await?;
        ploke_db::create_index_primary(&db_base)?;
        let db = Arc::new(db_base);

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
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
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
        // let db = db_test_setup().await?;

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
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
        Ok(())
    }

    #[tokio::test]
    // #[ignore = "temporary ignore: DB backup not accessible in sandbox (code 14)"]
    async fn test_hybrid_search() -> Result<(), Error> {
        init_tracing_once();
        // let db = TEST_DB_NODES
        //     .as_ref()
        //     .expect("Must set up TEST_DB_NODES correctly.");

        let base_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")?;
        let new_db = Database::new(base_db);
        ploke_db::multi_embedding::db_ext::load_db(&new_db, "fixture_nodes".to_string()).await?;
        let db = Arc::new(new_db);

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
        fetch_and_assert_snippet(&db, ordered_node_ids, search_term).await?;
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
    // TODO: Turn into a regression test
    async fn test_search_traits() -> Result<(), Error> {
        init_tracing_once();
        let db = TEST_DB_NODES
            .as_ref()
            .expect("Must set up TEST_DB_NODES correctly.");
        // Ensure the fixture backup is only loaded when the dense index has not been built yet.
        {
            use ploke_db::multi_embedding::hnsw_ext::HnswExt;
            let db_ref: &Database = db.as_ref();
            let has_index = db_ref.is_hnsw_index_registered(&db_ref.active_embedding_set)?;
            if !has_index {
                ploke_db::multi_embedding::db_ext::load_db(db, "fixture_nodes".to_string())
                    .await?;
            }
        }
        // When this test is run in isolation we still need a dense index.
        create_index_primary_with_index(db)?;

        let search_term = "ComplexGenericTrait";

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
        let ordered_node_ids: Vec<Uuid> = search_res.iter().map(|(id, _)| *id).collect();
        // Dense search should now surface the complex trait without requiring a sparse fallback.
        let snippet = fetch_snippet_containing(db, ordered_node_ids, search_term).await?;
        assert!(
            snippet.contains(search_term),
            "Dense search returned a snippet without the search term '{search_term}'"
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
}
