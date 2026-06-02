//! Wasm-safe tool transport contracts for persisted run records and UI readers.
//!
//! Serde-owned DTOs live in `ploke_core::tool_contracts`. This module re-exports them
//! and provides decode helpers that use only `serde_json` (no `ploke-tui` / `mio`).

pub use ploke_core::tool_contracts::{
    ApplyCodeEditResult, ApplyNsPatchResult, CanonicalEditOwned, CargoCommand, CargoDiagnostic,
    CargoScope, CargoSpan, CargoStatusReason, CargoSummary, CargoToolParamsOwned, CargoToolResult,
    CodeEditParamsOwned, ConciseContext, CreateFileParamsOwned, CreateFileResult, EdgesParamsOwned,
    GraphNodeType, InsertRustContainerKind, InsertRustItemParamsOwned, LegacySearchArguments,
    ListDirEntry, ListDirParamsOwned, ListDirResult, LookupParamsOwned, NsPatchOwned,
    NsPatchParamsOwned, NsReadParamsOwned, NsReadResult, PersistedToolCallArguments,
    PersistedToolResultContent, RequestCodeContextParamsOwned, RequestCodeContextResult,
    ToolArgumentDecodeError, ToolArgumentParseFailure, ToolCallArguments, ToolErrorCode,
    ToolErrorWire, ToolItemKind, ToolLlmErrorPayload, ToolLlmErrorValue, ToolResultContent,
    ToolResultDecodeError, ToolResultParseFailure, ToolRetryContext, ToolRetryContextField,
    ToolRetryContextValue, ToolUiField, ToolUiPayload, ToolVerbosity,
};
pub use ploke_core::tool_types::ToolName;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// Provider-supplied tool argument JSON captured as persisted text.
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
        let raw = r#"{"token_budget_per_result":512,"token_budget_total":2048,"search_term":"ToolRequestRecord"}"#;
        let decoded = decode_tool_arguments("request_code_context", raw);

        let PersistedToolCallArguments::Decoded(ToolCallArguments::RequestCodeContext(arguments)) =
            decoded
        else {
            panic!("expected decoded request_code_context arguments");
        };

        assert_eq!(arguments.token_budget_per_result, Some(512));
        assert_eq!(arguments.token_budget_total, Some(2048));
        assert_eq!(arguments.search_term.as_deref(), Some("ToolRequestRecord"));
    }

    #[test]
    fn tool_call_arguments_decode_legacy_request_code_context_budget() {
        let raw = r#"{"token_budget":2048,"search_term":"ToolRequestRecord"}"#;
        let decoded = decode_tool_arguments("request_code_context", raw);

        let PersistedToolCallArguments::Decoded(ToolCallArguments::RequestCodeContext(arguments)) =
            decoded
        else {
            panic!("expected decoded request_code_context arguments");
        };

        assert_eq!(arguments.token_budget_per_result, Some(2048));
        assert_eq!(arguments.token_budget_total, None);
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

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;

    #[test]
    fn wasm32_decode_tool_arguments_smoke() {
        let decoded = decode_tool_arguments("list_dir", r#"{"dir":"crates"}"#);
        assert!(matches!(
            decoded,
            PersistedToolCallArguments::Decoded(ToolCallArguments::ListDir(_))
        ));
    }

    #[test]
    fn wasm32_decode_tool_result_content_smoke() {
        let decoded = decode_tool_result_content(
            "list_dir",
            r#"{"ok":true,"dir":"crates","exists":true,"truncated":false,"entries":[]}"#,
        );
        assert!(matches!(
            decoded,
            PersistedToolResultContent::Decoded(ToolResultContent::ListDir(_))
        ));
    }
}
