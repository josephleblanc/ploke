use super::{ErrorPolicy, Result};

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

/// Iterator helpers over Result to reduce boilerplate at boundaries.
/// - collect_ok: eagerly collects Ok items, returning the first Error.
/// - first_error: scans and returns the first Error without allocation.
pub trait IterResultExt<T>: Sized {
    fn collect_ok(self) -> Result<Vec<T>>;
    fn first_error(self) -> Option<super::Error>;
}

impl<I, T> IterResultExt<T> for I
where
    I: IntoIterator<Item = Result<T>>,
{
    fn collect_ok(self) -> Result<Vec<T>> {
        let mut out = Vec::new();
        for r in self.into_iter() {
            out.push(r?);
        }
        Ok(out)
    }

    fn first_error(self) -> Option<super::Error> {
        for r in self.into_iter() {
            if let Err(e) = r {
                return Some(e);
            }
        }
        None
    }
}
