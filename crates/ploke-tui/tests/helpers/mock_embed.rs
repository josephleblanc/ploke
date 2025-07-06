use crate::helpers::mock_db::create_mock_db;
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource, EmbedError};
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, PartialEq, Eq)]
pub enum MockBehavior {
    Normal,
    RateLimited,
    DimensionMismatch,
    NetworkError,
}

pub struct MockEmbedder {
    pub dimensions: usize,
    pub behavior: MockBehavior,
}

#[async_trait]
impl EmbeddingSource for MockEmbedder {
    async fn generate_embeddings(
        &self,
        snippets: Vec<String>
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        match self.behavior {
            MockBehavior::Normal => Ok(vec![vec![0.0; self.dimensions]; snippets.len()]),
            MockBehavior::RateLimited => Err(EmbedError::HttpError {
                status: 429,
                body: "Rate Limited".to_string(),
                url: "https://mock-api".to_string(),
            }),
            MockBehavior::DimensionMismatch => 
                Ok(vec![vec![0.0; self.dimensions + 1]; snippets.len()]),
            MockBehavior::NetworkError => 
                Err(EmbedError::Network("Connection failed".to_string()))
        }
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}
