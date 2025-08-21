use super::{Error, Severity};

//// A policy for classifying and emitting errors.
//!
//! Libraries should not log or print directly; instead, they return [`crate::Result`] and let
//! the application install an `ErrorPolicy` to decide how to present or route errors.
//!
//! Classification is coarse-grained via [`Severity`]. Emission can be anything:
//! - tracing logs
//! - UI event bus
//! - structured diagnostics
//! - custom telemetry
//!
//! Example
//! ```rust,ignore
//! use ploke_error::{ErrorPolicy, Severity, Error};
//!
//! struct PrintPolicy;
//! impl ErrorPolicy for PrintPolicy {
//!     fn classify(&self, e: &Error) -> Severity { e.severity() }
//!     fn emit(&self, e: &Error) { eprintln!("[{:?}] {e}", self.classify(e)); }
//! }
//! ```
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

#[cfg(feature = "diagnostic")]
#[derive(Debug, Clone, Default)]
pub struct MiettePolicy;

#[cfg(feature = "diagnostic")]
impl ErrorPolicy for MiettePolicy {
    fn classify(&self, error: &Error) -> Severity {
        error.severity()
    }

    fn emit(&self, error: &Error) {
        // Render a rich diagnostic when available; fallback to Display otherwise.
        let report = miette::Report::new(error.clone());
        eprintln!("{report}");
    }
}
//// A composite policy that delegates to multiple policies.
//!
//! Behavior
//! - classify: returns the maximum severity among inner policies (defaulting to the error's own severity when empty).
//! - emit: delegates emission to all inner policies in insertion order.
//!
//! Example
//! ```rust,ignore
//! use ploke_error::{policy::{CombinedPolicy, NoopPolicy}, ErrorPolicy};
//! let policy = CombinedPolicy::new()
//!     .push(NoopPolicy::default());
//! // Optionally add feature-gated policies:
//! // #[cfg(feature = "tracing")] let policy = policy.push(ploke_error::policy::TracingPolicy::default());
//! // #[cfg(feature = "diagnostic")] let policy = policy.push(ploke_error::policy::MiettePolicy::default());
//! ```
#[derive(Default)]
pub struct CombinedPolicy {
    policies: Vec<Box<dyn ErrorPolicy>>,
}

impl CombinedPolicy {
    /// Create an empty CombinedPolicy.
    pub fn new() -> Self {
        Self { policies: Vec::new() }
    }

    /// Pre-allocate capacity for N policies.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { policies: Vec::with_capacity(capacity) }
    }

    /// Construct from an existing vector of boxed policies.
    pub fn from_vec(policies: Vec<Box<dyn ErrorPolicy>>) -> Self {
        Self { policies }
    }

    /// Add a policy by value (boxed internally). Consumes and returns Self for builder-style chaining.
    pub fn push<P: ErrorPolicy + 'static>(mut self, policy: P) -> Self {
        self.policies.push(Box::new(policy));
        self
    }

    /// Add an already boxed policy. Consumes and returns Self for builder-style chaining.
    pub fn add_boxed(mut self, policy: Box<dyn ErrorPolicy>) -> Self {
        self.policies.push(policy);
        self
    }
}

impl ErrorPolicy for CombinedPolicy {
    fn classify(&self, error: &Error) -> Severity {
        if self.policies.is_empty() {
            return error.severity();
        }
        let mut sev = error.severity();
        let mut rank = severity_rank(sev);
        for p in &self.policies {
            let s = p.classify(error);
            let r = severity_rank(s);
            if r > rank {
                rank = r;
                sev = s;
            }
        }
        sev
    }

    fn emit(&self, error: &Error) {
        for p in &self.policies {
            p.emit(error);
        }
    }
}

fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Warning => 0,
        Severity::Error => 1,
        Severity::Fatal => 2,
    }
}
