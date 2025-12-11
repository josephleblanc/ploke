#![doc = r#"
ploke-error — Workspace-wide error types, severity, and policy-driven emission.

Overview
- A single Error enum shared across crates.
- Severity classification for coarse, programmatic handling.
- DomainError for structured, non-fatal domain failures.
- Result alias for ergonomic propagation.
- Context and iterator extensions to reduce boilerplate.
- Policy-driven emission via ErrorPolicy so libraries stay side-effect free.

Quickstart
- Library code should:
  - return ploke_error::Result<T>
  - create structured errors (Fatal/Internal/Domain/Warning)
  - use ContextExt and ResultExt helpers when helpful
- Application code (binaries, TUI) should:
  - choose an ErrorPolicy (e.g., TracingPolicy, MiettePolicy, or CombinedPolicy)
  - optionally emit errors at boundaries without interleaving side-effects in core logic

Example: return Result and propagate with ?
```rust,ignore
use ploke_error::{Result, DomainError};

fn parse_user_input(s: &str) -> Result<usize> {
    s.trim().parse::<usize>().map_err(|e| {
        // Map into a structured domain error; caller decides how to emit.
        ploke_error::Error::from(DomainError::Ui { message: format!("invalid number: {e}") })
    })
}
```

Example: policy-driven emission at the boundary
```rust,ignore
use ploke_error::{Result, ErrorPolicy, ResultExt, policy::NoopPolicy};

fn handle_request(policy: &impl ErrorPolicy) -> Result<()> {
    parse_user_input("42").emit_event(policy)?;
    Ok(())
}

// In application initialization:
// let policy = NoopPolicy::default(); // or a custom policy that logs/traces
```

Example: combining policies for tracing + diagnostics
```rust,ignore
use ploke_error::{policy::CombinedPolicy, ErrorPolicy};

fn app_policy() -> CombinedPolicy {
    // Feature-gated policies can be combined; order determines emission order.
    let policy = CombinedPolicy::new()
        .push(ploke_error::policy::NoopPolicy::default());
    // When enabling optional features:
    // #[cfg(feature = "tracing")] let policy = policy.push(ploke_error::policy::TracingPolicy::default());
    // #[cfg(feature = "diagnostic")] let policy = policy.push(ploke_error::policy::MiettePolicy::default());
    policy
}
```

Example: iterator ergonomics
```rust,ignore
use ploke_error::{Result, result_ext::IterResultExt};

fn gather(values: &[&str]) -> Result<Vec<usize>> {
    let iter = values.iter().map(|s| s.parse::<usize>().map_err(|e| {
        ploke_error::Error::from(ploke_error::DomainError::Transform { message: e.to_string() })
    }));
    iter.collect_ok()
}
```

Feature flags
- tracing: enables TracingPolicy
- diagnostic: enables MiettePolicy and miette::Diagnostic impls
- serde: enables Serialize/Deserialize on a subset of types (e.g., Severity, DomainError, WarningError)

Guidance
- Prefer structured Error variants and DomainError over ad-hoc strings.
- Use ErrorPolicy to classify/emit; avoid logging in library code.
- ContextExt captures context lazily; add it only where it improves UX.
"#]

pub mod context;
pub mod domain;
pub mod fatal;
pub mod internal;
pub mod policy;
#[cfg(feature = "serde")]
pub mod pretty;
pub mod result_ext;
pub mod severity;
pub mod warning;

// public exports
pub use context::{ContextExt, ContextualError, ErrorContext, SourceSpan};
pub use domain::DomainError;
pub use fatal::FatalError;
pub use internal::InternalError;
#[cfg(feature = "diagnostic")]
pub use policy::MiettePolicy;
#[cfg(feature = "tracing")]
pub use policy::TracingPolicy;
pub use policy::{CombinedPolicy, ErrorPolicy, NoopPolicy};
#[cfg(feature = "serde")]
pub use pretty::PrettyDebug;
pub use result_ext::{IterResultExt, ResultExt};
pub use severity::Severity;
pub use warning::WarningError;

// common imports for submodules

/// Workspace-wide result alias used by all crates in the project.
/// The default error type is this crate's [`Error`].
/// Use this throughout library code and propagate failures with `?`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Top-level error type used across the ploke workspace.
///
/// Variants group failures into coarse classes; see [`Error::severity`] for programmatic classification.
/// Prefer returning `Result<T>` from functions and let callers decide how to emit via an [`policy::ErrorPolicy`].
///
/// Migration note:
/// - Prefer [`Error::Domain`] with a specific [`DomainError`] over deprecated stringly variants.
/// - Emission/logging should be performed by an application-supplied policy rather than inline in libraries.
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
    // #[deprecated(note = "Use Error::Domain(DomainError::Ui { message }) instead.")]
    #[error("{msg} {0}", msg = "UiError: ")]
    UiError(String),
    // #[deprecated(note = "Use Error::Domain(DomainError::Transform { message }) instead.")]
    #[error("{msg} {0}", msg = "TransformError: ")]
    TransformError(String),
}

impl Error {
    /// Returns true if this error is a Warning variant.
    ///
    /// Useful for quick classification when a caller wants to continue
    /// processing while recording non-fatal issues.
    pub fn is_warning(&self) -> bool {
        matches!(self, Error::Warning(_))
    }

    /// Coarse severity classification for programmatic handling.
    ///
    /// Typical usage:
    /// - map severity to logging level
    /// - decide whether to continue or abort a loop
    /// - route errors to different channels/handlers in an application
    pub fn severity(&self) -> Severity {
        match self {
            Error::Warning(_) => Severity::Warning,
            Error::Fatal(_) => Severity::Fatal,
            Error::Internal(_)
            | Error::Domain(_)
            | Error::Context(_)
            | Error::UiError(_)
            | Error::TransformError(_) => Severity::Error,
        }
    }
}
