use wiremock::{Mock, MockServer, ResponseTemplate, http::Method};
use ploke_db::Database;
use ploke_embed::{indexer::{IndexerTask, EmbeddingProcessor, CozoBackend}, error::truncate_string};
use crate::helpers::{mock_db::create_mock_db, mock_embed::MockEmbedder};
use tokio::sync::{broadcast, mpsc};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn full_indexing_cycle() {
    // Create test database with 10 unindexed nodes
    let db = create_mock_db(10);
    
    // Initialize mock embedder
    let processor = EmbeddingProcessor::new(
        EmbeddingSource::Mock(MockEmbedder { 
            dimensions: 384, 
            behavior: MockBehavior::Normal 
        })
    );
    
    // Setup indexer
    let (progress_tx, _) = broadcast::channel(100);
    let (control_tx, _) = mpsc::channel(4);
    let indexer = IndexerTask {
        db: db.clone(),
        io: todo!(), // Use actual mock in real implementation
        embedding_processor: processor,
        cancellation_token: Default::default(),
        batch_size: 5,
    };
    
    // Execute full indexing
    indexer.run(progress_tx, control_tx).await.unwrap();
    
    // Verify all nodes indexed
    assert_eq!(db.count_pending_embeddings().unwrap(), 0);
}
