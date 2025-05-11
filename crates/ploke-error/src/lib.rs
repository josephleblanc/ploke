pub mod context;
pub mod fatal;
pub mod internal;
pub mod warning;

pub use context::{ContextualError, ErrorContext};
pub use fatal::FatalError;
pub use warning::WarningError;

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
