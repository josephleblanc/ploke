use super::{ErrorPolicy, Result};

/// Extension trait for `Result` enabling policy-driven emission without
/// contaminating core control-flow with side-effects.
/// 
/// Typical usage: at subsystem boundaries in applications, call one of the
/// helpers to emit errors via your chosen [`ErrorPolicy`], while preserving
/// the original result for further handling.
/// 
/// Example
/// ```rust,ignore
/// use ploke_error::{Result, ResultExt, ErrorPolicy, DomainError};
/// 
/// fn do_work(policy: &impl ErrorPolicy) -> Result<()> {
///     let r: Result<()> = Err(DomainError::Ui { message: "bad input".into() }.into());
///     r.emit_error(policy) // Emitted according to policy, still Err for caller to handle
/// }
/// ```
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

//// Iterator helpers over `Result` to reduce boilerplate at boundaries.
//!
//! - `collect_ok`: eagerly collects `Ok` items, returning the first `Error`
///   (equivalent to `collect::<Result<Vec<_>, _>>()` but clearer at call sites).
//! - `first_error`: scans and returns the first `Error` without allocation.
//!
//! Example
//! ```rust,ignore
//! use ploke_error::{Result, result_ext::IterResultExt, DomainError};
//!
//! let items: Vec<Result<u32>> = vec![Ok(1), Ok(2), Err(DomainError::Io { message: "disk".into() }.into())];
//! assert!(items.clone().first_error().is_some());
//! let collected = items.collect_ok(); // -> Err(_)
//! ```
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
