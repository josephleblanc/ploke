use super::*;
use std::backtrace::Backtrace;

#[derive(Debug, thiserror::Error)]
pub enum InternalError {
    #[error("Internal compiler error: {0}")]
    CompilerError(String, #[backtrace] Backtrace),
    
    #[error("Unexpected state: {0}")]
    InvalidState(String, #[backtrace] Backtrace),
    
    #[error("Feature not implemented: {0}")]
    NotImplemented(String, #[backtrace] Backtrace),
}
