use crate::tools::error::{ToolError, ToolErrorCode};
use ploke_core::tool_types::ToolName;

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
