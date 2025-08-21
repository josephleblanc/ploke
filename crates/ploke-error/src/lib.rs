pub mod context;
pub mod fatal;
pub mod internal;
pub mod warning;
pub mod domain;
pub mod severity;
pub mod policy;
pub mod result_ext;

 // public exports
pub use context::{ContextualError, ErrorContext, SourceSpan, ContextExt};
pub use fatal::FatalError;
pub use internal::InternalError;
pub use warning::WarningError;
pub use domain::DomainError;
pub use severity::Severity;
pub use policy::{ErrorPolicy, NoopPolicy, CombinedPolicy};
#[cfg(feature = "tracing")]
pub use policy::TracingPolicy;
#[cfg(feature = "diagnostic")]
pub use policy::MiettePolicy;
pub use result_ext::{ResultExt, IterResultExt};

 // common imports for submodules

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg_attr(feature = "diagnostic", derive(miette::Diagnostic))]
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
    #[error(transparent)]
    Context(#[from] ContextualError),

    // NOTE: Unsure about error design. Using a simple wrapper around the type of error coming out of a
    // target crate for convenience for now.
    #[deprecated(note = "Use Error::Domain(DomainError::Ui { message }) instead.")]
    #[error("{msg} {0}", msg = "UiError: ")]
    UiError(String),
    #[deprecated(note = "Use Error::Domain(DomainError::Transform { message }) instead.")]
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
            Error::Internal(_) | Error::Domain(_) | Error::UiError(_) | Error::TransformError(_) | Error::Context(_) => Severity::Error,
        }
    }
}
