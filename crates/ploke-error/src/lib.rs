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
    // NOTE: Unsure about error design. Using a simple wrapper around the type of error coming out of a
    // target crate for convenience for now.
    #[error("{msg} {0}", msg = "UiError: ")]
    UiError(String),
    #[error("{msg} {0}", msg = "TransformError: ")]
    TransformError(String),
}


// Remove test function
