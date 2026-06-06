use color_eyre::Result;
use serde_json::json;

use crate::{
    manager::RequestMessage,
    router_only::{
        ChatCompRequest, Router,
        nebius::{Nebius, NebiusChatCompFields, NebiusServiceTier},
    },
};

#[test]
fn nebius_router_uses_token_factory_urls_and_api_key_name() {
    assert_eq!(Nebius::BASE_URL, "https://api.tokenfactory.nebius.com/v1");
    assert_eq!(
        Nebius::COMPLETION_URL,
        "https://api.tokenfactory.nebius.com/v1/chat/completions"
    );
    assert_eq!(
        Nebius::MODELS_URL,
        "https://api.tokenfactory.nebius.com/v1/models"
    );
    assert_eq!(Nebius::API_KEY_NAME, "NEBIUS_API_KEY");
}

#[test]
fn nebius_chat_request_serializes_openai_compatible_body() -> Result<()> {
    let router = NebiusChatCompFields::default()
        .with_store(false)
        .with_max_completion_tokens(100)
        .with_user("diagnostic-user")
        .with_extra_body(json!({"trace": "ploke-skeleton"}))
        .with_service_tier(NebiusServiceTier::Auto);

    let request = ChatCompRequest::<Nebius>::default()
        .with_model_str("meta-llama/Meta-Llama-3.1-70B-Instruct")?
        .with_message(RequestMessage::new_user("Hello!".to_string()))
        .with_max_tokens(100)
        .with_temperature(0.5)
        .with_router_bundle(router);

    let value = serde_json::to_value(&request)?;
    assert_eq!(
        value,
        json!({
            "messages": [
                {
                    "role": "user",
                    "content": "Hello!"
                }
            ],
            "model": "meta-llama/Meta-Llama-3.1-70B-Instruct",
            "max_tokens": 100,
            "temperature": 0.5,
            "store": false,
            "max_completion_tokens": 100,
            "user": "diagnostic-user",
            "extra_body": {
                "trace": "ploke-skeleton"
            },
            "service_tier": "auto"
        })
    );

    let roundtrip: ChatCompRequest<Nebius> = serde_json::from_value(value.clone())?;
    assert_eq!(
        roundtrip.core.model.to_string(),
        "meta-llama/Meta-Llama-3.1-70B-Instruct"
    );
    assert_eq!(roundtrip.router.service_tier, Some(NebiusServiceTier::Auto));
    assert_eq!(serde_json::to_value(roundtrip)?, value);

    Ok(())
}
