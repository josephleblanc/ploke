use std::collections::HashMap;

use crate::tools::ToolCall;

use super::{chat_msg::Role, *};

#[derive(Deserialize, Debug)]
pub(crate) struct OpenAiResponse {
    #[serde(default)]
    id: String,
    #[serde(default)]
    choices: Vec<Choices>,
    #[serde(default)]
    created: i64,
    #[serde(default)]
    model: String,
    #[serde(default)]
    object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logprobs: Option<serde_json::Value>,
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
    prompt_tokens: i64,
    /** The tokens generated */
    completion_tokens: i64,
    /** Sum of the above two fields */
    total_tokens: i64,
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
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    // May or may not be present
    role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    // May or may not be present
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub(crate) struct ErrorResponse {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    // Contains additional error information such as provider details, the raw error message, etc.
    // Original is Record<string, unknown>
    metadata: Option<HashMap<String, serde_json::Value>>,
}

// Use OpenAI-style normalized tool call shape per OpenRouter docs

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct ResponseMessage {
    // When tool_calls are present, role may be null/absent
    role: Option<Role>,
    // When tool_calls are present, content may be null/absent
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logprobs: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refusal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<String>,
}
