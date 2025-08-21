#[derive(Debug, Clone, thiserror::Error)]
pub enum DomainError {
    #[error("UI error: {message}")]
    Ui { message: String },

    #[error("Transform error: {message}")]
    Transform { message: String },

    #[error("Database error: {message}")]
    Db { message: String },

    #[error("IO error: {message}")]
    Io { message: String },

    #[error("RAG error: {message}")]
    Rag { message: String },

    #[error("Ingest error: {message}")]
    Ingest { message: String },
}
