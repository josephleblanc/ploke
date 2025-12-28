/// NsRead tool wiring for the TUI. Provides a non-semantic read path that enforces crate-root
/// scoping, respects configured byte caps, and applies optional line slicing before emitting a
/// `ToolResult`. Prefer this module whenever the agent needs direct file reads outside the semantic
/// graph (configs, docs, or Rust files that failed to index).
use std::{borrow::Cow, ops::Deref as _, path::PathBuf};

use ploke_core::file_hash::FileHash;
use serde::{Deserialize, Serialize};

use super::{ToolDescr, ToolError, ToolErrorCode, ToolInvocationError, ToolName};
use crate::{tools::ToolResult, tools::tool_ui_error, utils::path_scoping};
use ploke_io::{ReadFileRequest, ReadFileResponse, ReadRange, ReadStrategy};
use tokio::fs;

const FILE_DESC: &str = "Absolute or crate-root-relative file path.";
const START_LINE_DESC: &str = "Optional 1-based line from which to start reading.";
const END_LINE_DESC: &str = "Optional 1-based line at which to stop reading (inclusive).";
const MAX_BYTES_DESC: &str = "Maximum number of UTF-8 bytes to return. Defaults to editor config.";
/// Default byte cap (32 KiB) applied when callers omit `max_bytes`, keeping NsRead outputs concise.
const DEFAULT_READ_BYTE_CAP: usize = 32 * 1024;

lazy_static::lazy_static! {
    static ref NS_READ_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "file": { "type": "string", "description": FILE_DESC },
            "start_line": { "type": "integer", "minimum": 1, "description": START_LINE_DESC },
            "end_line": { "type": "integer", "minimum": 1, "description": END_LINE_DESC },
            "max_bytes": { "type": "integer", "minimum": 1, "description": MAX_BYTES_DESC },
        },
        "required": ["file"],
        "additionalProperties": false
    });
}

/// NsRead is the non-semantic read companion to NsPatch. It exists so the agent can fetch the
/// exact contents of any workspace file (Rust or not) when semantic context lookup is unavailable,
/// incomplete, or too lossy. Typical use cases include inspecting configuration files, verifying the
/// latest state of a file that failed to parse into the code graph, or double-checking text before
/// crafting a non-semantic patch. All reads are scoped through IoManager so path validation and
/// auditing stay consistent with the write pipeline.
/// Tool entry point for non-semantic file reads enforced via IoManager. All behavior is documented
/// in the module doc above so future contributors understand when to prefer NsRead over semantic
/// context retrieval.
pub struct NsRead;

#[derive(Debug, Clone, Deserialize)]
pub struct NsReadParams<'a> {
    #[serde(borrow)]
    pub file: Cow<'a, str>,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub end_line: Option<u32>,
    #[serde(default)]
    pub max_bytes: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NsReadParamsOwned {
    pub file: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub max_bytes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NsReadResult {
    pub ok: bool,
    pub file_path: String,
    pub exists: bool,
    pub byte_len: Option<u64>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub truncated: bool,
    pub content: Option<String>,
    pub file_hash: Option<FileHash>,
}

impl super::Tool for NsRead {
    type Output = NsReadResult;
    type OwnedParams = NsReadParamsOwned;
    type Params<'de> = NsReadParams<'de>;

    fn name() -> ToolName {
        ToolName::NsRead
    }

    fn description() -> ToolDescr {
        ToolDescr::NsRead
    }

    fn schema() -> &'static serde_json::Value {
        NS_READ_PARAMETERS.deref()
    }

    fn adapt_error(err: ToolInvocationError) -> ToolError {
        let hint = "Use an absolute path or crate-root-relative file path (e.g., \"src/lib.rs\"). \
Directories are not valid for read_file.";
        match err {
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Io { message },
            )) => ToolError::new(ToolName::NsRead, ToolErrorCode::Io, message).retry_hint(hint),
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Ui { message },
            )) => ToolError::new(ToolName::NsRead, ToolErrorCode::InvalidFormat, message)
                .retry_hint(hint),
            other => other.into_tool_error(ToolName::NsRead),
        }
    }

    fn build(_ctx: &super::Ctx) -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        NsReadParamsOwned {
            file: params.file.clone().into_owned(),
            start_line: params.start_line,
            end_line: params.end_line,
            max_bytes: params.max_bytes,
        }
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        use ploke_error::{DomainError, InternalError};

        let NsReadParams {
            file,
            start_line,
            end_line,
            max_bytes,
        } = params;

        if let (Some(start), Some(end)) = (start_line, end_line)
            && end < start
        {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: "end_line must be greater than or equal to start_line".to_string(),
            }));
        }

        let crate_root = ctx
            .state
            .system
            .read()
            .await
            .focused_crate_root()
            .ok_or_else(|| {
                ploke_error::Error::Domain(DomainError::Ui {
                    message:
                        "No crate is currently focused; load a workspace before using read_file."
                            .to_string(),
                })
            })?;

        let requested_path = PathBuf::from(file.as_ref());
        let abs_path =
            path_scoping::resolve_in_crate_root(&requested_path, &crate_root).map_err(|err| {
                ploke_error::Error::Domain(DomainError::Io {
                    message: format!(
                        "invalid path: {err}. Paths must be absolute or crate-root-relative."
                    ),
                })
            })?;

        if let Ok(meta) = fs::metadata(&abs_path).await {
            if meta.is_dir() {
                return Err(tool_ui_error(
                    "read_file expects a file path, not a directory. \
Paths must be absolute or crate-root-relative (e.g., \"src/lib.rs\").",
                ));
            }
        }

        let byte_cap = max_bytes
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_READ_BYTE_CAP);

        let request = ReadFileRequest {
            file_path: abs_path,
            range: if start_line.is_some() || end_line.is_some() {
                Some(ReadRange {
                    start_line,
                    end_line,
                })
            } else {
                None
            },
            max_bytes: Some(byte_cap),
            strategy: ReadStrategy::Plain,
        };

        let read_resp = ctx
            .state
            .io_handle
            .read_file(request)
            .await
            .map_err(|err| {
                ploke_error::Error::Internal(InternalError::CompilerError(format!(
                    "io channel error: {err}"
                )))
            })??;

        let ReadFileResponse {
            exists,
            file_path,
            byte_len,
            content,
            truncated: io_truncated,
            file_hash,
        } = read_resp;

        let (content, slice_truncated) = match content {
            Some(src) => {
                let (sliced, truncated) = slice_content_lines(src, start_line, end_line);
                (Some(sliced), truncated)
            }
            None => (None, false),
        };

        let result = NsReadResult {
            ok: true,
            file_path: file_path.display().to_string(),
            exists,
            byte_len,
            start_line,
            end_line,
            truncated: io_truncated || slice_truncated,
            content,
            file_hash,
        };
        let summary = if result.exists {
            format!("Read {}", result.file_path)
        } else {
            format!("Missing file {}", result.file_path)
        };
        let ui_payload = super::ToolUiPayload::new(Self::name(), ctx.call_id.clone(), summary)
            .with_field("exists", result.exists.to_string())
            .with_field("truncated", (io_truncated || slice_truncated).to_string())
            .with_field(
                "lines",
                match (result.start_line, result.end_line) {
                    (Some(start), Some(end)) => format!("{start}-{end}"),
                    (Some(start), None) => format!("{start}-"),
                    (None, Some(end)) => format!("1-{end}"),
                    (None, None) => "full".to_string(),
                },
            );

        let content = serde_json::to_string(&result).map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to serialize NsReadResult: {err}"
            )))
        })?;

        Ok(ToolResult {
            content,
            ui_payload: Some(ui_payload),
        })
    }
}

/// Convert optional 1-based start/end lines into a UTF-8 safe slice, returning whether the slice
/// was truncated relative to the original IoManager response.
fn slice_content_lines(
    content: String,
    start_line: Option<u32>,
    end_line: Option<u32>,
) -> (String, bool) {
    if start_line.is_none() && end_line.is_none() {
        return (content, false);
    }

    let total_len = content.len();
    let (start_byte, end_byte, truncated) = line_byte_window(&content, start_line, end_line);

    if start_byte >= end_byte || start_byte >= total_len {
        return (String::new(), truncated);
    }

    let end_clamped = end_byte.min(total_len);
    (content[start_byte..end_clamped].to_string(), truncated)
}

/// Compute byte offsets for the requested line window, noting truncation when the requested lines
/// exceed available content.
fn line_byte_window(
    content: &str,
    start_line: Option<u32>,
    end_line: Option<u32>,
) -> (usize, usize, bool) {
    let total_len = content.len();
    let mut truncated = false;

    let start_line_num = start_line.unwrap_or(1).max(1);
    let (start_raw, start_missing) = byte_offset_for_line(content, start_line_num);
    let start_byte = start_raw.min(total_len);
    if let Some(line) = start_line {
        if line > 1 && start_byte > 0 {
            truncated = true;
        }
        if start_missing {
            truncated = true;
        }
    }

    let (end_raw, end_missing) = match end_line {
        Some(end_line_num) => {
            let (idx, missing) = byte_offset_for_line(content, end_line_num.saturating_add(1));
            (idx, missing)
        }
        None => (total_len, false),
    };
    let end_byte_raw = end_raw;
    let end_byte = end_byte_raw.min(total_len).max(start_byte);

    if end_line.is_some() {
        if end_byte_raw < total_len {
            truncated = true;
        }
        if end_missing {
            truncated = true;
        }
    }

    (start_byte, end_byte, truncated)
}

/// Map a 1-based line number to a byte offset, returning whether the requested line was missing so
/// callers can detect truncation conditions.
fn byte_offset_for_line(content: &str, target_line: u32) -> (usize, bool) {
    if target_line <= 1 {
        return (0, false);
    }

    let mut current_line = 1;
    for (idx, ch) in content.char_indices() {
        if ch == '\n' {
            current_line += 1;
            if current_line == target_line {
                return (idx + 1, false);
            }
        }
    }
    (content.len(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use serde_json::json;
    use std::borrow::Cow;

    #[test]
    fn schema_matches_expected() {
        let schema = NsRead::schema().clone();
        let expected = json!({
            "type": "object",
            "properties": {
                "file": { "type": "string", "description": FILE_DESC },
                "start_line": { "type": "integer", "minimum": 1, "description": START_LINE_DESC },
                "end_line": { "type": "integer", "minimum": 1, "description": END_LINE_DESC },
                "max_bytes": { "type": "integer", "minimum": 1, "description": MAX_BYTES_DESC },
            },
            "required": ["file"],
            "additionalProperties": false
        });
        assert_eq!(schema, expected);
    }

    #[test]
    fn into_owned_transfers_fields() {
        let params = NsReadParams {
            file: Cow::Borrowed("src/lib.rs"),
            start_line: Some(10),
            end_line: Some(20),
            max_bytes: Some(1024),
        };
        let owned = NsRead::into_owned(&params);
        assert_eq!(owned.file, "src/lib.rs");
        assert_eq!(owned.start_line, Some(10));
        assert_eq!(owned.end_line, Some(20));
        assert_eq!(owned.max_bytes, Some(1024));
    }

    #[test]
    fn slice_content_lines_returns_full_when_no_range() {
        let input = "line1\nline2\nline3\n".to_string();
        let (out, truncated) = slice_content_lines(input.clone(), None, None);
        assert_eq!(out, input);
        assert!(!truncated);
    }

    #[test]
    fn slice_content_lines_honors_start_and_end() {
        let input = "a\nb\nc\nd\n".to_string();
        let (out, truncated) = slice_content_lines(input, Some(2), Some(3));
        assert_eq!(out, "b\nc\n");
        assert!(truncated);
    }

    #[test]
    fn slice_content_lines_handles_start_beyond_len() {
        let input = "only one\n".to_string();
        let (out, truncated) = slice_content_lines(input, Some(5), None);
        assert!(out.is_empty());
        assert!(truncated);
    }

    #[test]
    fn byte_offset_for_line_first_line() {
        let content = "first\nsecond";
        assert_eq!(byte_offset_for_line(content, 1), (0, false));
    }
}
