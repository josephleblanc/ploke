//! Tests for error handling and recovery hints.
//!
//! These tests verify the `XtaskError` type, error conversions,
//! recovery hints, and the validation error builder pattern.

use std::error::Error;
use std::io;

use xtask::error::{ErrorCode, RecoveryHint, Result, XtaskError};

// =============================================================================
// XtaskError Creation Tests
// =============================================================================

/// To Prove: XtaskError::new() creates a generic error
/// Given: An error message string
/// When: XtaskError::new() is called
/// Then: Returns XtaskError::Generic with the message
#[test]
fn error_new_creates_generic_error() {
    let err = XtaskError::new("something went wrong");

    match &err {
        XtaskError::Generic(msg) => assert_eq!(msg, "something went wrong"),
        _ => panic!("Expected Generic variant"),
    }
}

/// To Prove: XtaskError::internal() creates an internal error
/// Given: An error message string
/// When: XtaskError::internal() is called
/// Then: Returns XtaskError::Internal with the message
#[test]
fn error_internal_creates_internal_error() {
    let err = XtaskError::internal("unexpected condition");

    assert!(err.is_internal());
    match &err {
        XtaskError::Internal(msg) => assert_eq!(msg, "unexpected condition"),
        _ => panic!("Expected Internal variant"),
    }
}

/// To Prove: XtaskError::validation() creates a validation error builder
/// Given: A validation context string
/// When: XtaskError::validation() is called
/// Then: Returns ValidationBuilder that can add recovery hint
#[test]
fn error_validation_creates_validation_builder() {
    // Without recovery hint
    let err: XtaskError = XtaskError::validation("invalid input").into();

    assert!(err.is_validation());
    match &err {
        XtaskError::Validation { context, recovery } => {
            assert_eq!(context, "invalid input");
            assert!(recovery.is_none());
        }
        _ => panic!("Expected Validation variant"),
    }
}

/// To Prove: ValidationBuilder::with_recovery() adds recovery hint
/// Given: A ValidationBuilder
/// When: with_recovery() is called
/// Then: Returns Validation error with recovery hint
#[test]
fn error_validation_with_recovery() {
    let err = XtaskError::validation("missing argument")
        .with_recovery("Use --help to see available options");

    match &err {
        XtaskError::Validation { context, recovery } => {
            assert_eq!(context, "missing argument");
            assert_eq!(
                recovery.as_deref(),
                Some("Use --help to see available options")
            );
        }
        _ => panic!("Expected Validation variant"),
    }
}

// =============================================================================
// Error Conversion Tests
// =============================================================================

/// To Prove: String converts to XtaskError::Generic
/// Given: A String
/// When: Into<XtaskError>::into() is called
/// Then: Returns XtaskError::Generic
#[test]
fn error_from_string() {
    let err: XtaskError = "error message".to_string().into();

    match &err {
        XtaskError::Generic(msg) => assert_eq!(msg, "error message"),
        _ => panic!("Expected Generic variant"),
    }
}

/// To Prove: &str converts to XtaskError::Generic
/// Given: A string slice
/// When: Into<XtaskError>::into() is called
/// Then: Returns XtaskError::Generic
#[test]
fn error_from_str() {
    let err: XtaskError = "error message".into();

    match &err {
        XtaskError::Generic(msg) => assert_eq!(msg, "error message"),
        _ => panic!("Expected Generic variant"),
    }
}

/// To Prove: io::Error converts to XtaskError::Io
/// Given: An io::Error
/// When: Into<XtaskError>::into() is called
/// Then: Returns XtaskError::Io with the error message
#[test]
fn error_from_io_error() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let err: XtaskError = io_err.into();

    assert!(err.is_io());
    match &err {
        XtaskError::Io(msg) => assert!(msg.contains("file not found")),
        _ => panic!("Expected Io variant"),
    }
}

/// To Prove: serde_json::Error converts to XtaskError::Serialization
/// Given: An invalid JSON string
/// When: serde_json::from_str() fails and error is converted
/// Then: Returns XtaskError::Serialization
#[test]
fn error_from_json_error() {
    let json_result: std::result::Result<serde_json::Value, serde_json::Error> =
        serde_json::from_str("not valid json");
    let json_err = json_result.unwrap_err();
    let err: XtaskError = json_err.into();

    match &err {
        XtaskError::Serialization(_) => {
            // Expected
        }
        _ => panic!("Expected Serialization variant"),
    }
}

// =============================================================================
// Error Context Tests
// =============================================================================

/// To Prove: XtaskError::with_context() wraps error with command context
/// Given: An existing XtaskError
/// When: with_context() is called with a command name
/// Then: Returns XtaskError::CommandFailed with context
#[test]
fn error_with_context_wraps_error() {
    let err = XtaskError::new("original error").with_context("my-command");

    match &err {
        XtaskError::CommandFailed { command, reason } => {
            assert_eq!(command, "my-command");
            assert!(reason.contains("original error"));
        }
        _ => panic!("Expected CommandFailed variant"),
    }
}

/// To Prove: XtaskError::with_context() can be chained
/// Given: A chain of context additions
/// When: with_context() is called multiple times
/// Then: Each call wraps the previous error
#[test]
fn error_with_context_chain() {
    let err = XtaskError::new("root cause")
        .with_context("inner-command")
        .with_context("outer-command");

    match &err {
        XtaskError::CommandFailed { command, reason } => {
            assert_eq!(command, "outer-command");
            assert!(reason.contains("inner-command"));
        }
        _ => panic!("Expected CommandFailed variant"),
    }
}

// =============================================================================
// Recovery Hint Tests
// =============================================================================

/// To Prove: XtaskError::recovery_suggestion() returns hints for known errors
/// Given: Different error variants
/// When: recovery_suggestion() is called
/// Then: Returns appropriate hint or None
#[test]
fn error_recovery_suggestion() {
    // Validation with explicit recovery hint
    let err = XtaskError::validation("test").with_recovery("do this");
    assert_eq!(err.recovery_suggestion(), Some("do this"));

    // CommandFailed has default suggestion
    let err = XtaskError::new("fail").with_context("cmd");
    assert!(err.recovery_suggestion().is_some());
    assert!(err.recovery_suggestion().unwrap().contains("Check the command"));

    // Resource errors have suggestions
    let err = XtaskError::Resource("missing".to_string());
    assert!(err.recovery_suggestion().is_some());

    // Database errors have suggestions
    let err = XtaskError::Database("connection failed".to_string());
    assert!(err.recovery_suggestion().is_some());

    // IO errors have suggestions
    let err = XtaskError::Io("permission denied".to_string());
    assert!(err.recovery_suggestion().is_some());

    // Internal errors have suggestions
    let err = XtaskError::Internal("bug".to_string());
    assert!(err.recovery_suggestion().is_some());

    // Parse: diagnostic text is included in Display only (avoids doubling in print_report).
    let err = XtaskError::Parse("failed".to_string());
    assert!(err.recovery_suggestion().is_none());

    // Generic errors have no suggestion
    let err = XtaskError::new("generic");
    assert!(err.recovery_suggestion().is_none());
}

/// To Prove: RecoveryHint::new() creates structured recovery hints
/// Given: Error code, message, and suggestion
/// When: RecoveryHint::new() is called
/// Then: Returns structured recovery hint
#[test]
fn recovery_hint_creation() {
    let hint = RecoveryHint::new(
        ErrorCode::ValidationError,
        "Invalid input provided",
        "Check the documentation for valid options",
    );

    assert_eq!(hint.code, ErrorCode::ValidationError);
    assert_eq!(hint.message, "Invalid input provided");
    assert_eq!(hint.suggestion, "Check the documentation for valid options");
}

/// To Prove: RecoveryHint::format() produces displayable output
/// Given: A RecoveryHint
/// When: format() is called
/// Then: Returns formatted string with all fields
#[test]
fn recovery_hint_format() {
    let hint = RecoveryHint::new(
        ErrorCode::DatabaseError,
        "Connection failed",
        "Check your database configuration",
    );

    let formatted = hint.format();

    assert!(formatted.contains("E300")); // Database error code
    assert!(formatted.contains("Connection failed"));
    assert!(formatted.contains("Check your database configuration"));
    assert!(formatted.contains("💡"));
}

// =============================================================================
// Error Type Check Tests
// =============================================================================

/// To Prove: XtaskError::is_validation() correctly identifies validation errors
/// Given: Different error variants
/// When: is_validation() is called
/// Then: Returns true only for Validation variant
#[test]
fn error_is_validation() {
    let err: XtaskError = XtaskError::validation("test").into();
    assert!(err.is_validation());
    assert!(!XtaskError::new("test").is_validation());
    assert!(!XtaskError::Io("test".to_string()).is_validation());
    assert!(!XtaskError::Internal("test".to_string()).is_validation());
}

/// To Prove: XtaskError::is_io() correctly identifies IO errors
/// Given: Different error variants
/// When: is_io() is called
/// Then: Returns true only for Io variant
#[test]
fn error_is_io() {
    let io_err = io::Error::new(io::ErrorKind::Other, "test");
    assert!(XtaskError::from(io_err).is_io());
    assert!(!XtaskError::new("test").is_io());
    let v_err: XtaskError = XtaskError::validation("test").into();
    assert!(!v_err.is_io());
}

/// To Prove: XtaskError::is_internal() correctly identifies internal errors
/// Given: Different error variants
/// When: is_internal() is called
/// Then: Returns true only for Internal variant
#[test]
fn error_is_internal() {
    assert!(XtaskError::internal("test").is_internal());
    assert!(!XtaskError::new("test").is_internal());
    assert!(!XtaskError::Io("test".to_string()).is_internal());
}

// =============================================================================
// Error Display Tests
// =============================================================================

/// To Prove: XtaskError::Display formats all variants correctly
/// Given: Different error variants
/// When: to_string() is called
/// Then: Returns formatted string with error type and message
#[test]
fn error_display_formats() {
    assert_eq!(
        XtaskError::Generic("test".to_string()).to_string(),
        "test"
    );
    let parse_err = XtaskError::Parse("syntax error".to_string());
    let parse_s = parse_err.to_string();
    assert!(
        parse_s.starts_with("Parse error: syntax error"),
        "got: {parse_s}"
    );
    assert!(
        parse_s.contains("parse debug"),
        "expected diagnostic hint in Display: {parse_s}"
    );
    assert_eq!(
        XtaskError::Transform("transform failed".to_string()).to_string(),
        "Transform error: transform failed"
    );
    assert_eq!(
        XtaskError::Database("connection lost".to_string()).to_string(),
        "Database error: connection lost"
    );
    assert_eq!(
        XtaskError::Embedding("model error".to_string()).to_string(),
        "Embedding error: model error"
    );
    assert_eq!(
        XtaskError::Resource("not found".to_string()).to_string(),
        "Resource error: not found"
    );
    let validation_err: XtaskError = XtaskError::validation("invalid").into();
    assert_eq!(validation_err.to_string(), "Validation error: invalid");
    assert_eq!(
        XtaskError::CommandFailed {
            command: "cmd".to_string(),
            reason: "failed".to_string(),
        }
        .to_string(),
        "Command 'cmd' failed: failed"
    );
    assert_eq!(
        XtaskError::Io("disk full".to_string()).to_string(),
        "IO error: disk full"
    );
    assert_eq!(
        XtaskError::Serialization("bad json".to_string()).to_string(),
        "Serialization error: bad json"
    );
    assert_eq!(
        XtaskError::Internal("bug".to_string()).to_string(),
        "Internal error: bug"
    );
}

// =============================================================================
// ErrorCode Tests
// =============================================================================

/// To Prove: ErrorCode::as_str() returns correct code strings
/// Given: Each ErrorCode variant
/// When: as_str() is called
/// Then: Returns formatted error code (e.g., "E000", "E100")
#[test]
fn error_code_as_str() {
    assert_eq!(ErrorCode::Generic.as_str(), "E000");
    assert_eq!(ErrorCode::ParseError.as_str(), "E100");
    assert_eq!(ErrorCode::TransformError.as_str(), "E200");
    assert_eq!(ErrorCode::DatabaseError.as_str(), "E300");
    assert_eq!(ErrorCode::EmbeddingError.as_str(), "E400");
    assert_eq!(ErrorCode::ResourceNotFound.as_str(), "E500");
    assert_eq!(ErrorCode::ValidationError.as_str(), "E600");
    assert_eq!(ErrorCode::CommandNotFound.as_str(), "E700");
    assert_eq!(ErrorCode::InvalidArguments.as_str(), "E800");
    assert_eq!(ErrorCode::Timeout.as_str(), "E900");
    assert_eq!(ErrorCode::InternalError.as_str(), "E999");
}

/// To Prove: Error codes follow consistent pattern
/// Given: All error codes
/// When: Checked for format
/// Then: All follow "E###" pattern where # is digit
#[test]
fn error_code_format_pattern() {
    let codes = vec![
        ErrorCode::Generic,
        ErrorCode::ParseError,
        ErrorCode::TransformError,
        ErrorCode::DatabaseError,
        ErrorCode::EmbeddingError,
        ErrorCode::ResourceNotFound,
        ErrorCode::ValidationError,
        ErrorCode::CommandNotFound,
        ErrorCode::InvalidArguments,
        ErrorCode::Timeout,
        ErrorCode::InternalError,
    ];

    for code in codes {
        let s = code.as_str();
        assert_eq!(s.len(), 4);
        assert!(s.starts_with('E'));
        assert!(s[1..].chars().all(|c| c.is_ascii_digit()));
    }
}

// =============================================================================
// Error Trait Tests
// =============================================================================

/// To Prove: XtaskError implements Error trait
/// Given: An XtaskError
/// When: Error trait methods are called
/// Then: Behavior is as expected for the trait
#[test]
fn error_trait_implementation() {
    let err: Box<dyn Error> = Box::new(XtaskError::new("test"));

    assert_eq!(err.to_string(), "test");
    // source() returns None for XtaskError (Io stores String)
    assert!(err.source().is_none());
}

// =============================================================================
// Result Type Tests
// =============================================================================

/// To Prove: Result type alias works correctly for Ok values
/// Given: A function returning Result<T>
/// When: Function returns Ok(value)
/// Then: Result contains the value
#[test]
fn result_type_ok() {
    fn returns_ok() -> Result<i32> {
        Ok(42)
    }

    assert_eq!(returns_ok().unwrap(), 42);
}

/// To Prove: Result type alias works correctly for Err values
/// Given: A function returning Result<T>
/// When: Function returns Err(XtaskError)
/// Then: Result contains the error
#[test]
fn result_type_err() {
    fn returns_err() -> Result<i32> {
        Err(XtaskError::new("failed"))
    }

    assert!(returns_err().is_err());
    assert_eq!(returns_err().unwrap_err().to_string(), "failed");
}

/// To Prove: Result supports standard error handling patterns
/// Given: A Result value
/// When: Using ? operator or match
/// Then: Works with standard Rust error handling
#[test]
fn result_error_handling_patterns() {
    fn might_fail(succeed: bool) -> Result<String> {
        if succeed {
            Ok("success".to_string())
        } else {
            Err(XtaskError::new("failure"))
        }
    }

    fn chain_operations() -> Result<String> {
        let val = might_fail(true)?;
        Ok(format!("got: {}", val))
    }

    assert_eq!(chain_operations().unwrap(), "got: success");

    fn chain_fail() -> Result<String> {
        let val = might_fail(false)?;
        Ok(val)
    }

    assert!(chain_fail().is_err());
}

// =============================================================================
// ploke_error integration (PRIMARY_TASK_SPEC §D)
// =============================================================================

/// To Prove: `ploke_error::Error` converts to `XtaskError` for workspace command boundaries.
#[test]
fn error_from_ploke_error_maps_for_database_domain() {
    use ploke_error::DomainError;

    let pe = ploke_error::Error::from(DomainError::Db {
        message: "fixture probe failed".into(),
    });
    let xe: XtaskError = pe.into();
    let s = xe.to_string();
    assert!(
        s.contains("fixture probe failed"),
        "display should carry ploke_error content: {s}"
    );
    assert!(
        xe.recovery_suggestion().is_some(),
        "mapped errors should still offer recovery hints where applicable: {xe:?}"
    );
}

// =============================================================================
// Error Report Tests (print_report is tested manually)
// =============================================================================

/// To Prove: Error report formatting includes all relevant information
/// Given: An error with context and recovery hint
/// When: Error is formatted for display
/// Then: Contains error message, context, and recovery hint
#[test]
fn error_report_contains_all_info() {
    let err = XtaskError::validation("missing required argument")
        .with_recovery("Provide the argument or use --help");

    let err_with_context = err.with_context("parse file");

    let display = err_with_context.to_string();

    // Should contain the command name
    assert!(display.contains("parse file"));
    // Should contain the validation context
    assert!(display.contains("missing required argument"));
}
