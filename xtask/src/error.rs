//! Error types for xtask commands.
//!
//! This module provides the unified error type [`XtaskError`] used throughout
//! the xtask system, along with recovery hints and context enrichment.

use std::error::Error;
use std::fmt;

/// Appended to [`XtaskError::Parse`] in `Display` so CLI users see next diagnostic steps.
///
/// Kept out of [`XtaskError::recovery_suggestion`] for `Parse` to avoid duplicating this block when
/// using [`XtaskError::print_report`], which prints both `Display` and recovery.
pub const PARSE_FAILURE_DIAGNOSTIC_HINT: &str = "\
Hint: See `cargo xtask help parse` and `cargo xtask parse debug --help`. \
For parse/workspace failures, run `parse debug workspace <PATH>`; for a failing crate root, run \
`parse debug logical-paths`, `parse debug modules-premerge`, and `parse debug path-collisions`.";

/// Unified error type for xtask commands.
///
/// This enum provides a consistent error type that can represent:
/// - Generic errors with messages
/// - Parse errors from syn_parser
/// - Database errors from ploke_db
/// - Resource errors
/// - Validation errors with recovery hints
#[derive(Debug, Clone)]
pub enum XtaskError {
    /// Generic error with a message.
    Generic(String),

    /// Parse error from syn_parser.
    Parse(String),

    /// Database error from ploke_db.
    Database(String),

    /// Resource error (e.g., missing file, unavailable service).
    Resource(String),

    /// Validation error with optional recovery suggestion.
    Validation {
        /// Context about what was being validated.
        context: String,
        /// Optional recovery suggestion.
        recovery: Option<String>,
    },

    /// IO error.
    Io(String),

    /// Serialization error.
    Serialization(String),

    /// Internal error (for bugs/unexpected conditions).
    Internal(String),
}

impl XtaskError {
    /// Create a new generic error.
    ///
    /// # Example
    /// ```
    /// use xtask::error::XtaskError;
    ///
    /// let err = XtaskError::new("Something went wrong");
    /// ```
    pub fn new(message: impl Into<String>) -> Self {
        Self::Generic(message.into())
    }

    /// Create a new validation error.
    ///
    /// # Example
    /// ```
    /// use xtask::error::XtaskError;
    ///
    /// let err = XtaskError::validation("Invalid input")
    ///     .with_recovery("Use --help to see valid options");
    /// ```
    pub fn validation(context: impl Into<String>) -> ValidationBuilder {
        ValidationBuilder {
            context: context.into(),
        }
    }

    /// Get a recovery suggestion if available.
    ///
    /// Returns a helpful message suggesting how to recover from the error.
    pub fn recovery_suggestion(&self) -> Option<&str> {
        match self {
            Self::Validation { recovery, .. } => recovery.as_deref(),
            Self::Resource(_) => Some("Verify that required resources are available."),
            Self::Database(_) => Some("Check database connectivity and permissions."),
            Self::Io(_) => Some("Check file permissions and disk space."),
            Self::Internal(_) => Some("This may be a bug. Please report it."),
            // Full guidance is included in `Display` for `Parse` (see `fmt::Display`).
            Self::Parse(_) => None,
            _ => None,
        }
    }

    /// Check if this error is a validation error.
    pub fn is_validation(&self) -> bool {
        matches!(self, Self::Validation { .. })
    }
}

impl fmt::Display for XtaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generic(msg) => write!(f, "{}", msg),
            Self::Parse(msg) => write!(f, "Parse error: {}\n\n{}", msg, PARSE_FAILURE_DIAGNOSTIC_HINT),
            Self::Database(msg) => write!(f, "Database error: {}", msg),
            Self::Resource(msg) => write!(f, "Resource error: {}", msg),
            Self::Validation { context, .. } => write!(f, "Validation error: {}", context),
            Self::Io(err) => write!(f, "IO error: {}", err),
            Self::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl Error for XtaskError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        // Note: Io variant stores String, so no source available
        None
    }
}

impl From<String> for XtaskError {
    fn from(value: String) -> Self {
        Self::Generic(value)
    }
}

impl From<&str> for XtaskError {
    fn from(value: &str) -> Self {
        Self::Generic(value.to_string())
    }
}

impl From<std::io::Error> for XtaskError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<serde_json::Error> for XtaskError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<ploke_error::Error> for XtaskError {
    fn from(err: ploke_error::Error) -> Self {
        Self::Database(err.to_string())
    }
}

impl From<ploke_db::DbError> for XtaskError {
    fn from(err: ploke_db::DbError) -> Self {
        Self::Database(err.to_string())
    }
}

/// Builder for constructing validation errors with recovery hints.
///
/// # Example
/// ```
/// use xtask::error::XtaskError;
///
/// let err = XtaskError::validation("Invalid argument")
///     .with_recovery("Use --help to see valid options");
/// ```
pub struct ValidationBuilder {
    context: String,
}

impl ValidationBuilder {
    /// Add a recovery suggestion to the validation error.
    pub fn with_recovery(self, recovery: impl Into<String>) -> XtaskError {
        XtaskError::Validation {
            context: self.context,
            recovery: Some(recovery.into()),
        }
    }
}

impl From<ValidationBuilder> for XtaskError {
    fn from(builder: ValidationBuilder) -> Self {
        XtaskError::Validation {
            context: builder.context,
            recovery: None,
        }
    }
}
