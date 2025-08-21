use super::{Error, ErrorPolicy, Result};

/// Extension trait for Result to enable policy-driven emission
pub trait ResultExt<T> {
    /// Emit the error using the provided policy and return the result unchanged
    fn emit_event(self, policy: &impl ErrorPolicy) -> Self;
    
    /// If the result is an error, emit it as a warning using the policy
    fn emit_warning(self, policy: &impl ErrorPolicy) -> Self;
    
    /// If the result is an error, emit it as an error using the policy
    fn emit_error(self, policy: &impl ErrorPolicy) -> Self;
    
    /// If the result is an error, emit it as a fatal using the policy
    fn emit_fatal(self, policy: &impl ErrorPolicy) -> Self;
}

impl<T> ResultExt<T> for Result<T> {
    fn emit_event(self, policy: &impl ErrorPolicy) -> Self {
        if let Err(ref e) = self {
            policy.emit(e);
        }
        self
    }

    fn emit_warning(self, policy: &impl ErrorPolicy) -> Self {
        if let Err(ref e) = self {
            if policy.classify(e) == super::Severity::Warning {
                policy.emit(e);
            }
        }
        self
    }

    fn emit_error(self, policy: &impl ErrorPolicy) -> Self {
        if let Err(ref e) = self {
            if policy.classify(e) == super::Severity::Error {
                policy.emit(e);
            }
        }
        self
    }

    fn emit_fatal(self, policy: &impl ErrorPolicy) -> Self {
        if let Err(ref e) = self {
            if policy.classify(e) == super::Severity::Fatal {
                policy.emit(e);
            }
        }
        self
    }
}
