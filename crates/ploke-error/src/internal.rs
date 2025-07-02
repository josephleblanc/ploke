use std::sync::Arc;

#[derive(Clone, Debug, thiserror::Error)]
pub enum InternalError {
    #[error("Internal compiler error: {0}")]
    CompilerError(String),

    #[error("Unexpected state: {0}")]
    InvalidState(String),

    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    #[error("Embedding error: {0}")]
    EmbedderError(Arc<dyn std::error::Error + Send + Sync>),
}

impl InternalError {
    pub fn embedder_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> Self {
        Self::EmbedderError(Arc::new(e))
    }
}
