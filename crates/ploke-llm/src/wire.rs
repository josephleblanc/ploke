#![allow(
    dead_code,
    unused_variables,
    reason = "evolving api surface, may be useful, written 2025-12-15"
)]

use url::Url;

use super::manager::RequestMessage;
use super::router_only::openrouter::OpenRouterModelId;
use super::router_only::{ChatCompRequest, openrouter::OpenRouter};
use super::*;

#[derive(Clone, Debug)]
pub struct WireRequest {
    pub url: Url,
    pub authorization_env: ApiKeyEnv,
    pub body: serde_json::Value,
    pub content_type: &'static str, // "application/json"
}

pub fn build_openrouter_request(
    kimi: &OpenRouterModelId,
    prov: &ProviderConfig, // must be Transport::OpenRouter
    messages: Vec<RequestMessage>,
    prompt: Option<String>,
    llm_params: &LLMParameters,
) -> Result<WireRequest, IdError> {
    let Transport::OpenRouter {
        base,
        allow,
        api_key_env,
    } = &prov.transport
    else {
        return Err(IdError::Invalid("expected OpenRouter transport"));
    };

    // Build your existing CompReq<'a>
    let comp = ChatCompRequest::<OpenRouter> {
        // route only within this model; also set provider allowlist via ProviderPreferences
        // map parameters
        llm_params: llm_params.clone(),
        // ...fill other fields you use...
        ..Default::default()
    };

    let url = base
        .as_url()
        .join("chat/completions")
        .map_err(|_| IdError::Invalid("openrouter path"))?;
    let body = serde_json::to_value(comp).expect("CompReq to JSON");

    Ok(WireRequest {
        url,
        authorization_env: api_key_env.clone(),
        body,
        content_type: "application/json",
    })
}

pub fn build_direct_oai_request(
    kimi: &ModelKey,
    prov: &ProviderConfig, // must be Transport::DirectOAI
    messages: Vec<RequestMessage>,
    params: &LLMParameters,
) -> Result<WireRequest, IdError> {
    // TODO: We need to read the actual OpenAI spec for a specific structure on those endpoints.
    // Not urgent, get to it after we have the OpenRouter version working.
    // - Keep stub for now
    todo!()
}
