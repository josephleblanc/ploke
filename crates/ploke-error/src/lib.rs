pub mod context;
pub mod fatal;
pub mod internal;
pub mod warning;

// public exports
pub use context::{ContextualError, ErrorContext};
pub use fatal::FatalError;
pub use internal::InternalError;
pub use warning::WarningError;

// common imports for submodules
use std::path::PathBuf;

#[derive(Debug, Clone, thiserror::Error)]
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
    // #[error("{msg} {0}", msg = "PlokeDbError: ")]
    // DbError(String)
}


// Remove test function
