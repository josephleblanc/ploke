use std::sync::Arc;

//// Errors indicating bugs, invalid states, or infrastructure failures inside the system.
//!
//! These map to [`crate::Severity::Error`] and usually warrant investigation.
#[cfg_attr(feature = "diagnostic", derive(miette::Diagnostic))]
#[derive(Clone, Debug, thiserror::Error)]
pub enum InternalError {
    #[error("Internal compiler error: {0}")]
    CompilerError(String),

    #[error("Unexpected state: {0}")]
    InvalidState(&'static str),

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
