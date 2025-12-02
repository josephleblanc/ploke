use std::{borrow::Cow, ops::Deref as _};

use serde::{Deserialize, Serialize};

use super::{ToolDescr, ToolName};
use crate::tools::ToolResult;

const FILE_DESC: &str = "Absolute or workspace-relative file path.";
const START_LINE_DESC: &str = "Optional 1-based line from which to start reading.";
const END_LINE_DESC: &str = "Optional 1-based line at which to stop reading (inclusive).";
const MAX_BYTES_DESC: &str = "Maximum number of UTF-8 bytes to return. Defaults to editor config.";
const TRACKING_HASH_DESC: &str =
    "Optional tracking hash to enforce when reading verified Rust files.";

lazy_static::lazy_static! {
    static ref NS_READ_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "file": { "type": "string", "description": FILE_DESC },
            "start_line": { "type": "integer", "minimum": 1, "description": START_LINE_DESC },
            "end_line": { "type": "integer", "minimum": 1, "description": END_LINE_DESC },
            "max_bytes": { "type": "integer", "minimum": 1, "description": MAX_BYTES_DESC },
            "tracking_hash": { "type": "string", "description": TRACKING_HASH_DESC }
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
    #[serde(borrow)]
    pub tracking_hash: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NsReadParamsOwned {
    pub file: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub max_bytes: Option<u32>,
    pub tracking_hash: Option<String>,
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
            tracking_hash: params.tracking_hash.clone().map(|hash| hash.into_owned()),
        }
    }

    async fn execute<'de>(
        _params: Self::Params<'de>,
        _ctx: super::Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        Err(ploke_error::Error::Internal(
            ploke_error::InternalError::NotImplemented(
                "NsRead::execute pending implementation".to_string(),
            ),
        ))
    }
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
                "tracking_hash": { "type": "string", "description": TRACKING_HASH_DESC }
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
            tracking_hash: Some(Cow::Borrowed("abc123")),
        };
        let owned = NsRead::into_owned(&params);
        assert_eq!(owned.file, "src/lib.rs");
        assert_eq!(owned.start_line, Some(10));
        assert_eq!(owned.end_line, Some(20));
        assert_eq!(owned.max_bytes, Some(1024));
        assert_eq!(owned.tracking_hash.as_deref(), Some("abc123"));
    }
}
