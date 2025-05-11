mod context;
mod fatal;
mod internal;
mod warning;

// public exports
pub use context::{ContextualError, ErrorContext};
pub use fatal::FatalError;
pub use warning::WarningError;

// common imports for submodules
use std::backtrace::Backtrace;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Fatal(#[from] FatalError),
    #[error(transparent)]
    Warning(#[from] WarningError),
    #[error(transparent)]
    Internal(#[from] internal::InternalError),
}

// Remove test function
