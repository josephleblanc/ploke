use super::{Error, Severity};

/// A policy for classifying and emitting errors
pub trait ErrorPolicy: Send + Sync {
    /// Classify the error's severity
    fn classify(&self, error: &Error) -> Severity;
    
    /// Emit the error according to the policy (e.g., log, send to UI, etc.)
    fn emit(&self, error: &Error);
}

/// A no-operation policy that does nothing
#[derive(Debug, Clone, Default)]
pub struct NoopPolicy;

impl ErrorPolicy for NoopPolicy {
    fn classify(&self, error: &Error) -> Severity {
        error.severity()
    }

    fn emit(&self, _error: &Error) {
        // Intentionally do nothing
    }
}

/// A policy that uses the error's default severity and emits via tracing
#[cfg(feature = "tracing")]
#[derive(Debug, Clone, Default)]
pub struct TracingPolicy;

#[cfg(feature = "tracing")]
impl ErrorPolicy for TracingPolicy {
    fn classify(&self, error: &Error) -> Severity {
        error.severity()
    }

    fn emit(&self, error: &Error) {
        use tracing::{event, Level};
        
        match error.severity() {
            Severity::Warning => event!(Level::WARN, error = %error),
            Severity::Error | Severity::Fatal => event!(Level::ERROR, error = %error),
        }
    }
}
