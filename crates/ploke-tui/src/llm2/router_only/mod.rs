//! Router-specific implementations

pub(crate) mod openrouter;

use itertools::Itertools;
use openrouter::{FallbackMarker, MiddleOutMarker, Transform};
use serde::{Deserialize, Serialize};

use crate::{llm2::chat_msg::Role, tools::ToolDefinition};

use super::{
    LLMParameters, ModelKey,
    chat_msg::RequestMessage,
    request::{ChatCompReqCore, JsonObjMarker, endpoint::ToolChoice},
};
mod anthropic {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    // Note: Placeholder, just an example for now
    pub(crate) struct ChatCompFields {
        claude_specific_one: Option<String>,
        claude_specific_two: Option<String>,
    }
}

pub(crate) trait Router {
    type CompletionFields: ApiRoute;
    const BASE_URL: &str;
    const COMPLETION_URL: &str;
    const MODELS_URL: &str;
    const ENDPOINTS_TAIL: &str;
    const API_KEY_NAME: &str;

    fn resolve_api_key() -> Result<String, std::env::VarError> {
        // 1. Check provider-specific env var if specified
        let key_name = Self::API_KEY_NAME;
        std::env::var(key_name)
    }

    fn endpoints_url_string(model_key: ModelKey) -> String {
        // NOTE: This uses MODELS_URL for the base part, not the simple base
        [
            Self::MODELS_URL,
            model_key.id.as_str(),
            Self::ENDPOINTS_TAIL,
        ]
        .into_iter()
        .join("/")
    }
}

#[derive(Default, Copy, Clone, Debug)]
pub(crate) struct OpenRouter;

impl Router for OpenRouter {
    type CompletionFields = openrouter::ChatCompFields;
    const BASE_URL: &str = "https://openrouter.ai/api/v1";
    const COMPLETION_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
    const MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
    const ENDPOINTS_TAIL: &str = "endpoints";
    const API_KEY_NAME: &str = "OPENROUTER_API_KEY";
}

impl ApiRoute for openrouter::ChatCompFields {}

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
                content: "You are a helpful assistant, please help the user with their requests.".to_string(),
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

    // ⬇️ Router-specific fields merged at the top level
    #[serde(flatten)]
    pub(crate) router: R,
}

#[cfg(test)]
pub(crate) const MODELS_JSON_RAW: &str = "crates/ploke-tui/data/models/all_raw.json";
pub(crate) const ENDPOINTS_JSON_DIR: &str = "crates/ploke-tui/data/endpoints/";
pub(crate) const COMPLETION_JSON_SIMPLE_DIR: &str = "crates/ploke-tui/data/chat_completions/";

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{llm::LlmError, llm2::newtypes::ModelId};

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
        use std::path::PathBuf;

        use ploke_test_utils::workspace_root;

        use crate::llm2::chat_msg::Role;

        // TODO: we need to handle more items like the below, which the `ModelKey` doesn't
        // currently handle:
        // - nousresearch/deephermes-3-llama-3-8b-preview:free
        // - Should turn into a raw curl request like:
        //  https://openrouter.ai/api/v1/models/nousresearch/deephermes-3-llama-3-8b-preview%3Afree/endpoints

        let model_key = ModelKey::from_string(String::from("qwen/qwen3-30b-a3b"))?;
        let url = OpenRouter::endpoints_url_string(model_key);
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
        use std::path::PathBuf;

        use ploke_test_utils::workspace_root;

        use crate::llm2::chat_msg::Role;

        let model_key = ModelKey::from_string(String::from(
            "nousresearch/deephermes-3-llama-3-8b-preview:free",
        ))?;
        let url = OpenRouter::endpoints_url_string(model_key);
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

        let model_key = ModelKey::from_string(String::from("qwen/qwen3-30b-a3b-thinking-2507"))?;
        let key = OpenRouter::resolve_api_key()?;
        println!("key: {}", key);
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
            model: model_key.id(),
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
