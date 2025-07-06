use super::*;
use crate::helpers::mock_embed::MockBehavior;
use ploke_embed::{indexer::IndexingStatus, error::{EmbedError, truncate_string}};
use mockall::{predicate::*, Sequence};

#[tokio::test]
async fn http_error_propagation() {
    // Setup
    let (mut progress_rx, state) = setup_test_environment(
        MockBehavior::RateLimited,
        10
    ).await;
    
    // Capture progress state
    let mut status: Option<IndexingStatus> = None;
    while let Ok(progress) = progress_rx.recv().await {
        if progress.total > 0 { 
            status = Some(progress);
            break;
        }
    }
    
    // Add generated embeddings
    run_embedding_phase(&state).await;

    // Verify error
    let status = status.unwrap();
    assert!(!status.errors.is_empty());
    assert!(status.errors[0].contains("429"));
    assert!(status.errors[0].contains("Rate Limited"));
}
