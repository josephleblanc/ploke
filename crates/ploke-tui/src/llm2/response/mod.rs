use std::collections::HashMap;

use crate::tools::ToolCall;

use super::{manager::Role, *};

use serde::{Deserialize, Serialize};
#[derive(Deserialize, Debug, Serialize, Clone)]
pub(crate) struct OpenAiResponse {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) choices: Vec<Choices>,
    #[serde(default)]
    pub(super) created: i64,
    #[serde(default)]
    pub(super) model: String,
    #[serde(default)]
    pub(super) object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) system_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) logprobs: Option<serde_json::Value>,
}

/// Response format specification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ResponseFormat {
    #[default]
    Text,
    JsonObject,
}


/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TokenUsage {
    pub(crate) prompt_tokens: u32,
    pub(crate) completion_tokens: u32,
    pub(crate) total_tokens: u32,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialOrd, PartialEq)]
pub(super) enum ResponseObject {
    #[serde(rename = "chat.completion")]
    ChatCompletion,
    #[serde(rename = "chat.completion.chunk")]
    ChatCompletionChunk,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialOrd, PartialEq)]
pub(super) struct ResponseUsage {
    /** Including images and tools if any */
    pub(super) prompt_tokens: i64,
    /** The tokens generated */
    pub(super) completion_tokens: i64,
    /** Sum of the above two fields */
    pub(super) total_tokens: i64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub(crate) struct Choices {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) logprobs: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) finish_reason: Option<FinishReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) native_finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<ResponseMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<ErrorResponse>,
    // For non-streaming choices that might have text instead of message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    // For streaming choices
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) delta: Option<StreamingDelta>,
}

/// Generation completion reasons
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "snake_case")]
pub(crate) enum FinishReason {
    Stop,          // Natural stop sequence
    Length,        // Max tokens reached
    ContentFilter, // Blocked by safety system
    ToolCalls,     // Stopped for tool execution
    Timeout,       // Processing time exceeded
    Error(String), // Error description
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub(crate) struct StreamingDelta {
    // May be null or string
    pub(super) content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    // May or may not be present
    pub(super) role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    // May or may not be present
    pub(super) tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub(crate) struct ErrorResponse {
    pub(super) code: i64,
    pub(super) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    // Contains additional error information such as provider details, the raw error message, etc.
    // Original is Record<string, unknown>
    pub(super) metadata: Option<HashMap<String, serde_json::Value>>,
}

// Use OpenAI-style normalized tool call shape per OpenRouter docs

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Choice {
    pub(super) message: ResponseMessage,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ResponseMessage {
    // When tool_calls are present, role may be null/absent
    pub(super) role: Option<Role>,
    // When tool_calls are present, content may be null/absent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) logprobs: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) refusal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reasoning: Option<String>,
}

// AI: add unit tests for `OpenAiResponse` serailization and deserialization AI!
