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

use tracing::{error, warn};

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: std::fmt::Debug,
{
    fn emit_event(self, severity: ErrorSeverity) -> Self {
        if let Err(err) = self.as_ref() {
            match severity {
                ErrorSeverity::Warning => warn!(target: "ploke_tui::error", "Warning: {:?}", err),
                ErrorSeverity::Error => error!(target: "ploke_tui::error", "Error: {:?}", err),
                ErrorSeverity::Fatal => error!(target: "ploke_tui::error", "Fatal: {:?}", err),
            }
        }
        self
    }

    fn emit_warning(self) -> Self {
        self.emit_event(ErrorSeverity::Warning)
    }

    fn emit_error(self) -> Self {
        self.emit_event(ErrorSeverity::Error)
    }

    fn emit_fatal(self) -> Self {
        self.emit_event(ErrorSeverity::Fatal)
    }
}

impl<E> ErrorExt for E
where
    E: std::fmt::Debug,
{
    fn emit_event(&self, severity: ErrorSeverity) {
        match severity {
            ErrorSeverity::Warning => warn!(target: "ploke_tui::error", "Warning: {:?}", self),
            ErrorSeverity::Error => error!(target: "ploke_tui::error", "Error: {:?}", self),
            ErrorSeverity::Fatal => error!(target: "ploke_tui::error", "Fatal: {:?}", self),
        }
    }
}
