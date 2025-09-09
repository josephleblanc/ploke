//! Router-specific implementations

pub(crate) mod openrouter;

use itertools::Itertools;
use openrouter::{FallbackMarker, MiddleOutMarker, OpenRouterModelId, Transform};
use serde::{Deserialize, Serialize};

use crate::{llm2::chat_msg::Role, tools::ToolDefinition};

use super::{
    EndpointKey, EndpointsResponse, LLMParameters, ModelId, ModelKey,
    chat_msg::RequestMessage,
    registry::user_prefs::RegistryPrefs,
    request::{
        ChatCompReqCore, JsonObjMarker,
        endpoint::{EndpointData, ToolChoice},
        models,
    },
    types::model_types::ModelVariant,
};
mod anthropic {
    use super::*;
    // Note: Placeholder, just an example for now
    #[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Hash, Eq)]
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

    async fn fetch_models_iter(
        client: &reqwest::Client,
    ) -> color_eyre::Result<impl IntoIterator<Item = Self::Models>> {
        Self::fetch_models(client).await.map(|r| r.into_iter())
    }
}

pub(crate) trait HasEndpoint: Router {
    type EpResponse: for<'a> Deserialize<'a> + Into<EndpointsResponse>;
    type Error;

    async fn fetch_model_endpoints(
        client: &reqwest::Client,
        model: Self::RouterModelId,
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
        let parsed = resp.json::<Self::EpResponse>().await?;

        Ok(parsed)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Hash, Eq)]
pub(crate) enum RouterVariants {
    OpenRouter(openrouter::OpenRouter),
    Anthropic(anthropic::Anthropic),
}

impl Default for RouterVariants {
    fn default() -> Self {
        Self::OpenRouter(openrouter::OpenRouter)
    }
}

pub(crate) trait RouterModelId: From<ModelId> {
    fn into_key(self) -> ModelKey;
    fn key(&self) -> &ModelKey;
    fn into_url_format(self) -> String;
}

pub(crate) trait Router {
    type CompletionFields: ApiRoute;
    type RouterModelId: RouterModelId + From<EndpointKey> + From<ModelId>;
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

    fn endpoints_url(model: Self::RouterModelId) -> String {
        // OpenRouter’s models path treats ':' as a reserved char → percent-encode
        // Use a lightweight escape because only ':' needs it for your case
        let base = model.into_url_format();
        format!("{}/{}/{}", Self::MODELS_URL, base, Self::ENDPOINTS_TAIL)
    }

    fn tranform_endpoint_key(&self, endpoint_key: EndpointKey) -> Self::RouterModelId {
        Self::RouterModelId::from(endpoint_key)
    }

    fn enpoint_to_url(&self, endpoint_key: EndpointKey) -> String {
        let model_id = self.tranform_endpoint_key(endpoint_key);
        Self::endpoints_url(model_id)
    }
}

pub(crate) trait ApiRoute: Sized + Default {
    type Parent: TryFrom<RouterVariants> + Into<RouterVariants> + Default;
    fn parent() -> Self::Parent {
        Self::Parent::default()
    }
    fn router_variant() -> RouterVariants {
        Self::parent().into()
    }
    fn completion_all_fields(
        self,
        core: ChatCompReqCore,
        tools: Option<Vec<ToolDefinition>>,
        llm_params: LLMParameters,
        tool_choice: Option<ToolChoice>,
    ) -> ChatCompRequest<Self> {
        ChatCompRequest::<Self> {
            model_key: Some(core.model.key.clone()),
            core,
            llm_params,
            tools,
            tool_choice,
            router: self,
        }
    }
    fn completion_core(self, core: ChatCompReqCore) -> ChatCompRequest<Self> {
        ChatCompRequest::<Self> {
            model_key: Some(core.model.key),
            router: self,
            ..Default::default()
        }
    }
    fn completion_core_with_params(
        self,
        llm_params: LLMParameters,
        core: ChatCompReqCore,
    ) -> ChatCompRequest<Self> {
        ChatCompRequest::<Self> {
            model_key: Some(core.model.key),
            llm_params,
            router: self,
            ..Default::default()
        }
    }
    fn completion_core_with_tools(
        self,
        core: ChatCompReqCore,
        tools: Option<Vec<ToolDefinition>>,
    ) -> ChatCompRequest<Self> {
        ChatCompRequest::<Self> {
            model_key: Some(core.model.key),
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
use std::{str::FromStr as _, sync::OnceLock};

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
    #[serde(skip, default)]
    pub(crate) model_key: Option<ModelKey>,
    // Trying a new pattern where these are contained in ChatCompReqCore
    //
    // #[serde(default = "default_messages")]
    // pub(crate) messages: Vec<RequestMessage>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) prompt: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) response_format: Option<JsonObjMarker>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) stop: Option<Vec<String>>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) stream: Option<bool>,
    #[serde(flatten)]
    pub(crate) core: ChatCompReqCore,
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

impl<R: ApiRoute> ChatCompRequest<R> {
    pub(crate) fn params_union(mut self, prefs: &RegistryPrefs) -> Self {
        let model_prefs = self.model_key.as_ref().and_then(|m| prefs.models.get(&m));
        if let Some(pref) = model_prefs.and_then(|mp| mp.get_default_profile()) {
            self.llm_params = self.llm_params.merge_some(&pref.params);
        }
        self
    }

    /// Set the messages for the completion request
    pub fn with_messages(mut self, messages: Vec<RequestMessage>) -> Self {
        self.core = self.core.with_messages(messages);
        self
    }

    /// Set a single message for the completion request
    pub fn with_message(mut self, message: RequestMessage) -> Self {
        self.core = self.core.with_message(message);
        self
    }

    /// Set the prompt for the completion request (alternative to messages)
    pub fn with_prompt(mut self, prompt: String) -> Self {
        self.core = self.core.with_prompt(prompt);
        self
    }

    /// Set the model for the completion request
    pub fn with_model(mut self, model: ModelId) -> Self {
        self.core = self.core.with_model(model);
        self
    }

    /// Set the model by string (parses into ModelId)
    pub fn with_model_str(self, model_str: &str) -> Result<Self, crate::llm2::IdError> {
        let model = ModelId::from_str(model_str)?;
        Ok(self.with_model(model))
    }

    /// Set the response format to JSON object
    pub fn with_json_response(mut self) -> Self {
        self.core = self.core.with_json_response();
        self
    }

    /// Set the stop sequences
    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.core = self.core.with_stop(stop);
        self
    }

    /// Add a single stop sequence
    pub fn with_stop_sequence(mut self, stop: String) -> Self {
        self.core = self.core.with_stop_sequence(stop);
        self
    }

    /// Enable or disable streaming
    pub fn with_streaming(mut self, stream: bool) -> Self {
        self.core = self.core.with_streaming(stream);
        self
    }

    /// Enable streaming (convenience method)
    pub fn streaming(self) -> Self {
        self.with_streaming(true)
    }

    /// Disable streaming (convenience method)
    pub fn non_streaming(self) -> Self {
        self.with_streaming(false)
    }

    /// Set max tokens parameter - Range: [1, context_length)
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.llm_params = self.llm_params.with_max_tokens(max_tokens);
        self
    }

    /// Set temperature parameter - Range: [0, 2]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.llm_params = self.llm_params.with_temperature(temperature);
        self
    }

    /// Set seed parameter - Integer only
    pub fn with_seed(mut self, seed: i64) -> Self {
        self.llm_params = self.llm_params.with_seed(seed);
        self
    }

    /// Set top_p parameter - Range: (0, 1]
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.llm_params = self.llm_params.with_top_p(top_p);
        self
    }

    /// Set top_k parameter - Range: [1, Infinity) Not available for OpenAI models
    pub fn with_top_k(mut self, top_k: f32) -> Self {
        self.llm_params = self.llm_params.with_top_k(top_k);
        self
    }

    /// Set frequency_penalty parameter - Range: [-2, 2]
    pub fn with_frequency_penalty(mut self, frequency_penalty: f32) -> Self {
        self.llm_params = self.llm_params.with_frequency_penalty(frequency_penalty);
        self
    }

    /// Set presence_penalty parameter - Range: [-2, 2]
    pub fn with_presence_penalty(mut self, presence_penalty: f32) -> Self {
        self.llm_params = self.llm_params.with_presence_penalty(presence_penalty);
        self
    }

    /// Set repetition_penalty parameter - Range: (0, 2]
    pub fn with_repetition_penalty(mut self, repetition_penalty: f32) -> Self {
        self.llm_params = self.llm_params.with_repetition_penalty(repetition_penalty);
        self
    }

    /// Set logit_bias parameter - { [key: number]: number }
    pub fn with_logit_bias(mut self, logit_bias: std::collections::BTreeMap<i32, f32>) -> Self {
        self.llm_params = self.llm_params.with_logit_bias(logit_bias);
        self
    }

    /// Set top_logprobs parameter - Integer only
    pub fn with_top_logprobs(mut self, top_logprobs: i32) -> Self {
        self.llm_params = self.llm_params.with_top_logprobs(top_logprobs);
        self
    }

    /// Set min_p parameter - Range: [0, 1]
    pub fn with_min_p(mut self, min_p: f32) -> Self {
        self.llm_params = self.llm_params.with_min_p(min_p);
        self
    }

    /// Set top_a parameter - Range: [0, 1]
    pub fn with_top_a(mut self, top_a: f32) -> Self {
        self.llm_params = self.llm_params.with_top_a(top_a);
        self
    }
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
        use crate::llm2::ModelId;
        use openrouter::OpenRouterModelId;
        use std::path::PathBuf;

        use ploke_test_utils::workspace_root;

        use crate::llm2::chat_msg::Role;

        let model_id = OpenRouterModelId::from_str("qwen/qwen3-30b-a3b-thinking-2507")?;
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
            router: openrouter::ChatCompFields::default(),
            core: ChatCompReqCore {
                messages: vec![msg],
                ..Default::default()
            },
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
