use std::sync::Arc;

#[derive(thiserror::Error, Debug, Clone)]
pub enum EmbedError {
    #[error("Snippet fetch failed: {0}")]
    SnippetFetch(#[from] ploke_io::IoError),

    #[error("Embedding computation failed: {0}")]
    Embedding(String),

    #[error("Database operation failed: {0}")]
    Database(#[from] ploke_db::DbError),

    #[error("Local model error: {0}")]
    LocalModel(String),

    #[error("Hugging Face API error: {0}")]
    HuggingFace(#[from] HuggingFaceError),

    #[error("OpenAI API error: {0}")]
    OpenAI(#[from] OpenAIError),

    #[error("Network Error: {0}")]
    Network(String),

    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Cancelled operation: {0}")]
    Cancelled(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Query error: {0}")]
    QueryError(String),
    
    #[error("Ploke core error: {0}")]
    PlokeCore(#[from] ploke_error::Error),

    #[error("Broadcast send error: {0}")]
    BroadcastSendError(String),
}

impl From<tokio::sync::broadcast::error::SendError<crate::indexer::IndexingStatus>> for EmbedError {
    fn from(e: tokio::sync::broadcast::error::SendError<crate::indexer::IndexingStatus>) -> Self {
        EmbedError::BroadcastSendError(e.to_string())
    }
}

impl From<candle_core::Error> for EmbedError {
    fn from(e: candle_core::Error) -> Self {
        EmbedError::LocalModel(e.to_string())
    }
}

impl From<hf_hub::api::tokio::ApiError> for EmbedError {
    fn from(e: hf_hub::api::tokio::ApiError) -> Self {
        EmbedError::Config(e.to_string())
    }
}

impl From<tokenizers::Error> for EmbedError {
    fn from(e: tokenizers::Error) -> Self {
        EmbedError::Embedding(e.to_string())
    }
}

impl From<reqwest::Error> for EmbedError {
    fn from(e: reqwest::Error) -> Self {
        EmbedError::Network(e.to_string())
    }
}

impl From<EmbedError> for ploke_error::Error {
    fn from(e: EmbedError) -> Self {
        ploke_error::Error::Internal(ploke_error::InternalError::EmbedderError(Arc::new(e)))
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum HuggingFaceError {
    #[error("API error: status {status}, body {body}")]
    ApiError { status: u16, body: String },
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum OpenAIError {
    #[error("API error: status {status}, body {body}")]
    ApiError { status: u16, body: String },
}
