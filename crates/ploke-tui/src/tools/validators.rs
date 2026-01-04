use crate::tools::error::{ToolError, ToolErrorCode};
use ploke_core::tool_types::ToolName;
use serde_json::json;

/// Validate that a numeric context/token limit does not exceed `max`.
pub fn validate_context_limit(
    tool: ToolName,
    field: &'static str,
    value: u64,
    max: u64,
) -> Result<u64, ToolError> {
    if value > max {
        Err(ToolError::new(
            tool,
            ToolErrorCode::FieldTooLarge,
            format!("{field} must be <= {max}"),
        )
        .field(field)
        .expected(format!("<= {max}"))
        .received(value.to_string()))
    } else {
        Ok(value)
    }
}

/// Basic unified-diff shape check to avoid opaque failures downstream.
pub fn validate_unified_diff(
    tool: ToolName,
    field: &'static str,
    diff: &str,
) -> Result<(), ToolError> {
    let has_headers = diff.contains("---") && diff.contains("+++");
    let has_hunk = diff.contains("@@");
    if has_headers && has_hunk {
        Ok(())
    } else {
        Err(ToolError::new(
            tool,
            ToolErrorCode::MalformedDiff,
            "expected unified diff with ---/+++ headers and @@ hunks",
        )
        .field(field)
        .expected("unified diff with ---/+++ and @@")
        .snippet(super::error::truncate_for_error(diff, 512)))
    }
}

/// Validate a file path string for basic safety and portability concerns.
pub fn validate_file_path_basic(
    tool: ToolName,
    field: &'static str,
    path: &str,
    allow_unicode: bool,
) -> Result<(), ToolError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(ToolError::new(
            tool,
            ToolErrorCode::InvalidFormat,
            "file path is empty",
        )
        .field(field)
        .retry_hint("Provide a non-empty file path (e.g., \"src/lib.rs\").")
        .retry_context(json!({
            "input_path": path,
            "reason": "empty",
        })));
    }

    if !allow_unicode && !path.is_ascii() {
        return Err(ToolError::new(
            tool,
            ToolErrorCode::InvalidFormat,
            "file path contains non-ASCII characters",
        )
        .field(field)
        .retry_hint("Use ASCII characters in file paths for portability.")
        .retry_context(json!({
            "input_path": path,
            "reason": "non_ascii",
        })));
    }

    if path.chars().any(|c| c == '\0' || c.is_control()) {
        return Err(ToolError::new(
            tool,
            ToolErrorCode::InvalidFormat,
            "file path contains control characters",
        )
        .field(field)
        .retry_hint("Remove control characters from the file path.")
        .retry_context(json!({
            "input_path": path,
            "reason": "control_characters",
        })));
    }

    let invalid_chars: Vec<char> = path
        .chars()
        .filter(|c| matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
        .collect();
    if !invalid_chars.is_empty() {
        return Err(ToolError::new(
            tool,
            ToolErrorCode::InvalidFormat,
            "file path contains invalid characters",
        )
        .field(field)
        .retry_hint("Remove invalid characters like <, >, \", |, ?, *.")
        .retry_context(json!({
            "input_path": path,
            "reason": "invalid_characters",
            "invalid_chars": invalid_chars,
        })));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_path_empty_is_invalid() {
        let err = validate_file_path_basic(ToolName::CreateFile, "file_path", "   ", false)
            .expect_err("expected empty path error");
        assert_eq!(err.code, ToolErrorCode::InvalidFormat);
        assert_eq!(err.field, Some("file_path"));
        assert!(err.retry_hint.is_some());
        let ctx = err.retry_context.expect("retry context");
        assert_eq!(ctx.get("reason").and_then(|v| v.as_str()), Some("empty"));
    }

    #[test]
    fn file_path_with_invalid_chars_is_invalid() {
        let err = validate_file_path_basic(ToolName::CreateFile, "file_path", "src/<bad>.rs", false)
            .expect_err("expected invalid char error");
        let ctx = err.retry_context.expect("retry context");
        let invalid = ctx.get("invalid_chars").and_then(|v| v.as_array());
        assert!(invalid.is_some());
    }

    #[test]
    fn file_path_with_unicode_is_invalid_when_disallowed() {
        let err = validate_file_path_basic(
            ToolName::CreateFile,
            "file_path",
            "src/ma\u{00df}.rs",
            false,
        )
            .expect_err("expected unicode error");
        let ctx = err.retry_context.expect("retry context");
        assert_eq!(ctx.get("reason").and_then(|v| v.as_str()), Some("non_ascii"));
    }

    #[test]
    fn file_path_valid_passes() {
        validate_file_path_basic(ToolName::CreateFile, "file_path", "src/lib.rs", false)
            .expect("valid path");
    }
}
