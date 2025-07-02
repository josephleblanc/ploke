pub type EmbeddingVector = Vec<f32>;

pub trait EmbeddingService {
    fn compute_embedding(&self, snippet: &str) -> BoxFuture<'_, Result<EmbeddingVector, ploke_error::Error> >;

    fn dimensions(&self) -> usize;
}
