//! Passive owners for benchmark agent-turn artifacts.
//!
//! `agent-turn-trace.json` and `agent-turn-summary.json` currently share the
//! same wire shape. The trace file is rewritten as the turn progresses; the
//! summary file is the final artifact. These records mirror the persisted shape
//! only. They do not grant tool execution, replay, or runtime authority.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::record::{Record, RecordFamily, RecordFormat};
use crate::tool_contracts::{ToolArgumentsJson, ToolName};

pub const AGENT_TURN_TRACE_SCHEMA_V1: &str = "agent-turn-trace.v1";
pub const AGENT_TURN_SUMMARY_SCHEMA_V1: &str = "agent-turn-summary.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentTurnTraceRecord(pub AgentTurnArtifactRecord);

impl Record for AgentTurnTraceRecord {
    const FAMILY: RecordFamily = RecordFamily::AgentTurnTrace;
    const SCHEMA: &'static str = AGENT_TURN_TRACE_SCHEMA_V1;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentTurnSummaryRecord(pub AgentTurnArtifactRecord);

impl Record for AgentTurnSummaryRecord {
    const FAMILY: RecordFamily = RecordFamily::AgentTurnSummary;
    const SCHEMA: &'static str = AGENT_TURN_SUMMARY_SCHEMA_V1;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTurnArtifactRecord {
    pub task_id: String,
    pub selected_model: String,
    pub issue_prompt: String,
    pub user_message_id: String,
    pub events: Vec<ObservedTurnEventRecord>,
    pub prompt_debug: Option<String>,
    pub terminal_record: Option<TurnFinishedRecord>,
    pub final_assistant_message: Option<MessageSnapshotRecord>,
    pub patch_artifact: PatchArtifactRecord,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub llm_prompt: Vec<RequestMessageRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_response: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservedTurnEventRecord {
    DebugCommand(String),
    LlmEvent(String),
    LlmResponse(LlmResponseRecord),
    ToolRequested(ToolRequestRecord),
    ToolCompleted(ToolCompletedRecord),
    ToolFailed(ToolFailedRecord),
    MessageUpdated(MessageSnapshotRecord),
    TurnFinished(TurnFinishedRecord),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmResponseRecord {
    pub content: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsageRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReasonRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<LlmMetadataRecord>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TokenUsageRecord {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReasonRecord {
    Stop,
    Length,
    ContentFilter,
    ToolCalls,
    Timeout,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmMetadataRecord {
    pub model: String,
    pub usage: TokenUsageRecord,
    pub finish_reason: FinishReasonRecord,
    pub processing_time: Duration,
    pub cost: f64,
    pub performance: PerformanceMetricsRecord,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerformanceMetricsRecord {
    pub tokens_per_second: f32,
    pub time_to_first_token: Duration,
    pub queue_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequestMessageRecord {
    pub role: RequestRoleRecord,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ProviderToolCallRecord>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RequestRoleRecord {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderToolCallRecord {
    #[serde(rename = "id")]
    pub call_id: String,
    #[serde(rename = "type")]
    pub call_type: FunctionCallMarker,
    pub function: ProviderFunctionCallRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionCallMarker;

impl FunctionCallMarker {
    pub const VALUE: &'static str = "function";
}

impl Serialize for FunctionCallMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for FunctionCallMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = FunctionCallMarker;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("\"function\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value == FunctionCallMarker::VALUE {
                    Ok(FunctionCallMarker)
                } else {
                    Err(E::invalid_value(serde::de::Unexpected::Str(value), &self))
                }
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderFunctionCallRecord {
    pub name: ToolName,
    pub arguments: ToolArgumentsJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolRequestRecord {
    pub request_id: String,
    pub parent_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: ToolArgumentsJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCompletedRecord {
    pub request_id: String,
    pub parent_id: String,
    pub call_id: String,
    pub tool: String,
    pub content: String,
    pub ui_payload: Option<ToolUiPayloadRecord>,
    #[serde(default)]
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolFailedRecord {
    pub request_id: String,
    pub parent_id: String,
    pub call_id: String,
    pub tool: Option<String>,
    pub error: String,
    pub ui_payload: Option<ToolUiPayloadRecord>,
    #[serde(default)]
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolVerbosityRecord {
    Minimal,
    Normal,
    Verbose,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolUiFieldRecord {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolUiPayloadRecord {
    pub tool: ToolName,
    pub call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<ToolUiFieldRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    pub verbosity: ToolVerbosityRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolErrorWireRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ToolErrorCodeRecord>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorCodeRecord {
    #[serde(alias = "FieldTooLarge")]
    FieldTooLarge,
    #[serde(alias = "WrongType")]
    WrongType,
    #[serde(alias = "MissingField")]
    MissingField,
    #[serde(alias = "MalformedDiff")]
    MalformedDiff,
    #[serde(alias = "InvalidFormat")]
    InvalidFormat,
    #[serde(alias = "Io")]
    Io,
    #[serde(alias = "Timeout")]
    Timeout,
    #[serde(alias = "Internal")]
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolErrorWireRecord {
    pub user: String,
    pub llm: ToolLlmErrorPayloadRecord,
    pub system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolLlmErrorPayloadRecord {
    pub ok: bool,
    pub tool: ToolName,
    pub code: ToolErrorCodeRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub received: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_context: Option<ToolRetryContextRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolRetryContextRecord {
    pub fields: Vec<ToolRetryContextFieldRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolRetryContextFieldRecord {
    pub name: String,
    pub value: ToolRetryContextValueRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ToolRetryContextValueRecord {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    StringList(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MessageSnapshotRecord {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub tool_call_id: Option<String>,
    pub content_len: usize,
    pub content_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TurnFinishedRecord {
    pub session_id: String,
    pub request_id: String,
    pub parent_id: String,
    pub assistant_message_id: String,
    pub outcome: String,
    pub error_id: Option<String>,
    pub summary: String,
    pub attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProposalSnapshotRecord {
    pub request_id: String,
    pub call_id: String,
    pub status: String,
    pub files: Vec<String>,
    pub preview_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchArtifactRecord {
    pub edit_proposals: Vec<ProposalSnapshotRecord>,
    pub create_proposals: Vec<ProposalSnapshotRecord>,
    pub applied: bool,
    pub all_proposals_applied: bool,
    pub expected_file_changes: Vec<ExpectedFileChangeRecord>,
    pub any_expected_file_changed: bool,
    pub all_expected_files_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExpectedFileChangeRecord {
    pub path: String,
    pub existed_before: bool,
    pub exists_after: bool,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
    pub changed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_turn_trace_accepts_current_artifact_shape() {
        let json = r#"{
            "task_id": "task-1",
            "selected_model": "anthropic/claude-sonnet-4",
            "issue_prompt": "Fix the bug.",
            "user_message_id": "user-1",
            "events": [
                {"DebugCommand": "pump"},
                {"LlmResponse": {
                    "content": "I will inspect the code.",
                    "model": "anthropic/claude-sonnet-4",
                    "usage": {
                        "prompt_tokens": 10,
                        "completion_tokens": 5,
                        "total_tokens": 15
                    },
                    "finish_reason": "tool_calls"
                }},
                {"ToolRequested": {
                    "request_id": "req-1",
                    "parent_id": "parent-1",
                    "call_id": "call-1",
                    "tool": "read_file",
                    "arguments": "{\"file\":\"src/lib.rs\"}"
                }},
                {"ToolCompleted": {
                    "request_id": "req-1",
                    "parent_id": "parent-1",
                    "call_id": "call-1",
                    "tool": "read_file",
                    "content": "ok",
                    "ui_payload": null,
                    "latency_ms": 42
                }},
                {"ToolFailed": {
                    "request_id": "req-2",
                    "parent_id": "parent-1",
                    "call_id": "call-2",
                    "tool": "apply_code_edit",
                    "error": "no match",
                    "ui_payload": null,
                    "latency_ms": 13
                }},
                {"MessageUpdated": {
                    "id": "message-1",
                    "kind": "Assistant",
                    "status": "Completed",
                    "tool_call_id": null,
                    "content_len": 21,
                    "content_preview": "I will inspect the code"
                }},
                {"TurnFinished": {
                    "session_id": "session-1",
                    "request_id": "req-1",
                    "parent_id": "parent-1",
                    "assistant_message_id": "assistant-1",
                    "outcome": "completed",
                    "error_id": null,
                    "summary": "done",
                    "attempts": 1
                }}
            ],
            "prompt_debug": null,
            "terminal_record": null,
            "final_assistant_message": null,
            "patch_artifact": {
                "edit_proposals": [],
                "create_proposals": [],
                "applied": false,
                "all_proposals_applied": false,
                "expected_file_changes": [],
                "any_expected_file_changed": false,
                "all_expected_files_changed": false
            },
            "llm_prompt": [
                {
                    "role": "assistant",
                    "content": "Calling tools...",
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"file\":\"src/lib.rs\"}"
                        }
                    }]
                },
                {
                    "role": "tool",
                    "content": "ok",
                    "tool_call_id": "call-1"
                }
            ],
            "llm_response": "I will inspect the code."
        }"#;

        let trace: AgentTurnTraceRecord =
            serde_json::from_str(json).expect("current trace shape should deserialize");

        assert_eq!(trace.0.events.len(), 7);
        assert_eq!(trace.0.llm_prompt.len(), 2);
        let ObservedTurnEventRecord::ToolRequested(request) = &trace.0.events[2] else {
            panic!("expected tool request event");
        };
        assert_eq!(request.arguments.as_str(), r#"{"file":"src/lib.rs"}"#);
    }

    #[test]
    fn agent_turn_summary_uses_same_artifact_wire_shape() {
        let artifact = AgentTurnArtifactRecord {
            task_id: "task-1".to_string(),
            selected_model: "openai/gpt-5".to_string(),
            issue_prompt: "Fix the bug.".to_string(),
            user_message_id: "user-1".to_string(),
            events: Vec::new(),
            prompt_debug: None,
            terminal_record: None,
            final_assistant_message: None,
            patch_artifact: PatchArtifactRecord {
                edit_proposals: Vec::new(),
                create_proposals: Vec::new(),
                applied: false,
                all_proposals_applied: false,
                expected_file_changes: Vec::new(),
                any_expected_file_changed: false,
                all_expected_files_changed: false,
            },
            llm_prompt: Vec::new(),
            llm_response: None,
        };

        let summary = AgentTurnSummaryRecord(artifact);
        let encoded = serde_json::to_string(&summary).expect("serialize summary");
        let decoded: AgentTurnSummaryRecord =
            serde_json::from_str(&encoded).expect("deserialize summary");

        assert_eq!(decoded.0.task_id, "task-1");
    }

    #[test]
    fn tool_ui_payload_record_roundtrips_current_agent_turn_shape() {
        let payload = ToolUiPayloadRecord {
            tool: ToolName::ApplyCodeEdit,
            call_id: "call-1".to_string(),
            request_id: Some("7a703948-f5df-4c35-aef6-631cd169ef7c".to_string()),
            proposal_id: Some("4c775c08-c673-4e88-8c36-209c83f3ba45".to_string()),
            summary: "edit staged".to_string(),
            fields: vec![
                ToolUiFieldRecord {
                    name: "status".to_string(),
                    value: "staged".to_string(),
                },
                ToolUiFieldRecord {
                    name: "file".to_string(),
                    value: "src/lib.rs".to_string(),
                },
            ],
            details: Some("Ready to apply".to_string()),
            verbosity: ToolVerbosityRecord::Normal,
            error: Some(ToolErrorWireRecord {
                user: "apply_code_edit: invalid patch".to_string(),
                llm: ToolLlmErrorPayloadRecord {
                    ok: false,
                    tool: ToolName::ApplyCodeEdit,
                    code: ToolErrorCodeRecord::InvalidFormat,
                    field: Some("patch".to_string()),
                    expected: Some("unified diff".to_string()),
                    received: Some("plain text".to_string()),
                    message: "invalid patch".to_string(),
                    snippet: Some("@@ broken".to_string()),
                    retry_hint: Some("Send a unified diff".to_string()),
                    retry_context: Some(ToolRetryContextRecord {
                        fields: vec![ToolRetryContextFieldRecord {
                            name: "candidate_files".to_string(),
                            value: ToolRetryContextValueRecord::StringList(vec![
                                "src/lib.rs".to_string(),
                                "src/main.rs".to_string(),
                            ]),
                        }],
                    }),
                },
                system: "tool=ApplyCodeEdit code=InvalidFormat: invalid patch".to_string(),
            }),
            error_code: Some(ToolErrorCodeRecord::InvalidFormat),
        };

        let encoded = serde_json::to_string(&payload).expect("serialize tool ui payload record");
        let decoded: ToolUiPayloadRecord =
            serde_json::from_str(&encoded).expect("deserialize tool ui payload record");

        assert_eq!(decoded, payload);
    }
}
