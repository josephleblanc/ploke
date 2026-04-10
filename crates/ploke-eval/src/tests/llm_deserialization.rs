//! Tests for LLM response deserialization edge cases

use ploke_llm::response::OpenAiResponse;
use ploke_llm::manager::{parse_chat_outcome, ChatStepOutcome};
use ploke_llm::LlmError;

/// Fixture: qwen/qwen3.6-plus response with reasoning but no content field
const QWEN_REASONING_NO_CONTENT_JSON: &str = include_str!(
    "fixtures/qwen_reasoning_no_content_response.json"
);

/// DIAGNOSTIC TEST (pre-fix): Verifies qwen reasoning-only responses fail deserialization
/// 
/// This test documents the exact failure mode. It passes when the bug exists,
/// fails after the fix. Marked #[ignore] after fix lands, then removed.
#[test]
#[cfg(not(feature = "qwen_reasoning_fix"))]
fn test_qwen_reasoning_only_fails_deserialization() {
    let response: OpenAiResponse = serde_json::from_str(QWEN_REASONING_NO_CONTENT_JSON)
        .expect("Raw JSON should deserialize to OpenAiResponse");

    // Verify the response structure matches what qwen returns
    assert_eq!(response.model, "qwen/qwen3.6-plus-04-02");
    assert_eq!(response.choices.len(), 1);
    
    // Parse via public API - this should fail with the current code
    let result = parse_chat_outcome(QWEN_REASONING_NO_CONTENT_JSON);
    
    match result {
        Err(LlmError::Deserialization { message, .. }) => {
            assert!(
                message.contains("No usable choice"),
                "Expected 'No usable choice' error, got: {message}"
            );
        }
        Ok(_) => panic!("Expected deserialization failure, but parsing succeeded"),
        Err(other) => panic!("Expected Deserialization error, got: {other:?}"),
    }
}

/// REGRESSION TEST (post-fix): Verifies qwen reasoning-only responses work correctly
/// 
/// This test passes after the fix. It ensures reasoning-only responses are handled
/// gracefully by coalescing reasoning to content.
#[test]
#[cfg(feature = "qwen_reasoning_fix")]
fn test_qwen_reasoning_only_coalesces_to_content() {
    let response: OpenAiResponse = serde_json::from_str(QWEN_REASONING_NO_CONTENT_JSON)
        .expect("Raw JSON should deserialize to OpenAiResponse");

    assert_eq!(response.model, "qwen/qwen3.6-plus-04-02");
    assert_eq!(response.choices.len(), 1);
    
    // Parse via public API - this should succeed after the fix
    let result = parse_chat_outcome(QWEN_REASONING_NO_CONTENT_JSON);
    
    match result {
        Ok(chat_step) => {
            match chat_step.outcome {
                ChatStepOutcome::Content { content, .. } => {
                    // The reasoning should be coalesced to content
                    assert!(
                        content.is_some(),
                        "Content should be Some after coalescing reasoning"
                    );
                    let content_text = content.unwrap();
                    assert!(
                        content_text.contains("replace_with_captures_at"),
                        "Content should contain the reasoning text, got: {content_text}"
                    );
                }
                other => panic!("Expected Content outcome, got: {other:?}"),
            }
        }
        Err(e) => panic!("Expected successful parsing after fix, got error: {e}"),
    }
}

/// Test that normal responses (with content) still work correctly
#[test]
fn test_normal_response_with_content_parses_correctly() {
    let normal_response = r#"{
        "id": "test-normal",
        "object": "chat.completion",
        "created": 1234567890,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": {
                "role": "assistant",
                "content": "This is the response content."
            }
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    }"#;

    let response: OpenAiResponse = serde_json::from_str(normal_response)
        .expect("Normal response should deserialize");
    
    assert_eq!(response.choices.len(), 1);
    
    // Parse and verify outcome
    let result = parse_chat_outcome(normal_response).expect("Should parse successfully");
    match result.outcome {
        ChatStepOutcome::Content { content, .. } => {
            assert_eq!(content.as_ref().map(|s| s.as_ref()), Some("This is the response content."));
        }
        other => panic!("Expected Content outcome, got: {other:?}"),
    }
}

/// Test that responses with tool calls work correctly (not affected by fix)
#[test]
fn test_response_with_tool_calls_parses_correctly() {
    let response_json = r#"{
        "id": "test-tool",
        "object": "chat.completion",
        "created": 1234567890,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "finish_reason": "tool_calls",
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_123",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"file\": \"/path/to/file.rs\"}"
                    }
                }]
            }
        }],
        "usage": {
            "prompt_tokens": 50,
            "completion_tokens": 25,
            "total_tokens": 75
        }
    }"#;

    let result = parse_chat_outcome(response_json).expect("Should parse successfully");
    match result.outcome {
        ChatStepOutcome::ToolCalls { calls, .. } => {
            assert_eq!(calls.len(), 1, "Expected exactly one tool call");
            // ToolName is an enum in ploke-core, not a string
        }
        other => panic!("Expected ToolCalls outcome, got: {other:?}"),
    }
}
