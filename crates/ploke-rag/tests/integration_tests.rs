#![allow(dead_code, unused_imports)]
use std::{path::PathBuf, sync::Arc};

use ploke_core::{embeddings::EmbeddingSet, EmbeddingData};
use ploke_db::{create_index_for_set, multi_embedding::db_ext::EmbeddingExt, Database, DbError};
use ploke_embed::{
    indexer::{EmbeddingProcessor, EmbeddingSource},
    local::{EmbeddingConfig, LocalEmbedder},
    runtime::EmbeddingRuntime,
};
use ploke_error::Error;
use ploke_io::IoManagerHandle;
use ploke_rag::RagService;
use ploke_test_utils::workspace_root;
use tokio::time::{sleep, Duration};

const LOCAL_FIXTURE_BACKUP: &str =
    "tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92";

fn local_fixture_backup_path() -> PathBuf {
    workspace_root().join(LOCAL_FIXTURE_BACKUP)
}

lazy_static::lazy_static! {
    pub static ref TEST_DB_NODES: Result<Arc<Database>, Error> = {
        let db = Database::init_with_schema()?;
        let target_file = local_fixture_backup_path();
        db.import_backup_with_embeddings(&target_file)
            .map_err(ploke_error::Error::from)?;

        let expected_set = EmbeddingSet::default();
        let embedding_count = db
            .count_embeddings_for_set(&expected_set)
            .map_err(ploke_error::Error::from)?;
        if embedding_count == 0 {
            return Err(Error::from(DbError::Cozo(format!(
                "Fixture backup {} does not contain embeddings for the default local set {}",
                target_file.display(),
                expected_set.rel_name
            ))));
        }

        db.set_active_set(expected_set.clone())
            .map_err(ploke_error::Error::from)?;
        create_index_for_set(&db, &expected_set).map_err(ploke_error::Error::from)?;
        Ok(Arc::new(db))
    };
}

async fn setup_rag() -> Result<RagService, Error> {
    let db = TEST_DB_NODES
        .as_ref()
        .expect("Must set up TEST_DB_NODES correctly.");

    let model = LocalEmbedder::new(EmbeddingConfig::default())?;
    let source = EmbeddingSource::Local(model);
    let embedding_processor = EmbeddingProcessor::new(source);
    let embedding_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        embedding_processor,
    ));
    RagService::new(db.clone(), embedding_runtime).map_err(Into::into)
}

async fn fetch_snippet_containing(
    db: &Arc<Database>,
    ordered_node_ids: Vec<uuid::Uuid>,
    search_term: &str,
) -> Result<String, Error> {
    let node_info: Vec<EmbeddingData> = db.get_nodes_ordered(ordered_node_ids)?;
    let io_handle = IoManagerHandle::new();

    let snippet_find: Vec<String> = io_handle
        .get_snippets_batch(node_info)
        .await
        .expect("Problem receiving")
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    snippet_find
        .into_iter()
        .find(|snip| snip.contains(search_term))
        .ok_or_else(|| {
            ploke_error::Error::Internal(ploke_error::internal::InternalError::CompilerError(
                format!("No snippet found for term {search_term}"),
            ))
        })
}

// #[tokio::test]
// async fn test_bm25_exact_identifier() -> Result<(), Error> {
//     let rag = setup_rag().await?;
//
//     rag.bm25_rebuild().await?;
//     let hits = rag.search_bm25("ComplexGenericTrait", 15).await?;
//     assert!(hits.iter().any(|(_, score)| *score > 0.0));
//     Ok(())
// }

#[cfg(test)]
mod benches {
    use criterion::{criterion_group, criterion_main, Criterion};
    use std::hint::black_box;
    use tokio::runtime::Runtime;

    // fn bench_bm25_exact_identifier(c: &mut Criterion) {
    //     let rt = Runtime::new().unwrap();
    //     c.bench_function("bm25_exact_identifier", |b| {
    //         b.to_async(&rt).iter(|| async {
    //             let rag = super::setup_rag().await.unwrap();
    //             rag.bm25_rebuild().await.unwrap();
    //             let hits = rag
    //                 .search_bm25(black_box("ComplexGenericTrait"), black_box(15))
    //                 .await
    //                 .unwrap();
    //             black_box(hits);
    //         });
    //     });
    // }

    // criterion_group!(benches, bench_bm25_exact_identifier);
    // criterion_main!(benches);
}
