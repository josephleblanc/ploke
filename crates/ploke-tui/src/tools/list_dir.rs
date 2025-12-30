//! ListDir tool: safe, structured directory listing without shell access.
use std::{borrow::Cow, ops::Deref as _, path::PathBuf, time::UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::fs;

use super::{ToolDescr, ToolError, ToolErrorCode, ToolInvocationError, ToolName};
use crate::{tools::ToolResult, tools::tool_io_error, tools::tool_ui_error, utils::path_scoping};

const DIR_DESC: &str = "Absolute or crate-root-relative directory path.";
const INCLUDE_HIDDEN_DESC: &str = "Include hidden entries starting with '.' (default: false).";
const SORT_DESC: &str =
    "Sort order: name (asc), mtime (newest first), size (largest first), none (filesystem order). \
Default: name.";
const MAX_ENTRIES_DESC: &str = "Maximum number of entries to return (default: no limit).";

lazy_static::lazy_static! {
    static ref LIST_DIR_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "dir": { "type": "string", "description": DIR_DESC },
            "include_hidden": { "type": "boolean", "description": INCLUDE_HIDDEN_DESC },
            "sort": { "type": "string", "enum": ["name", "mtime", "size", "none"], "description": SORT_DESC },
            "max_entries": { "type": "integer", "minimum": 1, "description": MAX_ENTRIES_DESC }
        },
        "required": ["dir"],
        "additionalProperties": false
    });
}

pub struct ListDir;

#[derive(Debug, Clone, Deserialize)]
pub struct ListDirParams<'a> {
    #[serde(borrow)]
    pub dir: Cow<'a, str>,
    #[serde(default)]
    pub include_hidden: Option<bool>,
    #[serde(default)]
    pub sort: Option<Cow<'a, str>>,
    #[serde(default)]
    pub max_entries: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListDirParamsOwned {
    pub dir: String,
    pub include_hidden: bool,
    pub sort: Option<String>,
    pub max_entries: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDirEntry {
    pub name: String,
    pub path: String,
    pub kind: String, // "file" | "dir" | "symlink" | "other"
    pub size_bytes: Option<u64>,
    pub modified_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDirResult {
    pub ok: bool,
    pub dir: String,
    pub exists: bool,
    pub truncated: bool,
    pub entries: Vec<ListDirEntry>,
}

#[derive(Debug, Clone, Copy)]
enum SortMode {
    Name,
    Mtime,
    Size,
    None,
}

impl SortMode {
    fn parse(input: Option<&str>) -> Result<Self, ploke_error::Error> {
        match input.unwrap_or("name") {
            "name" => Ok(Self::Name),
            "mtime" => Ok(Self::Mtime),
            "size" => Ok(Self::Size),
            "none" => Ok(Self::None),
            other => Err(tool_ui_error(format!(
                "invalid sort: {other}. expected one of: name, mtime, size, none"
            ))),
        }
    }
}

impl super::Tool for ListDir {
    type Output = ListDirResult;
    type OwnedParams = ListDirParamsOwned;
    type Params<'de> = ListDirParams<'de>;

    fn name() -> ToolName {
        ToolName::ListDir
    }

    fn description() -> ToolDescr {
        ToolDescr::ListDir
    }

    fn schema() -> &'static serde_json::Value {
        LIST_DIR_PARAMETERS.deref()
    }

    fn adapt_error(err: ToolInvocationError) -> ToolError {
        let hint = "Provide a directory path that is absolute or crate-root-relative \
(e.g., \"src\" or \"/abs/path/to/crate/src\").";
        match err {
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Ui { message },
            )) => ToolError::new(ToolName::ListDir, ToolErrorCode::InvalidFormat, message)
                .retry_hint(hint),
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Io { message },
            )) => ToolError::new(ToolName::ListDir, ToolErrorCode::Io, message).retry_hint(hint),
            other => other.into_tool_error(ToolName::ListDir),
        }
    }

    fn build(_ctx: &super::Ctx) -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        ListDirParamsOwned {
            dir: params.dir.clone().into_owned(),
            include_hidden: params.include_hidden.unwrap_or(false),
            sort: params.sort.as_ref().map(|s| s.clone().into_owned()),
            max_entries: params.max_entries,
        }
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        use ploke_error::{DomainError, InternalError};

        let include_hidden = params.include_hidden.unwrap_or(false);
        let sort_mode = SortMode::parse(params.sort.as_deref())?;
        let max_entries = match params.max_entries {
            Some(0) => {
                return Err(tool_ui_error("max_entries must be >= 1"));
            }
            other => other,
        };

        let crate_root = ctx
            .state
            .system
            .read()
            .await
            .focused_crate_root()
            .ok_or_else(|| {
                ploke_error::Error::Domain(DomainError::Ui {
                    message:
                        "No crate is currently focused; load a workspace before using list_dir."
                            .to_string(),
                })
            })?;

        let requested_path = PathBuf::from(params.dir.as_ref());
        let abs_path =
            path_scoping::resolve_in_crate_root(&requested_path, &crate_root).map_err(|err| {
                ploke_error::Error::Domain(DomainError::Io {
                    message: format!(
                        "invalid path: {err}. Paths must be absolute or crate-root-relative."
                    ),
                })
            })?;

        let dir_display = abs_path
            .strip_prefix(&crate_root)
            .unwrap_or(abs_path.as_path())
            .display()
            .to_string();

        let meta = match fs::metadata(&abs_path).await {
            Ok(meta) => meta,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let result = ListDirResult {
                    ok: true,
                    dir: dir_display,
                    exists: false,
                    truncated: false,
                    entries: Vec::new(),
                };
                let content = serde_json::to_string(&result).map_err(|e| {
                    ploke_error::Error::Internal(InternalError::CompilerError(format!(
                        "failed to serialize ListDirResult: {e}"
                    )))
                })?;
                let summary = format!("Missing directory {}", result.dir);
                let ui_payload =
                    super::ToolUiPayload::new(Self::name(), ctx.call_id.clone(), summary)
                        .with_field("exists", "false")
                        .with_field("entries", "0");
                return Ok(ToolResult {
                    content,
                    ui_payload: Some(ui_payload),
                });
            }
            Err(err) => {
                return Err(tool_io_error(format!("failed to stat directory: {err}")));
            }
        };

        if !meta.is_dir() {
            return Err(tool_ui_error(
                "list_dir expects a directory path, not a file.",
            ));
        }

        let mut entries = Vec::new();
        let mut read_dir = fs::read_dir(&abs_path)
            .await
            .map_err(|err| tool_io_error(format!("failed to read dir: {err}")))?;
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|err| tool_io_error(format!("failed to read dir entry: {err}")))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if !include_hidden && name.starts_with('.') {
                continue;
            }

            let path = entry.path();
            let meta = fs::symlink_metadata(&path)
                .await
                .map_err(|err| tool_io_error(format!("failed to stat entry {name}: {err}")))?;
            let file_type = meta.file_type();
            let kind = if file_type.is_dir() {
                "dir"
            } else if file_type.is_file() {
                "file"
            } else if file_type.is_symlink() {
                "symlink"
            } else {
                "other"
            };
            let size_bytes = if file_type.is_file() {
                Some(meta.len())
            } else {
                None
            };
            let modified_ms = meta.modified().ok().and_then(|t| {
                t.duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_millis() as i64)
            });

            let rel_path = path
                .strip_prefix(&crate_root)
                .unwrap_or(path.as_path())
                .display()
                .to_string();

            entries.push(ListDirEntry {
                name,
                path: rel_path,
                kind: kind.to_string(),
                size_bytes,
                modified_ms,
            });
        }

        match sort_mode {
            SortMode::Name => entries.sort_by(|a, b| a.name.cmp(&b.name)),
            SortMode::Mtime => entries.sort_by(|a, b| {
                let a_key = a.modified_ms.unwrap_or(i64::MIN);
                let b_key = b.modified_ms.unwrap_or(i64::MIN);
                b_key.cmp(&a_key).then_with(|| a.name.cmp(&b.name))
            }),
            SortMode::Size => entries.sort_by(|a, b| {
                let a_key = a.size_bytes.unwrap_or(0);
                let b_key = b.size_bytes.unwrap_or(0);
                b_key.cmp(&a_key).then_with(|| a.name.cmp(&b.name))
            }),
            SortMode::None => {}
        }

        let mut truncated = false;
        if let Some(max) = max_entries {
            let max = max as usize;
            if entries.len() > max {
                entries.truncate(max);
                truncated = true;
            }
        }

        let result = ListDirResult {
            ok: true,
            dir: dir_display,
            exists: true,
            truncated,
            entries,
        };

        let summary = format!("Listed {} entries in {}", result.entries.len(), result.dir);
        let ui_payload = super::ToolUiPayload::new(Self::name(), ctx.call_id.clone(), summary)
            .with_field("exists", result.exists.to_string())
            .with_field("entries", result.entries.len().to_string())
            .with_field("truncated", result.truncated.to_string());

        let content = serde_json::to_string(&result).map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to serialize ListDirResult: {err}"
            )))
        })?;

        Ok(ToolResult {
            content,
            ui_payload: Some(ui_payload),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{Ctx, Tool};
    use crate::{EventBus, EventBusCaps};
    use crate::app_state::{SystemState, SystemStatus};
    use serde_json::json;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[test]
    fn schema_matches_expected() {
        let schema = ListDir::schema().clone();
        let expected = json!({
            "type": "object",
            "properties": {
                "dir": { "type": "string", "description": DIR_DESC },
                "include_hidden": { "type": "boolean", "description": INCLUDE_HIDDEN_DESC },
                "sort": { "type": "string", "enum": ["name", "mtime", "size", "none"], "description": SORT_DESC },
                "max_entries": { "type": "integer", "minimum": 1, "description": MAX_ENTRIES_DESC }
            },
            "required": ["dir"],
            "additionalProperties": false
        });
        assert_eq!(schema, expected);
    }

    #[test]
    fn into_owned_transfers_fields() {
        let params = ListDirParams {
            dir: Cow::Borrowed("src"),
            include_hidden: Some(true),
            sort: Some(Cow::Borrowed("mtime")),
            max_entries: Some(10),
        };
        let owned = ListDir::into_owned(&params);
        assert_eq!(owned.dir, "src");
        assert!(owned.include_hidden);
        assert_eq!(owned.sort.as_deref(), Some("mtime"));
        assert_eq!(owned.max_entries, Some(10));
    }

    #[tokio::test]
    async fn list_dir_reports_entries() {
        let dir = tempdir().expect("temp dir");
        let file_path = dir.path().join("alpha.txt");
        fs::write(&file_path, "hello").await.expect("write");

        let mut state = crate::test_utils::mock::create_mock_app_state();
        state.system = SystemState::new(SystemStatus::new(Some(dir.path().to_path_buf())));
        let state = std::sync::Arc::new(state);
        let event_bus = std::sync::Arc::new(EventBus::new(EventBusCaps::default()));
        let ctx = Ctx {
            state,
            event_bus,
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            call_id: ploke_core::ArcStr::from("list-dir-test"),
        };

        let params = ListDirParams {
            dir: Cow::Borrowed("."),
            include_hidden: Some(false),
            sort: Some(Cow::Borrowed("name")),
            max_entries: None,
        };
        let result = ListDir::execute(params, ctx).await.expect("execute");
        let parsed: ListDirResult =
            serde_json::from_str(&result.content).expect("parse result");

        assert!(parsed.exists);
        assert!(parsed.entries.iter().any(|e| e.name == "alpha.txt"));
    }
}
