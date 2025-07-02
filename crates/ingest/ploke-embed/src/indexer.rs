use std::sync::Arc;
use ploke_core::TrackingHash;
use ploke_db::{embedding::EmbeddingNode, Database};
use ploke_io::{IoManagerHandle, SnippetRequest};
use tracing::{info_span, instrument};

use crate::{embedding_service::EmbeddingService, error::BatchError};

pub struct IndexerTask {
    db: Arc<Database>,
    io: IoManagerHandle,
    cancellation_token: CancellationToken,
    batch_size: usize,
}

impl IndexerTask {
    async fn run(&self) -> Result {
        while let Some(batch) = self.next_batch().await? {
            process_batch(batch)?;
        }
    }

    async fn next_batch(&self) -> Result {
        todo!()
    }
}


/// Processes a batch of nodes for embedding generation                       
#[instrument(skip_all, fields(batch_size = nodes.len()))]
pub async fn process_batch(
    db: &Database,
    io_manager: &IoManagerHandle,
    embedding_service: &dyn EmbeddingService,
    nodes: Vec<EmbeddingNode>,
    report_progress: impl Fn(usize, usize) + Send + Sync,
) -> Result<(), BatchError> {
    let ctx_span = info_span!("process_batch");
    let _guard = ctx_span.enter();

    // Convert nodes to snippet requests
    let requests = nodes
        .iter()
        .map(|node| SnippetRequest {
            path: node.path.clone(),
            content_hash: TrackingHash(node.content_hash),
            start: node.start_byte,
            end: node.end_byte,
        })
        .collect::<Vec<_>>();

    // Fetch snippets
    let snippet_results = io_manager
        .get_snippets_batch(requests)
        .await
        .map_err(|e| BatchError::SnippetFetch(e.into()))?;

    // Process embeddings
    let mut updates = Vec::with_capacity(nodes.len());
    for (i, (node, snippet_result)) in nodes.into_iter().zip(snippet_results).enumerate() {
        report_progress(i, nodes.len());

        let snippet = match snippet_result {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Skipping node {}: {:?}", node.id, e);
                continue;
            }
        };

        let embedding = embedding_service
            .compute_embedding(&snippet)
            .await
            .map_err(BatchError::Embedding)?;

        // Validate vector dimensions
        if embedding.len() != embedding_service.dimensions() {
            return Err(BatchError::DimensionMismatch {
                expected: embedding_service.dimensions(),
                actual: embedding.len(),
            });
        }

        updates.push((node.id, embedding));
    }

    // Update database in bulk
    db.update_embeddings_batch(updates)
        .await
        .map_err(BatchError::Database)?;

    report_progress(nodes.len(), nodes.len());
    Ok(())
}

// pub enum EmbeddingService {
//     Local(Model),
//     Remote(Service),
// }
//
// pub struct Model {}
//
// pub struct Service {}

pub struct CancellationToken {}
