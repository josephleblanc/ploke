#[derive(Debug, thiserror::Error)]
pub enum InternalError {
    #[error("Internal compiler error: {0}")]
    CompilerError(String),

    #[error("Unexpected state: {0}")]
    InvalidState(String),

    #[error("Feature not implemented: {0}")]
    NotImplemented(String),
}
