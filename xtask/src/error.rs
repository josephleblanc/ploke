//! Error types for xtask commands.
//!
//! This module provides the unified error type [`XtaskError`] used throughout
//! the xtask system, along with recovery hints and context enrichment.

use std::error::Error;
use std::fmt;

/// Unified error type for xtask commands.
///
/// This enum provides a consistent error type that can represent:
/// - Generic errors with messages
/// - Parse errors from syn_parser
/// - Transform errors from ploke_transform
/// - Database errors from ploke_db
/// - Embedding errors from ploke_embed
/// - Resource errors
/// - Validation errors with recovery hints
/// - Command execution failures
#[derive(Debug, Clone)]
pub enum XtaskError {
    /// Generic error with a message.
    Generic(String),

    /// Parse error from syn_parser.
    Parse(String),

    /// Transform error from ploke_transform.
    Transform(String),

    /// Database error from ploke_db.
    Database(String),

    /// Embedding error from ploke_embed.
    Embedding(String),

    /// Resource error (e.g., missing file, unavailable service).
    Resource(String),

    /// Validation error with optional recovery suggestion.
    Validation {
        /// Context about what was being validated.
        context: String,
        /// Optional recovery suggestion.
        recovery: Option<String>,
    },

    /// Command execution failure.
    CommandFailed {
        /// Name of the command that failed.
        command: String,
        /// Reason for the failure.
        reason: String,
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

    /// Create a new internal error.
    ///
    /// Use this for unexpected conditions that indicate a bug.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
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

    /// Add context to an error.
    ///
    /// Wraps the error in a `CommandFailed` variant with the given context.
    pub fn with_context(self, command: impl Into<String>) -> Self {
        Self::CommandFailed {
            command: command.into(),
            reason: self.to_string(),
        }
    }

    /// Get a recovery suggestion if available.
    ///
    /// Returns a helpful message suggesting how to recover from the error.
    pub fn recovery_suggestion(&self) -> Option<&str> {
        match self {
            Self::Validation { recovery, .. } => recovery.as_deref(),
            Self::CommandFailed { .. } => Some("Check the command arguments and try again."),
            Self::Resource(_) => Some("Verify that required resources are available."),
            Self::Database(_) => Some("Check database connectivity and permissions."),
            Self::Io(_) => Some("Check file permissions and disk space."),
            Self::Internal(_) => Some("This may be a bug. Please report it."),
            _ => None,
        }
    }

    /// Check if this error is a validation error.
    pub fn is_validation(&self) -> bool {
        matches!(self, Self::Validation { .. })
    }

    /// Check if this error is an IO error.
    pub fn is_io(&self) -> bool {
        matches!(self, Self::Io(_))
    }

    /// Check if this error is an internal error.
    pub fn is_internal(&self) -> bool {
        matches!(self, Self::Internal(_))
    }

    /// Print a user-friendly error report.
    ///
    /// Outputs the error and any available recovery suggestions to stderr.
    pub fn print_report(&self) {
        eprintln!("\n❌ Error: {}", self);

        if let Some(recovery) = self.recovery_suggestion() {
            eprintln!("\n💡 Recovery suggestion: {}", recovery);
        }

        // Check if there's a tracing log
        if let Ok(log_dir) = std::env::var("PLOKE_TRACE_DIR") {
            eprintln!("\n📝 Trace log available in: {}", log_dir);
            eprintln!(
                "   Search for relevant spans using: rg '{}' {}",
                self.to_string().split_whitespace().next().unwrap_or(""),
                log_dir
            );
        }
    }
}

impl fmt::Display for XtaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generic(msg) => write!(f, "{}", msg),
            Self::Parse(msg) => write!(f, "Parse error: {}", msg),
            Self::Transform(msg) => write!(f, "Transform error: {}", msg),
            Self::Database(msg) => write!(f, "Database error: {}", msg),
            Self::Embedding(msg) => write!(f, "Embedding error: {}", msg),
            Self::Resource(msg) => write!(f, "Resource error: {}", msg),
            Self::Validation { context, .. } => write!(f, "Validation error: {}", context),
            Self::CommandFailed { command, reason, .. } => {
                write!(f, "Command '{}' failed: {}", command, reason)
            }
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

/// Error codes for structured error identification.
///
/// These codes can be used for programmatic error handling and testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Generic error.
    Generic,
    /// Parse-related error.
    ParseError,
    /// Transform-related error.
    TransformError,
    /// Database-related error.
    DatabaseError,
    /// Embedding-related error.
    EmbeddingError,
    /// Resource not found.
    ResourceNotFound,
    /// Validation error.
    ValidationError,
    /// Command not found.
    CommandNotFound,
    /// Invalid arguments.
    InvalidArguments,
    /// Execution timeout.
    Timeout,
    /// Internal error.
    InternalError,
}

impl ErrorCode {
    /// Get a string representation of the error code.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Generic => "E000",
            Self::ParseError => "E100",
            Self::TransformError => "E200",
            Self::DatabaseError => "E300",
            Self::EmbeddingError => "E400",
            Self::ResourceNotFound => "E500",
            Self::ValidationError => "E600",
            Self::CommandNotFound => "E700",
            Self::InvalidArguments => "E800",
            Self::Timeout => "E900",
            Self::InternalError => "E999",
        }
    }
}

/// A recovery hint with structured information.
#[derive(Debug, Clone)]
pub struct RecoveryHint {
    /// Error code for identification.
    pub code: ErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Suggestion for recovery.
    pub suggestion: String,
}

impl RecoveryHint {
    /// Create a new recovery hint.
    pub fn new(code: ErrorCode, message: impl Into<String>, suggestion: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            suggestion: suggestion.into(),
        }
    }

    /// Format the hint for display.
    pub fn format(&self) -> String {
        format!(
            "[{}] {}\n💡 {}",
            self.code.as_str(),
            self.message,
            self.suggestion
        )
    }
}

/// Result type alias for xtask operations.
pub type Result<T> = std::result::Result<T, XtaskError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_error() {
        let err = XtaskError::new("test error");
        assert!(matches!(err, XtaskError::Generic(msg) if msg == "test error"));
    }

    #[test]
    fn test_internal_error() {
        let err = XtaskError::internal("bug detected");
        assert!(matches!(&err, XtaskError::Internal(msg) if msg == "bug detected"));
        assert!(err.is_internal());
    }

    #[test]
    fn test_validation_builder() {
        let err = XtaskError::validation("invalid input").with_recovery("try again");
        assert!(matches!(err, XtaskError::Validation { context, recovery } 
            if context == "invalid input" && recovery == Some("try again".to_string())));
    }

    #[test]
    fn test_validation_without_recovery() {
        let err = XtaskError::from(XtaskError::validation("invalid"));
        assert!(matches!(err, XtaskError::Validation { context, recovery } 
            if context == "invalid" && recovery.is_none()));
    }

    #[test]
    fn test_recovery_suggestion() {
        let err = XtaskError::validation("test").with_recovery("fix it");
        assert_eq!(err.recovery_suggestion(), Some("fix it"));

        let err = XtaskError::internal("bug");
        assert!(err.recovery_suggestion().is_some());

        let err = XtaskError::new("generic");
        assert!(err.recovery_suggestion().is_none());
    }

    #[test]
    fn test_with_context() {
        let err = XtaskError::new("original").with_context("my-command");
        assert!(matches!(err, XtaskError::CommandFailed { command, reason }
            if command == "my-command" && reason == "original"));
    }

    #[test]
    fn test_from_string() {
        let err: XtaskError = "test".to_string().into();
        assert!(matches!(err, XtaskError::Generic(msg) if msg == "test"));
    }

    #[test]
    fn test_from_str() {
        let err: XtaskError = "test".into();
        assert!(matches!(err, XtaskError::Generic(msg) if msg == "test"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: XtaskError = io_err.into();
        assert!(matches!(err, XtaskError::Io(msg) if msg == "file not found"));
    }

    #[test]
    fn test_error_code_as_str() {
        assert_eq!(ErrorCode::Generic.as_str(), "E000");
        assert_eq!(ErrorCode::ParseError.as_str(), "E100");
        assert_eq!(ErrorCode::DatabaseError.as_str(), "E300");
        assert_eq!(ErrorCode::InternalError.as_str(), "E999");
    }

    #[test]
    fn test_recovery_hint_format() {
        let hint = RecoveryHint::new(
            ErrorCode::ValidationError,
            "Invalid input",
            "Use --help for options",
        );
        let formatted = hint.format();
        assert!(formatted.contains("E600"));
        assert!(formatted.contains("Invalid input"));
        assert!(formatted.contains("Use --help for options"));
    }

    #[test]
    fn test_display() {
        let err = XtaskError::Parse("syntax error".to_string());
        assert_eq!(err.to_string(), "Parse error: syntax error");

        let err = XtaskError::Database("connection failed".to_string());
        assert_eq!(err.to_string(), "Database error: connection failed");

        let err = XtaskError::Internal("bug".to_string());
        assert_eq!(err.to_string(), "Internal error: bug");
    }

    #[test]
    fn test_is_validation() {
        let err: XtaskError = XtaskError::validation("test").into();
        assert!(err.is_validation());

        let err = XtaskError::new("test");
        assert!(!err.is_validation());
    }

    #[test]
    fn test_is_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let err = XtaskError::from(io_err);
        assert!(err.is_io());

        let err = XtaskError::new("test");
        assert!(!err.is_io());
    }

    #[test]
    fn test_source() {
        // Io variant now stores String, so no source available
        let err = XtaskError::new("test");
        assert!(err.source().is_none());
    }

    #[test]
    fn test_result_type_alias() {
        fn returns_ok() -> Result<i32> {
            Ok(42)
        }

        fn returns_err() -> Result<i32> {
            Err(XtaskError::new("error"))
        }

        assert_eq!(returns_ok().unwrap(), 42);
        assert!(returns_err().is_err());
    }
}
