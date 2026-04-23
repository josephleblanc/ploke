//! Tests for LLM response deserialization edge cases
//!
//! This module tests the qwen/qwen3.6-plus reasoning-only response bug.
//!
//! The fixture `qwen_reasoning_no_content_response.json` is a real captured
//! LLM response from a live run that failed with `RESPONSE_DESERIALIZATION_FAILED`.
//! It contains only a `reasoning` field without a `content` field, which is
//! non-standard but occurs with some qwen model versions.

use ploke_llm::manager::parse_chat_outcome;
use ploke_llm::response::OpenAiResponse;

#[cfg(feature = "qwen_reasoning_fix")]
use ploke_llm::manager::ChatStepOutcome;

#[cfg(not(feature = "qwen_reasoning_fix"))]
use ploke_llm::LlmError;

/// Fixture: qwen/qwen3.6-plus response with reasoning but no content field
///
/// This is a real captured response from the live run at:
/// ~/.ploke-eval/instances/BurntSushi__ripgrep-2209/
///
/// The model returned reasoning without content, causing deserialization to fail
/// with "No usable choice" before the qwen_reasoning_fix feature was implemented.
const QWEN_REASONING_NO_CONTENT_JSON: &str =
    include_str!("fixtures/qwen_reasoning_no_content_response.json");

/// DIAGNOSTIC TEST (pre-fix): Verifies qwen reasoning-only responses fail deserialization
///
/// This test documents the exact failure mode. It passes when the bug exists
/// (feature disabled), fails after the fix (feature enabled).
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
