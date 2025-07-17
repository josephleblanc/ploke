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
}

impl Error {
    pub fn is_warning(&self) -> bool {
        matches!(self, Error::Warning(_))
    }
}

/// Severity levels for error events
#[derive(Debug, Clone, Copy)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Fatal,
}

/// Extension trait for ergonomic error emission
pub trait ResultExt<T> {
    /// Emit an error event through the global event bus
    fn emit_event(self, severity: ErrorSeverity) -> Self;
    
    /// Emit a warning event
    fn emit_warning(self) -> Self;
    
    /// Emit an error event
    fn emit_error(self) -> Self;
    
    /// Emit a fatal event
    fn emit_fatal(self) -> Self;
}

/// Extension trait for direct error emission
pub trait ErrorExt {
    fn emit_event(&self, severity: ErrorSeverity);
    fn emit_warning(&self) { self.emit_event(ErrorSeverity::Warning) }
    fn emit_error(&self) { self.emit_event(ErrorSeverity::Error) }
    fn emit_fatal(&self) { self.emit_event(ErrorSeverity::Fatal) }
}
