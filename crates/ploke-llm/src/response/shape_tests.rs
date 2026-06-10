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
fn google_malformed_function_call_finish_reason_deserializes() {
    let value = json!({
        "id": "chatcmpl-google-malformed",
        "object": "chat.completion",
        "created": 1770000000,
        "model": "google/gemini-2.5-flash",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "refusal": "Malformed function call: print(default_api.apply_code_edit(edits=[...]))"
                },
                "finish_reason": "malformed_function_call"
            }
        ],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 10,
            "total_tokens": 110
        }
    });

    let response: OpenAiResponse =
        serde_json::from_value(value).expect("google malformed_function_call response");
    let choice = &response.choices[0];
    assert_eq!(
        choice.finish_reason,
        Some(FinishReason::MalformedFunctionCall)
    );
    let message = choice.message.as_ref().expect("assistant message");
    assert!(
        message
            .refusal
            .as_deref()
            .is_some_and(|refusal| refusal.contains("default_api.apply_code_edit"))
    );
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
fn google_empty_visible_content_response_derives_completion_tokens() {
    let value = json!({
        "choices": [
            {
                "finish_reason": "stop",
                "index": 0,
                "logprobs": null,
                "message": {
                    "content": "",
                    "role": "assistant"
                }
            }
        ],
        "created": 1779382162,
        "id": "kjcPat6OHo6kq8YPg6CZ2AM",
        "model": "google/gemini-2.5-flash",
        "object": "chat.completion",
        "system_fingerprint": "",
        "usage": {
            "completion_tokens_details": {
                "reasoning_tokens": 295
            },
            "extra_properties": {
                "google": {
                    "traffic_type": "ON_DEMAND"
                }
            },
            "prompt_tokens": 7912,
            "total_tokens": 8207
        }
    });

    let response: OpenAiResponse =
        serde_json::from_value(value).expect("google empty-content response");
    let message = response.choices[0]
        .message
        .as_ref()
        .expect("assistant message");
    assert_eq!(message.content.as_deref(), Some(""));

    let usage = response.usage.expect("usage");
    assert_eq!(usage.prompt_tokens, 7912);
    assert_eq!(usage.completion_tokens, 295);
    assert_eq!(usage.total_tokens, 8207);
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
