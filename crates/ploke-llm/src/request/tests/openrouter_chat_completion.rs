use color_eyre::Result;
use serde_json::json;

use crate::{
    ReasoningConfig, ReasoningEffort,
    manager::RequestMessage,
    request::endpoint::FallbackMarker,
    router_only::{
        ChatCompRequest,
        openrouter::{ChatCompFields, MiddleOutMarker, OpenRouter, Transform},
    },
};

#[test]
fn openrouter_chat_request_keeps_canonical_model_and_router_fields() -> Result<()> {
    let router = ChatCompFields::default()
        .with_route(FallbackMarker)
        .with_transforms(Transform::MiddleOut([MiddleOutMarker]))
        .with_user("diagnostic-user".to_string());

    let request = ChatCompRequest::<OpenRouter>::default()
        .with_model_str("qwen/qwen3-30b-a3b:free")?
        .with_messages(vec![
            RequestMessage::new_system("You are concise.".to_string()),
            RequestMessage::new_user("Say ok.".to_string()),
        ])
        .with_max_tokens(16)
        .with_reasoning(ReasoningConfig::default().with_effort(ReasoningEffort::None))
        .with_router_bundle(router);

    let value = serde_json::to_value(&request)?;
    assert_eq!(
        value,
        json!({
            "messages": [
                {
                    "role": "system",
                    "content": "You are concise."
                },
                {
                    "role": "user",
                    "content": "Say ok."
                }
            ],
            "model": "qwen/qwen3-30b-a3b:free",
            "max_tokens": 16,
            "reasoning": {
                "effort": "none"
            },
            "transforms": ["middle-out"],
            "route": "fallback",
            "user": "diagnostic-user"
        })
    );

    let roundtrip: ChatCompRequest<OpenRouter> = serde_json::from_value(value.clone())?;
    assert_eq!(roundtrip.core.model.to_string(), "qwen/qwen3-30b-a3b:free");
    assert_eq!(serde_json::to_value(roundtrip)?, value);

    Ok(())
}
