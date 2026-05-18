//! Re-exports of the serializable tool transport contracts.
//!
//! These types are defined beside the `ploke-tui` tools because the tool
//! implementation is the place where the LLM-facing argument and result shapes
//! are maintained. `ploke-records` re-exports the owned transport DTOs instead
//! of mirroring them so persisted tool-call readers use the same Rust shapes the
//! tools use when serializing/deserializing LLM tool traffic.
//!
//! This module is intentionally only a passive records surface. It must not
//! expose `ploke-tui` execution state, app context, event buses, async tool
//! execution, database handles, or edit authority. The goal is type identity for
//! persisted/replay/UI readers, not access to the TUI runtime.

pub use ploke_tui::tools::{
    ApplyCodeEditResult, ConciseContext, CreateFileResult, RequestCodeContextResult, ToolErrorCode,
    ToolErrorWire, ToolLlmErrorPayload, ToolLlmErrorValue, ToolName, ToolRetryContext,
    ToolRetryContextField, ToolRetryContextValue, ToolUiField, ToolUiPayload, ToolVerbosity,
    cargo::{
        CargoCommand, CargoDiagnostic, CargoScope, CargoSpan, CargoStatusReason, CargoSummary,
        CargoToolParamsOwned, CargoToolResult,
    },
    code_edit::{CanonicalEditOwned, CodeEditParamsOwned},
    code_item_lookup::LookupParamsOwned,
    create_file::CreateFileParamsOwned,
    get_code_edges::EdgesParamsOwned,
    insert_rust_item::InsertRustItemParamsOwned,
    list_dir::{ListDirEntry, ListDirParamsOwned, ListDirResult},
    ns_patch::{ApplyNsPatchResult, NsPatchOwned, NsPatchParamsOwned},
    ns_read::{NsReadParamsOwned, NsReadResult},
    request_code_context::RequestCodeContextParamsOwned,
};

use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use std::fmt;

/// Provider-supplied tool argument JSON captured as persisted text.
///
/// This is intentionally transparent on the wire so older run records that
/// stored `"arguments": "{...}"` continue to deserialize, while production
/// readers get a named carrier instead of a bare `String`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ToolArgumentsJson {
    raw: String,
}

impl<'de> Deserialize<'de> for ToolArgumentsJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let captured = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;
        let raw = match serde_json::from_str::<String>(captured.get()) {
            Ok(raw) => raw,
            Err(_) => captured.get().to_string(),
        };
        Ok(Self { raw })
    }
}

impl ToolArgumentsJson {
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn decode_for_tool(&self, tool: &str) -> PersistedToolCallArguments {
        decode_tool_arguments(tool, &self.raw)
    }
}

impl From<String> for ToolArgumentsJson {
    fn from(raw: String) -> Self {
        Self { raw }
    }
}

impl From<&str> for ToolArgumentsJson {
    fn from(raw: &str) -> Self {
        Self {
            raw: raw.to_string(),
        }
    }
}

impl fmt::Display for ToolArgumentsJson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

/// Typed persisted view of a tool-call argument payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", content = "record", rename_all = "snake_case")]
pub enum PersistedToolCallArguments {
    Decoded(ToolCallArguments),
    ParseFailure(ToolArgumentParseFailure),
}

impl PersistedToolCallArguments {
    pub fn decoded(&self) -> Option<&ToolCallArguments> {
        match self {
            Self::Decoded(arguments) => Some(arguments),
            Self::ParseFailure(_) => None,
        }
    }

    pub fn parse_failure(&self) -> Option<&ToolArgumentParseFailure> {
        match self {
            Self::Decoded(_) => None,
            Self::ParseFailure(failure) => Some(failure),
        }
    }
}

/// Closed enum of currently owned tool argument DTOs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "arguments")]
pub enum ToolCallArguments {
    #[serde(rename = "request_code_context")]
    RequestCodeContext(RequestCodeContextParamsOwned),
    #[serde(rename = "apply_code_edit")]
    ApplyCodeEdit(CodeEditParamsOwned),
    #[serde(rename = "insert_rust_item")]
    InsertRustItem(InsertRustItemParamsOwned),
    #[serde(rename = "create_file")]
    CreateFile(CreateFileParamsOwned),
    #[serde(rename = "non_semantic_patch")]
    NsPatch(NsPatchParamsOwned),
    #[serde(rename = "read_file")]
    NsRead(NsReadParamsOwned),
    #[serde(rename = "code_item_lookup")]
    CodeItemLookup(LookupParamsOwned),
    #[serde(rename = "code_item_edges")]
    CodeItemEdges(EdgesParamsOwned),
    #[serde(rename = "cargo")]
    Cargo(CargoToolParamsOwned),
    #[serde(rename = "list_dir")]
    ListDir(ListDirParamsOwned),
    #[serde(rename = "search_code")]
    SearchCode(LegacySearchArguments),
    #[serde(rename = "search_symbols")]
    SearchSymbols(LegacySearchArguments),
    #[serde(rename = "query_codebase")]
    QueryCodebase(LegacySearchArguments),
}

impl ToolCallArguments {
    pub fn tool_name(&self) -> Option<ToolName> {
        match self {
            Self::RequestCodeContext(_) => Some(ToolName::RequestCodeContext),
            Self::ApplyCodeEdit(_) => Some(ToolName::ApplyCodeEdit),
            Self::InsertRustItem(_) => Some(ToolName::InsertRustItem),
            Self::CreateFile(_) => Some(ToolName::CreateFile),
            Self::NsPatch(_) => Some(ToolName::NsPatch),
            Self::NsRead(_) => Some(ToolName::NsRead),
            Self::CodeItemLookup(_) => Some(ToolName::CodeItemLookup),
            Self::CodeItemEdges(_) => Some(ToolName::CodeItemEdges),
            Self::Cargo(_) => Some(ToolName::Cargo),
            Self::ListDir(_) => Some(ToolName::ListDir),
            Self::SearchCode(_) | Self::SearchSymbols(_) | Self::QueryCodebase(_) => None,
        }
    }
}

/// Historical search tool argument shape kept for replay/protocol context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LegacySearchArguments {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_term: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

/// Typed record emitted when a persisted/provider argument string cannot be
/// decoded into the tool-specific DTO.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolArgumentParseFailure {
    pub tool: String,
    pub raw_arguments: String,
    pub error: ToolArgumentDecodeError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolArgumentDecodeError {
    UnknownTool { tool: String },
    InvalidJson { message: String },
}

pub fn decode_tool_arguments(tool: &str, raw: &str) -> PersistedToolCallArguments {
    match tool {
        "search_code" => return decode_as(tool, raw, ToolCallArguments::SearchCode),
        "search_symbols" => return decode_as(tool, raw, ToolCallArguments::SearchSymbols),
        "query_codebase" => return decode_as(tool, raw, ToolCallArguments::QueryCodebase),
        _ => {}
    }

    let Some(tool_name) = tool_name_from_persisted(tool) else {
        return PersistedToolCallArguments::ParseFailure(ToolArgumentParseFailure {
            tool: tool.to_string(),
            raw_arguments: raw.to_string(),
            error: ToolArgumentDecodeError::UnknownTool {
                tool: tool.to_string(),
            },
        });
    };

    match tool_name {
        ToolName::RequestCodeContext => decode_as(
            tool_name.as_str(),
            raw,
            ToolCallArguments::RequestCodeContext,
        ),
        ToolName::ApplyCodeEdit => {
            decode_as(tool_name.as_str(), raw, ToolCallArguments::ApplyCodeEdit)
        }
        ToolName::InsertRustItem => {
            decode_as(tool_name.as_str(), raw, ToolCallArguments::InsertRustItem)
        }
        ToolName::CreateFile => decode_as(tool_name.as_str(), raw, ToolCallArguments::CreateFile),
        ToolName::NsPatch => decode_as(tool_name.as_str(), raw, ToolCallArguments::NsPatch),
        ToolName::NsRead => decode_as(tool_name.as_str(), raw, ToolCallArguments::NsRead),
        ToolName::CodeItemLookup => {
            decode_as(tool_name.as_str(), raw, ToolCallArguments::CodeItemLookup)
        }
        ToolName::CodeItemEdges => {
            decode_as(tool_name.as_str(), raw, ToolCallArguments::CodeItemEdges)
        }
        ToolName::Cargo => decode_as(tool_name.as_str(), raw, ToolCallArguments::Cargo),
        ToolName::ListDir => decode_as(tool_name.as_str(), raw, ToolCallArguments::ListDir),
    }
}

fn decode_as<T>(
    tool: &str,
    raw: &str,
    wrap: impl FnOnce(T) -> ToolCallArguments,
) -> PersistedToolCallArguments
where
    T: DeserializeOwned,
{
    match serde_json::from_str::<T>(raw) {
        Ok(arguments) => PersistedToolCallArguments::Decoded(wrap(arguments)),
        Err(source) => PersistedToolCallArguments::ParseFailure(ToolArgumentParseFailure {
            tool: tool.to_string(),
            raw_arguments: raw.to_string(),
            error: ToolArgumentDecodeError::InvalidJson {
                message: source.to_string(),
            },
        }),
    }
}

/// Typed persisted view of a tool result `content` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", content = "record", rename_all = "snake_case")]
pub enum PersistedToolResultContent {
    Decoded(ToolResultContent),
    ParseFailure(ToolResultParseFailure),
}

impl PersistedToolResultContent {
    pub fn decoded(&self) -> Option<&ToolResultContent> {
        match self {
            Self::Decoded(result) => Some(result),
            Self::ParseFailure(_) => None,
        }
    }

    pub fn parse_failure(&self) -> Option<&ToolResultParseFailure> {
        match self {
            Self::Decoded(_) => None,
            Self::ParseFailure(failure) => Some(failure),
        }
    }
}

/// Closed enum of currently owned tool result DTOs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "result")]
pub enum ToolResultContent {
    #[serde(rename = "request_code_context")]
    RequestCodeContext(RequestCodeContextResult),
    #[serde(rename = "apply_code_edit")]
    ApplyCodeEdit(ApplyCodeEditResult),
    #[serde(rename = "insert_rust_item")]
    InsertRustItem(ApplyCodeEditResult),
    #[serde(rename = "create_file")]
    CreateFile(CreateFileResult),
    #[serde(rename = "non_semantic_patch")]
    NsPatch(ApplyNsPatchResult),
    #[serde(rename = "read_file")]
    NsRead(NsReadResult),
    #[serde(rename = "code_item_lookup")]
    CodeItemLookup(ConciseContext),
    #[serde(rename = "cargo")]
    Cargo(CargoToolResult),
    #[serde(rename = "list_dir")]
    ListDir(ListDirResult),
}

impl ToolResultContent {
    pub fn tool_name(&self) -> ToolName {
        match self {
            Self::RequestCodeContext(_) => ToolName::RequestCodeContext,
            Self::ApplyCodeEdit(_) => ToolName::ApplyCodeEdit,
            Self::InsertRustItem(_) => ToolName::InsertRustItem,
            Self::CreateFile(_) => ToolName::CreateFile,
            Self::NsPatch(_) => ToolName::NsPatch,
            Self::NsRead(_) => ToolName::NsRead,
            Self::CodeItemLookup(_) => ToolName::CodeItemLookup,
            Self::Cargo(_) => ToolName::Cargo,
            Self::ListDir(_) => ToolName::ListDir,
        }
    }
}

/// Typed record emitted when a persisted tool result string cannot be decoded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResultParseFailure {
    pub tool: String,
    pub raw_content: String,
    pub error: ToolResultDecodeError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolResultDecodeError {
    UnknownTool { tool: String },
    UnsupportedToolResult { tool: String },
    InvalidJson { message: String },
}

pub fn decode_tool_result_content(tool: &str, raw: &str) -> PersistedToolResultContent {
    let Some(tool_name) = tool_name_from_persisted(tool) else {
        return PersistedToolResultContent::ParseFailure(ToolResultParseFailure {
            tool: tool.to_string(),
            raw_content: raw.to_string(),
            error: ToolResultDecodeError::UnknownTool {
                tool: tool.to_string(),
            },
        });
    };

    match tool_name {
        ToolName::RequestCodeContext => decode_result_as(
            tool_name.as_str(),
            raw,
            ToolResultContent::RequestCodeContext,
        ),
        ToolName::ApplyCodeEdit => {
            decode_result_as(tool_name.as_str(), raw, ToolResultContent::ApplyCodeEdit)
        }
        ToolName::InsertRustItem => {
            decode_result_as(tool_name.as_str(), raw, ToolResultContent::InsertRustItem)
        }
        ToolName::CreateFile => {
            decode_result_as(tool_name.as_str(), raw, ToolResultContent::CreateFile)
        }
        ToolName::NsPatch => decode_result_as(tool_name.as_str(), raw, ToolResultContent::NsPatch),
        ToolName::NsRead => decode_result_as(tool_name.as_str(), raw, ToolResultContent::NsRead),
        ToolName::CodeItemLookup => {
            decode_result_as(tool_name.as_str(), raw, ToolResultContent::CodeItemLookup)
        }
        ToolName::CodeItemEdges => {
            PersistedToolResultContent::ParseFailure(ToolResultParseFailure {
                tool: tool_name.as_str().to_string(),
                raw_content: raw.to_string(),
                error: ToolResultDecodeError::UnsupportedToolResult {
                    tool: tool_name.as_str().to_string(),
                },
            })
        }
        ToolName::Cargo => decode_result_as(tool_name.as_str(), raw, ToolResultContent::Cargo),
        ToolName::ListDir => decode_result_as(tool_name.as_str(), raw, ToolResultContent::ListDir),
    }
}

fn decode_result_as<T>(
    tool: &str,
    raw: &str,
    wrap: impl FnOnce(T) -> ToolResultContent,
) -> PersistedToolResultContent
where
    T: DeserializeOwned,
{
    match serde_json::from_str::<T>(raw) {
        Ok(result) => PersistedToolResultContent::Decoded(wrap(result)),
        Err(source) => PersistedToolResultContent::ParseFailure(ToolResultParseFailure {
            tool: tool.to_string(),
            raw_content: raw.to_string(),
            error: ToolResultDecodeError::InvalidJson {
                message: source.to_string(),
            },
        }),
    }
}

fn tool_name_from_persisted(tool: &str) -> Option<ToolName> {
    match tool {
        "request_code_context" => Some(ToolName::RequestCodeContext),
        "apply_code_edit" => Some(ToolName::ApplyCodeEdit),
        "insert_rust_item" => Some(ToolName::InsertRustItem),
        "create_file" => Some(ToolName::CreateFile),
        "non_semantic_patch" | "ns_patch" => Some(ToolName::NsPatch),
        "read_file" | "ns_read" => Some(ToolName::NsRead),
        "code_item_lookup" => Some(ToolName::CodeItemLookup),
        "code_item_edges" => Some(ToolName::CodeItemEdges),
        "cargo" => Some(ToolName::Cargo),
        "list_dir" => Some(ToolName::ListDir),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_call_arguments_decode_request_code_context() {
        let raw = r#"{"token_budget":2048,"search_term":"ToolRequestRecord"}"#;
        let decoded = decode_tool_arguments("request_code_context", raw);

        let PersistedToolCallArguments::Decoded(ToolCallArguments::RequestCodeContext(arguments)) =
            decoded
        else {
            panic!("expected decoded request_code_context arguments");
        };

        assert_eq!(arguments.token_budget, Some(2048));
        assert_eq!(arguments.search_term.as_deref(), Some("ToolRequestRecord"));
    }

    #[test]
    fn tool_call_arguments_decode_legacy_search_code_query() {
        let raw = r#"{"query":"handle_request"}"#;
        let decoded = decode_tool_arguments("search_code", raw);

        let PersistedToolCallArguments::Decoded(ToolCallArguments::SearchCode(arguments)) = decoded
        else {
            panic!("expected decoded legacy search_code arguments");
        };

        assert_eq!(arguments.query.as_deref(), Some("handle_request"));
        assert_eq!(arguments.search_term, None);
    }

    #[test]
    fn tool_call_arguments_decode_legacy_search_symbols_search_term() {
        let raw = r#"{"search_term":"ToolRequestRecord"}"#;
        let decoded = decode_tool_arguments("search_symbols", raw);

        let PersistedToolCallArguments::Decoded(ToolCallArguments::SearchSymbols(arguments)) =
            decoded
        else {
            panic!("expected decoded legacy search_symbols arguments");
        };

        assert_eq!(arguments.search_term.as_deref(), Some("ToolRequestRecord"));
        assert_eq!(arguments.query, None);
    }

    #[test]
    fn tool_call_arguments_record_json_parse_failure() {
        let decoded = decode_tool_arguments("request_code_context", "{");

        let PersistedToolCallArguments::ParseFailure(failure) = decoded else {
            panic!("expected parse failure");
        };

        assert_eq!(failure.tool, "request_code_context");
        assert!(matches!(
            failure.error,
            ToolArgumentDecodeError::InvalidJson { .. }
        ));
    }

    #[test]
    fn tool_call_arguments_record_unknown_tool_failure() {
        let decoded = decode_tool_arguments("legacy_tool", "{}");

        let PersistedToolCallArguments::ParseFailure(failure) = decoded else {
            panic!("expected unknown-tool failure");
        };

        assert_eq!(
            failure.error,
            ToolArgumentDecodeError::UnknownTool {
                tool: "legacy_tool".to_string()
            }
        );
    }

    #[test]
    fn tool_result_content_decodes_ns_read() {
        let raw = r#"{
            "ok":true,
            "file_path":"crates/ploke-records/src/tool_contracts.rs",
            "exists":true,
            "byte_len":128,
            "start_line":1,
            "end_line":4,
            "truncated":false,
            "content":"pub mod tool_contracts;",
            "file_hash":null
        }"#;
        let decoded = decode_tool_result_content("read_file", raw);

        let PersistedToolResultContent::Decoded(ToolResultContent::NsRead(result)) = decoded else {
            panic!("expected decoded ns read result");
        };

        assert_eq!(
            result.file_path,
            "crates/ploke-records/src/tool_contracts.rs"
        );
        assert_eq!(result.byte_len, Some(128));
    }

    #[test]
    fn tool_result_content_records_json_failure() {
        let decoded = decode_tool_result_content("list_dir", "{");

        let PersistedToolResultContent::ParseFailure(failure) = decoded else {
            panic!("expected parse failure");
        };

        assert_eq!(failure.tool, "list_dir");
        assert!(matches!(
            failure.error,
            ToolResultDecodeError::InvalidJson { .. }
        ));
    }

    #[test]
    fn tool_call_arguments_json_keeps_legacy_wire_shape() {
        let captured = ToolArgumentsJson::from(r#"{"dir":"crates"}"#);
        let serialized = serde_json::to_string(&captured).expect("serialize");
        assert_eq!(serialized, r#""{\"dir\":\"crates\"}""#);

        let roundtrip: ToolArgumentsJson = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(roundtrip.as_str(), r#"{"dir":"crates"}"#);
    }

    #[test]
    fn tool_call_arguments_json_accepts_legacy_object_shape() {
        let captured: ToolArgumentsJson =
            serde_json::from_str(r#"{"file":"src/lib.rs","start_line":1}"#)
                .expect("deserialize legacy object arguments");

        assert_eq!(captured.as_str(), r#"{"file":"src/lib.rs","start_line":1}"#);
    }

    #[test]
    fn tool_error_wire_roundtrips_typed_retry_context() {
        let raw = r#"{
            "user":"create_file: invalid path",
            "llm":{
                "ok":false,
                "tool":"create_file",
                "code":"invalid_format",
                "field":"file_path",
                "expected":null,
                "received":null,
                "message":"invalid path",
                "snippet":null,
                "retry_hint":"Use a workspace-root-relative path.",
                "retry_context":{
                    "fields":[
                        {"name":"input_path","value":{"kind":"string","value":"../outside.rs"}},
                        {"name":"expected","value":{"kind":"string_list","value":["error","overwrite"]}}
                    ]
                }
            },
            "system":"tool=CreateFile code=InvalidFormat: invalid path"
        }"#;

        let wire = ToolErrorWire::parse(raw).expect("parse typed tool error wire");
        assert_eq!(wire.llm.code, ToolErrorCode::InvalidFormat);
        assert_eq!(
            wire.llm
                .retry_context
                .as_ref()
                .and_then(|ctx| ctx.get("input_path"))
                .and_then(ToolRetryContextValue::as_str),
            Some("../outside.rs")
        );

        let encoded = serde_json::to_string(&wire).expect("serialize typed tool error wire");
        let reparsed = ToolErrorWire::parse(&encoded).expect("reparse typed tool error wire");
        assert_eq!(
            reparsed.llm["retry_context"]
                .as_object()
                .and_then(|ctx| ctx.get("input_path"))
                .and_then(ToolLlmErrorValue::as_str),
            Some("../outside.rs")
        );
    }

    #[test]
    fn tool_error_wire_serde_deserialize_initializes_llm_index() {
        let raw = r#"{
            "user":"create_file: invalid path",
            "llm":{
                "ok":false,
                "tool":"create_file",
                "code":"invalid_format",
                "field":"file_path",
                "expected":null,
                "received":null,
                "message":"invalid path",
                "snippet":null,
                "retry_hint":"Use a workspace-root-relative path.",
                "retry_context":{
                    "fields":[
                        {"name":"input_path","value":{"kind":"string","value":"../outside.rs"}}
                    ]
                }
            },
            "system":"tool=CreateFile code=InvalidFormat: invalid path"
        }"#;

        let wire: ToolErrorWire =
            serde_json::from_str(raw).expect("serde-deserialize typed tool error wire");

        assert_eq!(
            wire.llm["retry_context"]
                .as_object()
                .and_then(|ctx| ctx.get("input_path"))
                .and_then(ToolLlmErrorValue::as_str),
            Some("../outside.rs")
        );
    }

    #[test]
    fn tool_error_wire_accepts_legacy_debug_code_spelling() {
        let raw = r#"{
            "user":"create_file: invalid path",
            "llm":{
                "ok":false,
                "tool":"create_file",
                "code":"InvalidFormat",
                "field":"file_path",
                "expected":null,
                "received":null,
                "message":"invalid path",
                "snippet":null,
                "retry_hint":null,
                "retry_context":null
            },
            "system":"tool=CreateFile code=InvalidFormat: invalid path"
        }"#;

        let wire = ToolErrorWire::parse(raw).expect("parse legacy code spelling");

        assert_eq!(wire.llm.code, ToolErrorCode::InvalidFormat);
        assert_eq!(
            wire.llm["code"].as_str(),
            Some("InvalidFormat"),
            "public index keeps the previous LLM-facing debug code label"
        );
    }
}
