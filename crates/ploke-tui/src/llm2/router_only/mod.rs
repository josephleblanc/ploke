//! Router-specific implementations

pub(crate) mod openrouter;

use itertools::Itertools;
use openrouter::{FallbackMarker, MiddleOutMarker, OpenRouterModelId, Transform};
use serde::{Deserialize, Serialize};

use crate::{llm2::chat_msg::Role, tools::ToolDefinition};

use super::{
    chat_msg::RequestMessage, request::{
        endpoint::{EndpointData, ToolChoice}, models, ChatCompReqCore, JsonObjMarker
    }, EndpointKey, EndpointsResponse, LLMParameters, ModelId, ModelKey
};
mod anthropic {
    use super::*;
    // Note: Placeholder, just an example for now
    #[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
    pub(crate) struct Anthropic;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    // Note: Placeholder, just an example for now
    pub(crate) struct ChatCompFields {
        claude_specific_one: Option<String>,
        claude_specific_two: Option<String>,
    }
}

pub(crate) trait HasModelId {
    fn model_id(&self) -> ModelId;
} 

pub(crate) trait HasModels: Router {
    // Models response
    type Response: for<'a> Deserialize<'a> + IntoIterator<Item = Self::Models>;
    type Models: for<'a> Deserialize<'a> + HasModelId + Into<models::ResponseItem>;
    type Error;

    async fn fetch_models(client: &reqwest::Client) -> color_eyre::Result<Self::Response> {
        let url = Self::MODELS_URL;
        let api_key = Self::resolve_api_key()?;

        let resp = client
            .get(url)
            .bearer_auth(api_key)
            .header("Accept", "application/json")
            .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
            .header("X-Title", "Ploke TUI")
            .send()
            .await?
            .error_for_status()?;

        let parsed = resp.json::<Self::Response>().await?;

        Ok(parsed)
    }

    async fn fetch_models_iter(client: &reqwest::Client) -> color_eyre::Result<impl IntoIterator<Item = Self::Models>> {
        Self::fetch_models(client).await
            .map(|r| r.into_iter())

    }
}

pub(crate) trait HasEndpoint: Router {
    type EpResponse: for<'a> Deserialize<'a> + Into<EndpointsResponse>;
    type Error;

    async fn fetch_model_endpoints(
        client: &reqwest::Client,
        model: Self::ModelId,
    ) -> color_eyre::Result<Self::EpResponse> {
        let url = Self::endpoints_url(model);
        let api_key = Self::resolve_api_key()?;

        let resp = client
            .get(url)
            .bearer_auth(api_key)
            .header("Accept", "application/json")
            .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
            .header("X-Title", "Ploke TUI")
            .send()
            .await?
            .error_for_status()?;

        // let body = resp.json().await?;
        // Parse with stronger typed endpoint shape first
        let parsed  = resp.json::<Self::EpResponse>().await?;

        Ok(parsed)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub(crate) enum RouterVariants {
    OpenRouter(openrouter::OpenRouter),
    Anthropic(anthropic::Anthropic),
}

pub(crate) trait RouterModelId {
    fn into_key(self) -> ModelKey;
    fn key(&self) -> &ModelKey;
    fn into_url_format(self) -> String;
}

pub(crate) trait Router {
    type CompletionFields: ApiRoute;
    type ModelId: RouterModelId + From<EndpointKey> + From<ModelId>;
    const BASE_URL: &str;
    const COMPLETION_URL: &str;
    const MODELS_URL: &str;
    const ENDPOINTS_TAIL: &str;
    const API_KEY_NAME: &str;
    const PROVIDERS_URL: &str;

    fn resolve_api_key() -> Result<String, std::env::VarError> {
        // 1. Check provider-specific env var if specified
        let key_name = Self::API_KEY_NAME;
        std::env::var(key_name)
    }

    fn endpoints_url(model: Self::ModelId) -> String {
        // OpenRouter’s models path treats ':' as a reserved char → percent-encode
        // Use a lightweight escape because only ':' needs it for your case
        let base = model.into_url_format();
        format!("{}/{}/{}", Self::MODELS_URL, base, Self::ENDPOINTS_TAIL)
    }

    fn tranform_endpoint_key(&self, endpoint_key: EndpointKey) -> Self::ModelId {
        Self::ModelId::from(endpoint_key)
    }

    fn enpoint_to_url(&self, endpoint_key: EndpointKey) -> String {
        let model_id = self.tranform_endpoint_key(endpoint_key);
        Self::endpoints_url(model_id)
    }
}

pub(crate) trait ApiRoute: Sized + Default {
    fn completion_all_fields(
        self,
        req: ChatCompReqCore,
        tools: Option<Vec<ToolDefinition>>,
        tool_choice: Option<ToolChoice>,
    ) -> ChatCompRequest<Self> {
        ChatCompRequest::<Self> {
            model: req.model,
            messages: req.messages,
            prompt: req.prompt,
            response_format: req.response_format,
            stop: req.stop,
            stream: req.stream,
            llm_params: req.llm_params,
            tools,
            tool_choice,
            router: self,
        }
    }
    fn completion_core(self, req: ChatCompReqCore) -> ChatCompRequest<Self> {
        ChatCompRequest::<Self> {
            messages: req.messages,
            prompt: req.prompt,
            model: req.model,
            response_format: req.response_format,
            stop: req.stop,
            stream: req.stream,
            llm_params: req.llm_params,
            router: self,
            ..Default::default()
        }
    }
    fn completion_core_with_tools(self, req: ChatCompReqCore) -> ChatCompRequest<Self> {
        ChatCompRequest::<Self> {
            messages: req.messages,
            prompt: req.prompt,
            model: req.model,
            response_format: req.response_format,
            stop: req.stop,
            stream: req.stream,
            llm_params: req.llm_params,
            router: self,
            ..Default::default()
        }
    }
    fn default_chat_completion() -> ChatCompRequest<Self> {
        ChatCompRequest::<Self> {
            router: Self::default(),
            ..Default::default()
        }
    }
}
use serde_json::{Value, json};
use std::sync::OnceLock;

static DEFAULT_MODEL: OnceLock<String> = OnceLock::new();
pub(crate) fn default_model() -> String {
    DEFAULT_MODEL
        .get_or_init(|| "moonshotai/kimi-k2".to_string())
        .clone()
}

static DEFAULT_MESSAGE: OnceLock<Vec<RequestMessage>> = OnceLock::new();
pub(crate) fn default_messages() -> Vec<RequestMessage> {
    DEFAULT_MESSAGE
        .get_or_init(|| {
            vec![RequestMessage {
                role: Role::System,
                content: "You are a helpful assistant, please help the user with their requests."
                    .to_string(),
                tool_call_id: None,
            }]
        })
        .clone()
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub(crate) struct ChatCompRequest<R: ApiRoute> {
    #[serde(default = "default_model")]
    pub(crate) model: String,
    #[serde(default = "default_messages")]
    pub(crate) messages: Vec<RequestMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) response_format: Option<JsonObjMarker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stream: Option<bool>,

    #[serde(flatten)]
    pub(crate) llm_params: LLMParameters,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_choice: Option<ToolChoice>,

    // Router-specific fields merged at the top level
    #[serde(flatten)]
    pub(crate) router: R,
}

#[cfg(test)]
pub(crate) const MODELS_JSON_RAW: &str = "crates/ploke-tui/data/models/all_raw.json";
pub(crate) const ENDPOINTS_JSON_DIR: &str = "crates/ploke-tui/data/endpoints/";
pub(crate) const COMPLETION_JSON_SIMPLE_DIR: &str = "crates/ploke-tui/data/chat_completions/";

#[cfg(test)]
mod tests {
    use crate::llm2::{chat_msg::Role, router_only::openrouter::OpenRouter};
    use std::time::Duration;

    use crate::{llm2::ModelId, llm2::error::LlmError};
    use std::{path::PathBuf, str::FromStr as _};

    use super::*;
    #[test]
    fn show_openrouter_json2() {
        let req = ChatCompRequest::<openrouter::ChatCompFields> {
            messages: Default::default(),
            router: openrouter::ChatCompFields::default()
                .with_route(FallbackMarker)
                .with_transforms(Transform::MiddleOut([MiddleOutMarker])),
            ..Default::default()
        };
        let j = serde_json::to_string_pretty(&req).unwrap();
        println!("{j}");
    }

    use color_eyre::Result;
    use reqwest::Client;
    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn test_simple_query_models() -> Result<()> {
        use ploke_test_utils::workspace_root;

        let url = OpenRouter::MODELS_URL;
        let key = OpenRouter::resolve_api_key()?;

        let response = Client::new()
            .get(url)
            .bearer_auth(key)
            .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;

        let response_json = response.text().await?;

        if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
            let mut dir = workspace_root();
            dir.push(MODELS_JSON_RAW);
            println!("Writing '/models' raw response to:\n{}", dir.display());
            std::fs::write(dir, response_json)?;
        }
        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn test_default_query_endpoints() -> Result<()> {
        use std::{path::PathBuf, str::FromStr as _};

        use ploke_test_utils::workspace_root;

        // TODO: we need to handle more items like the below, which the `ModelKey` doesn't
        // currently handle:
        // - nousresearch/deephermes-3-llama-3-8b-preview:free
        // - Should turn into a raw curl request like:
        //  https://openrouter.ai/api/v1/models/nousresearch/deephermes-3-llama-3-8b-preview%3Afree/endpoints

        let model_key = OpenRouterModelId::from_str("qwen/qwen3-30b-a3b")?;
        let url = OpenRouter::endpoints_url(model_key);
        eprintln!(
            "Constructed url to query `/:author/:model/endpoints at\n{}",
            url
        );
        assert_eq!(
            "https://openrouter.ai/api/v1/models/qwen/qwen3-30b-a3b/endpoints",
            url
        );
        let key = OpenRouter::resolve_api_key()?;
        let mut dir = workspace_root();
        dir.push(ENDPOINTS_JSON_DIR);

        let response = Client::new()
            .get(url)
            .bearer_auth(key)
            .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;

        let is_success = response.status().is_success();
        eprintln!("is_success: {}", is_success);
        eprintln!("status: {}", response.status());
        let response_text = response.text().await?;

        let response_value: serde_json::Value = serde_json::from_str(&response_text)?;

        if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
            let response_raw_pretty = serde_json::to_string_pretty(&response_value)?;
            std::fs::create_dir_all(&dir)?;
            dir.push("endpoints.json");
            println!("Writing raw json reponse to: {}", dir.display());
            std::fs::write(dir, response_raw_pretty)?;
        }
        assert!(is_success);

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn test_free_query_endpoints() -> Result<()> {
        use ploke_test_utils::workspace_root;

        use crate::llm2::chat_msg::Role;

        let model_key =
            OpenRouterModelId::from_str("nousresearch/deephermes-3-llama-3-8b-preview:free")?;
        let url = OpenRouter::endpoints_url(model_key);
        eprintln!(
            "Constructed url to query `/:author/:model/endpoints at\n{}",
            url
        );
        assert_eq!(
            "https://openrouter.ai/api/v1/models/nousresearch/deephermes-3-llama-3-8b-preview:free/endpoints",
            url
        );
        let key = OpenRouter::resolve_api_key()?;
        let mut dir = workspace_root();
        dir.push(ENDPOINTS_JSON_DIR);

        let response = Client::new()
            .get(url)
            .bearer_auth(key)
            .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;

        let is_success = response.status().is_success();
        eprintln!("is_success: {}", is_success);
        eprintln!("status: {}", response.status());
        let response_text = response.text().await?;

        let response_value: serde_json::Value = serde_json::from_str(&response_text)?;

        if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
            let response_raw_pretty = serde_json::to_string_pretty(&response_value)?;
            std::fs::create_dir_all(&dir)?;
            dir.push("free_model.json");
            println!("Writing raw json reponse to: {}", dir.display());
            std::fs::write(dir, response_raw_pretty)?;
        }
        assert!(is_success);

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn test_default_post_completions() -> Result<()> {
        use std::path::PathBuf;

        use ploke_test_utils::workspace_root;

        use crate::llm2::chat_msg::Role;

        let model_key = OpenRouterModelId::from_str("qwen/qwen3-30b-a3b-thinking-2507")?;
        let key = OpenRouter::resolve_api_key()?;
        let url = OpenRouter::COMPLETION_URL;
        let mut dir = workspace_root();
        dir.push(COMPLETION_JSON_SIMPLE_DIR);

        let content = String::from("Hello, can you tell me about lifetimes in Rust?");
        let msg = RequestMessage {
            role: Role::User,
            content,
            tool_call_id: None,
        };

        let req = ChatCompRequest::<openrouter::ChatCompFields> {
            messages: vec![msg],
            model: model_key.to_string(),
            router: openrouter::ChatCompFields::default(),
            ..Default::default()
        };

        if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
            let pretty = serde_json::to_string_pretty(&req)?;
            dir.push("request_se.json");
            println!("Writing serialized request to: {}", dir.display());
            std::fs::write(&dir, pretty)?;
        }

        let response = Client::new()
            .post(url)
            .bearer_auth(key)
            .json(&req)
            .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;
        let is_success = response.status().is_success();
        eprintln!("is_success: {}", is_success);
        eprintln!("status: {}", response.status());

        let response_text = response.text().await?;

        let response_value: serde_json::Value = serde_json::from_str(&response_text)?;

        if std::env::var("WRITE_MODE").unwrap_or_default() == "1" {
            let response_raw_pretty = serde_json::to_string_pretty(&response_value)?;
            dir.pop();
            dir.push("response_raw.json");
            println!("Writing raw json reponse to: {}", dir.display());
            std::fs::write(dir, response_raw_pretty)?;
        }
        assert!(is_success);
        assert!(response_text.contains("help"));

        Ok(())
    }
}
