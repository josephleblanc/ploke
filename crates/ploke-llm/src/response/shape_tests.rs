use serde::Deserialize;
use serde_json::json;

use super::*;

#[test]
fn google_success_response_deserializes_as_openai_response_shape() {
    let value = json!({
        "id": "chatcmpl-google-diagnostic",
        "object": "chat.completion",
        "created": 1770000000,
        "model": "gemini-2.5-flash",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "ok"
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 8,
            "completion_tokens": 1,
            "total_tokens": 9
        }
    });

    let response: OpenAiResponse = serde_json::from_value(value.clone()).expect("google response");
    assert_eq!(response.id, "chatcmpl-google-diagnostic");
    assert_eq!(response.model, "gemini-2.5-flash");
    assert_eq!(response.choices.len(), 1);

    let choice = &response.choices[0];
    assert_eq!(choice.finish_reason, Some(FinishReason::Stop));
    let message = choice.message.as_ref().expect("assistant message");
    assert_eq!(message.role, Some(Role::Assistant));
    assert_eq!(message.content.as_deref(), Some("ok"));
    assert_eq!(response.usage.expect("usage").total_tokens, 9);

    let serialized = serde_json::to_value(response).expect("serialize response");
    assert_eq!(serialized["choices"][0]["message"]["content"], "ok");
}

#[test]
fn google_resource_exhausted_error_shape_is_diagnostic_array() {
    #[derive(Debug, Deserialize)]
    struct GoogleErrorList(Vec<GoogleErrorEnvelope>);

    #[derive(Debug, Deserialize)]
    struct GoogleErrorEnvelope {
        error: GoogleError,
    }

    #[derive(Debug, Deserialize)]
    struct GoogleError {
        code: u16,
        message: String,
        status: String,
    }

    let value = json!([
        {
            "error": {
                "code": 429,
                "message": "Your prepayment credits are depleted.",
                "status": "RESOURCE_EXHAUSTED"
            }
        }
    ]);

    let errors: GoogleErrorList = serde_json::from_value(value).expect("google error array");
    let error = &errors.0[0].error;
    assert_eq!(error.code, 429);
    assert_eq!(error.status, "RESOURCE_EXHAUSTED");
    assert!(error.message.contains("prepayment credits"));
}

#[test]
fn openrouter_reasoning_response_deserializes_message_reasoning() {
    let value = json!({
        "id": "gen-openrouter-diagnostic",
        "object": "chat.completion",
        "created": 1770000001,
        "model": "qwen/qwen3-30b-a3b:free",
        "provider": "OpenRouter",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "ok",
                    "reasoning": "The user asked for a terse confirmation."
                },
                "finish_reason": "stop",
                "native_finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 12,
            "completion_tokens": 3,
            "total_tokens": 15
        }
    });

    let response: OpenAiResponse =
        serde_json::from_value(value.clone()).expect("openrouter response");
    assert_eq!(
        response.provider.as_ref().map(|provider| provider.as_str()),
        Some("OpenRouter")
    );

    let message = response.choices[0]
        .message
        .as_ref()
        .expect("assistant message");
    assert_eq!(message.content.as_deref(), Some("ok"));
    assert_eq!(
        message.reasoning.as_deref(),
        Some("The user asked for a terse confirmation.")
    );

    let serialized = serde_json::to_value(response).expect("serialize response");
    assert_eq!(
        serialized["choices"][0]["message"]["reasoning"],
        "The user asked for a terse confirmation."
    );
}
