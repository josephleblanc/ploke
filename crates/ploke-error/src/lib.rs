pub mod context;
pub mod fatal;
pub mod internal;
pub mod warning;
pub mod domain;
pub mod severity;

// public exports
pub use context::{ContextualError, ErrorContext};
pub use fatal::FatalError;
pub use internal::InternalError;
pub use warning::WarningError;
pub use domain::DomainError;
pub use severity::Severity;

// common imports for submodules
use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Fatal(#[from] FatalError),
    #[error(transparent)]
    Warning(#[from] WarningError),
    #[error(transparent)]
    Internal(#[from] InternalError),
    #[error(transparent)]
    Domain(#[from] DomainError),

    // NOTE: Unsure about error design. Using a simple wrapper around the type of error coming out of a
    // target crate for convenience for now.
    #[error("{msg} {0}", msg = "UiError: ")]
    UiError(String),
    #[error("{msg} {0}", msg = "TransformError: ")]
    TransformError(String),
}

impl Error {
    pub fn is_warning(&self) -> bool {
        matches!(self, Error::Warning(_))
    }

    pub fn severity(&self) -> Severity {
        match self {
            Error::Warning(_) => Severity::Warning,
            Error::Fatal(_) => Severity::Fatal,
            Error::Internal(_) | Error::Domain(_) | Error::UiError(_) | Error::TransformError(_) => Severity::Error,
        }
    }
}
