mod context;
mod fatal;
mod internal;
mod warning;

// public exports
pub use context::{ContextualError, ErrorContext};
pub use fatal::FatalError;
pub use internal::InternalError;
pub use warning::WarningError;

// common imports for submodules
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Fatal(#[from] FatalError),
    #[error(transparent)]
    Warning(#[from] WarningError),
    #[error(transparent)]
    Internal(#[from] InternalError),
}

// Remove test function
