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

pub fn validate_file_extension_allowlist(
    tool: ToolName,
    field: &'static str,
    path: &std::path::Path,
    allowed: &[String],
) -> Result<(), ToolError> {
    if allowed.is_empty() {
        return Err(ToolError::new(
            tool,
            ToolErrorCode::InvalidFormat,
            "no allowed file extensions configured",
        )
        .field(field)
        .retry_hint("Ask the user to configure tooling.create_file_extensions.")
        .retry_context(json!({
            "allowlist_len": 0,
        })));
    }

    let mut normalized_allowed: Vec<String> = Vec::with_capacity(allowed.len());
    for ext in allowed {
        let trimmed = ext.trim();
        if trimmed.is_empty() {
            return Err(ToolError::new(
                tool,
                ToolErrorCode::InvalidFormat,
                "allowed file extension list contains an empty entry",
            )
            .field(field)
            .retry_hint("Remove empty entries from tooling.create_file_extensions.")
            .retry_context(json!({
                "allowlist": allowed,
            })));
        }
        normalized_allowed.push(
            trimmed
                .trim_start_matches('.')
                .to_ascii_lowercase(),
        );
    }

    let allow_all = normalized_allowed
        .iter()
        .any(|ext| ext == "*" || ext == "any");

    let provided_ext = path.extension().and_then(|e| e.to_str());
    let normalized_ext = provided_ext.map(|e| e.trim().to_ascii_lowercase());
    if !allow_all {
        let Some(ext) = normalized_ext.as_deref() else {
            return Err(ToolError::new(
                tool,
                ToolErrorCode::InvalidFormat,
                "file path is missing an extension",
            )
            .field(field)
            .retry_hint("Provide a file path with an extension.")
            .retry_context(json!({
                "path": path.display().to_string(),
                "allowed_extensions": normalized_allowed,
            })));
        };

        if !normalized_allowed.iter().any(|allowed| allowed == ext) {
            return Err(ToolError::new(
                tool,
                ToolErrorCode::InvalidFormat,
                "file extension not allowed",
            )
            .field(field)
            .expected(normalized_allowed.join(", "))
            .received(ext.to_string())
            .retry_hint("Use an allowed file extension.")
            .retry_context(json!({
                "path": path.display().to_string(),
                "extension": ext,
                "allowed_extensions": normalized_allowed,
            })));
        }
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

    #[test]
    fn extension_allowlist_empty_is_invalid() {
        let err = validate_file_extension_allowlist(
            ToolName::CreateFile,
            "file_path",
            std::path::Path::new("src/lib.rs"),
            &[],
        )
        .expect_err("expected empty allowlist error");
        assert_eq!(err.code, ToolErrorCode::InvalidFormat);
        assert_eq!(err.field, Some("file_path"));
    }

    #[test]
    fn extension_allowlist_rejects_empty_entry() {
        let err = validate_file_extension_allowlist(
            ToolName::CreateFile,
            "file_path",
            std::path::Path::new("src/lib.rs"),
            &["".to_string()],
        )
        .expect_err("expected empty entry error");
        assert_eq!(err.code, ToolErrorCode::InvalidFormat);
    }

    #[test]
    fn extension_allowlist_rejects_missing_extension() {
        let err = validate_file_extension_allowlist(
            ToolName::CreateFile,
            "file_path",
            std::path::Path::new("README"),
            &["md".to_string()],
        )
        .expect_err("expected missing extension error");
        assert_eq!(err.code, ToolErrorCode::InvalidFormat);
    }

    #[test]
    fn extension_allowlist_rejects_disallowed_extension() {
        let err = validate_file_extension_allowlist(
            ToolName::CreateFile,
            "file_path",
            std::path::Path::new("README.md"),
            &["rs".to_string()],
        )
        .expect_err("expected disallowed extension error");
        assert_eq!(err.code, ToolErrorCode::InvalidFormat);
        assert_eq!(err.received.as_deref(), Some("md"));
    }

    #[test]
    fn extension_allowlist_allows_matching_extension() {
        validate_file_extension_allowlist(
            ToolName::CreateFile,
            "file_path",
            std::path::Path::new("README.MD"),
            &["md".to_string()],
        )
        .expect("expected allowed extension");
    }

    #[test]
    fn extension_allowlist_allows_special_any() {
        validate_file_extension_allowlist(
            ToolName::CreateFile,
            "file_path",
            std::path::Path::new("README.adoc"),
            &["any".to_string()],
        )
        .expect("expected any extension allowed");
    }
}
