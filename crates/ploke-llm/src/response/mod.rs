use std::collections::HashMap;

use crate::router_only::openrouter::providers::ProviderName;

mod tool_call;
pub use tool_call::{GoogleToolCallExtraContent, ToolCall, ToolCallExtraContent};

use super::manager::Role;
pub use tool_call::FunctionCall;

use serde::{Deserialize, Serialize};
#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct OpenAiResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub choices: Vec<Choices>,
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

/// Response format specification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ResponseFormat {
    #[default]
    Text,
    JsonObject,
}

/// Token usage statistics
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl<'de> Deserialize<'de> for TokenUsage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireTokenUsage {
            #[serde(default)]
            prompt_tokens: u32,
            completion_tokens: Option<u32>,
            #[serde(default)]
            total_tokens: u32,
        }

        let wire = WireTokenUsage::deserialize(deserializer)?;
        let completion_tokens = wire
            .completion_tokens
            .unwrap_or_else(|| wire.total_tokens.saturating_sub(wire.prompt_tokens));

        Ok(Self {
            prompt_tokens: wire.prompt_tokens,
            completion_tokens,
            total_tokens: wire.total_tokens,
        })
    }
}

#[derive(Deserialize, Debug, Copy, Clone, PartialOrd, PartialEq)]
#[allow(dead_code, reason = "Useful later when we handle streaming")]
pub(super) enum ResponseObject {
    #[serde(rename = "chat.completion")]
    ChatCompletion,
    #[serde(rename = "chat.completion.chunk")]
    ChatCompletionChunk,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialOrd, PartialEq)]
#[allow(dead_code, reason = "Useful later when we add token + cost tracking")]
pub(super) struct ResponseUsage {
    /** Including images and tools if any */
    pub(super) prompt_tokens: i64,
    /** The tokens generated */
    pub(super) completion_tokens: i64,
    /** Sum of the above two fields */
    pub(super) total_tokens: i64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Choices {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<ResponseMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
    // For non-streaming choices that might have text instead of message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    // For streaming choices
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<StreamingDelta>,
}

/// Generation completion reasons
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,          // Natural stop sequence
    Length,        // Max tokens reached
    ContentFilter, // Blocked by safety system
    ToolCalls,     // Stopped for tool execution
    Timeout,       // Processing time exceeded
    Error(String), // Error description
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct StreamingDelta {
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
pub struct ErrorResponse {
    pub(super) code: i64,
    pub(super) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    // Contains additional error information such as provider details, the raw error message, etc.
    // Original is Record<string, unknown>
    pub(super) metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ResponseMessage {
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

#[cfg(test)]
mod shape_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_response_serialization_deserialization() {
        let response = OpenAiResponse {
            id: "test-id".to_string(),
            choices: vec![Choices {
                logprobs: None,
                finish_reason: Some(FinishReason::Stop),
                native_finish_reason: Some("stop".to_string()),
                index: Some(0),
                message: Some(ResponseMessage {
                    role: Some(Role::Assistant),
                    content: Some("Hello, world!".to_string()),
                    tool_calls: None,
                    logprobs: None,
                    refusal: None,
                    reasoning: None,
                }),
                error: None,
                text: None,
                delta: None,
            }],
            created: 1234567890,
            model: "gpt-4".to_string(),
            object: "chat.completion".to_string(),
            provider: Some(ProviderName::new("OpenAI")),
            system_fingerprint: Some("test-fingerprint".to_string()),
            usage: Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
            logprobs: None,
        };

        // Test serialization
        let serialized = serde_json::to_string(&response).unwrap();
        let pretty = serde_json::to_string_pretty(&response).unwrap();
        println!("serialized pretty:\n{}", &pretty);

        // Test deserialization
        let deserialized: OpenAiResponse = serde_json::from_str(&serialized).unwrap();

        // Verify fields
        assert_eq!(deserialized.id, response.id);
        assert_eq!(deserialized.choices.len(), response.choices.len());
        assert_eq!(deserialized.created, response.created);
        assert_eq!(deserialized.model, response.model);
        assert_eq!(deserialized.object, response.object);
        assert_eq!(
            deserialized.provider.as_ref().map(ProviderName::as_str),
            Some("OpenAI")
        );
        assert_eq!(deserialized.system_fingerprint, response.system_fingerprint);
        assert_eq!(deserialized.usage.clone().unwrap().prompt_tokens, 10);
        assert_eq!(deserialized.usage.clone().unwrap().completion_tokens, 5);
        assert_eq!(deserialized.usage.clone().unwrap().total_tokens, 15);
    }

    #[test]
    fn test_openai_response_minimal() {
        let json = r#"{
            "id": "minimal",
            "choices": [],
            "created": 0,
            "model": "test-model",
            "object": "chat.completion"
        }"#;

        let response: OpenAiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "minimal");
        assert!(response.choices.is_empty());
        assert_eq!(response.provider.as_ref().map(ProviderName::as_str), None);
        assert_eq!(response.system_fingerprint, None);
        assert!(response.usage.is_none());
    }

    #[test]
    #[ignore = "fixture documents OpenAI tool-call response shape"]
    fn test_openai_response_with_tool_calls() {
        let value = serde_json::json!({
            "id": "chatcmpl-openai-tool-call-fixture",
            "object": "chat.completion",
            "created": 1770000002,
            "model": "gpt-4o-mini-2024-07-18",
            "system_fingerprint": "fp_openai_fixture",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [
                            {
                                "id": "call_request_context_1",
                                "type": "function",
                                "function": {
                                    "name": "request_code_context",
                                    "arguments": "{\"search_term\":\"OpenAiResponse tool_calls\",\"max_results\":3}"
                                }
                            }
                        ]
                    },
                    "finish_reason": "tool_calls"
                }
            ],
            "usage": {
                "prompt_tokens": 41,
                "completion_tokens": 13,
                "total_tokens": 54
            }
        });

        let response: OpenAiResponse =
            serde_json::from_value(value).expect("OpenAI tool-call response parses");

        assert_eq!(response.id, "chatcmpl-openai-tool-call-fixture");
        assert_eq!(response.object, "chat.completion");
        assert_eq!(response.created, 1770000002);
        assert_eq!(response.model, "gpt-4o-mini-2024-07-18");
        assert_eq!(
            response.system_fingerprint.as_deref(),
            Some("fp_openai_fixture")
        );
        assert!(response.provider.is_none());

        let usage = response.usage.as_ref().expect("usage");
        assert_eq!(usage.prompt_tokens, 41);
        assert_eq!(usage.completion_tokens, 13);
        assert_eq!(usage.total_tokens, 54);

        let choice = response.choices.first().expect("one choice");
        assert_eq!(choice.index, Some(0));
        assert_eq!(choice.finish_reason, Some(FinishReason::ToolCalls));
        let message = choice.message.as_ref().expect("assistant message");
        assert_eq!(message.role, Some(Role::Assistant));
        assert_eq!(message.content, None);

        let tool_calls = message.tool_calls.as_ref().expect("tool calls preserved");
        assert_eq!(tool_calls.len(), 1);
        let tool_call = &tool_calls[0];
        assert_eq!(tool_call.call_id.as_ref(), "call_request_context_1");
        assert_eq!(tool_call.call_type, ploke_core::tool_types::FunctionMarker);
        assert_eq!(
            tool_call.function.name,
            ploke_core::tool_types::ToolName::RequestCodeContext
        );
        assert_eq!(
            tool_call.function.arguments,
            "{\"search_term\":\"OpenAiResponse tool_calls\",\"max_results\":3}"
        );

        let serialized = serde_json::to_value(&response).expect("serialize response");
        let serialized_tool_call = &serialized["choices"][0]["message"]["tool_calls"][0];
        assert_eq!(serialized_tool_call["id"], "call_request_context_1");
        assert_eq!(serialized_tool_call["type"], "function");
        assert_eq!(
            serialized_tool_call["function"]["name"],
            "request_code_context"
        );
        assert_eq!(
            serialized_tool_call["function"]["arguments"],
            "{\"search_term\":\"OpenAiResponse tool_calls\",\"max_results\":3}"
        );
    }
}
