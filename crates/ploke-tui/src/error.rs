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
