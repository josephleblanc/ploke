use color_eyre::Result;
use serde_json::json;

use crate::{
    ReasoningConfig, ReasoningEffort,
    manager::RequestMessage,
    router_only::{
        ChatCompRequest,
        google::{
            ExtraBody, Google, GoogleChatCompFields, GoogleExtraBody, ThinkingConfig, ThinkingLevel,
        },
    },
};

#[test]
fn google_chat_request_serializes_api_model_and_google_extra_body() -> Result<()> {
    let router = GoogleChatCompFields {
        extra_body: Some(
            ExtraBody::default().with_google(
                GoogleExtraBody::default().with_thinking_config(
                    ThinkingConfig::default()
                        .with_thinking_level(ThinkingLevel::Low)
                        .with_include_thoughts(true),
                ),
            ),
        ),
    };

    let request = ChatCompRequest::<Google>::default()
        .with_model_str("google/gemini-2.5-flash")?
        .with_message(RequestMessage::new_user(
            "Reply with one short sentence.".to_string(),
        ))
        .with_max_tokens(64)
        .with_temperature(0.0)
        .with_reasoning(ReasoningConfig::default().with_effort(ReasoningEffort::Low))
        .with_router_bundle(router);

    let value = serde_json::to_value(&request)?;
    assert_eq!(
        value,
        json!({
            "messages": [
                {
                    "role": "user",
                    "content": "Reply with one short sentence."
                }
            ],
            "model": "gemini-2.5-flash",
            "max_tokens": 64,
            "temperature": 0.0,
            "reasoning": {
                "effort": "low"
            },
            "extra_body": {
                "google": {
                    "thinking_config": {
                        "thinking_level": "low",
                        "include_thoughts": true
                    }
                }
            }
        })
    );

    let roundtrip: ChatCompRequest<Google> = serde_json::from_value(value.clone())?;
    assert_eq!(roundtrip.core.model.to_string(), "google/gemini-2.5-flash");
    assert_eq!(serde_json::to_value(roundtrip)?, value);

    Ok(())
}

#[test]
fn google_prompt_request_roundtrips_response_format_stop_and_params() -> Result<()> {
    let request = ChatCompRequest::<Google>::default()
        .with_model_str("google/gemini-2.0-flash")?
        .with_prompt("Return JSON with a single `ok` boolean.".to_string())
        .with_json_response()
        .with_stop(vec!["\n\n".to_string()])
        .with_max_tokens(32)
        .non_streaming();

    let value = serde_json::to_value(&request)?;
    assert_eq!(
        value,
        json!({
            "messages": [],
            "prompt": "Return JSON with a single `ok` boolean.",
            "model": "gemini-2.0-flash",
            "response_format": {
                "type": "json_object"
            },
            "stop": ["\n\n"],
            "stream": false,
            "max_tokens": 32
        })
    );

    let roundtrip: ChatCompRequest<Google> = serde_json::from_value(value.clone())?;
    assert_eq!(roundtrip.core.model.to_string(), "google/gemini-2.0-flash");
    assert_eq!(serde_json::to_value(roundtrip)?, value);

    Ok(())
}
