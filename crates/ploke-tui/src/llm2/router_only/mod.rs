//! Router-specific implementations

pub(crate) mod openrouter;

use itertools::Itertools;
use openrouter::{FallbackMarker, MiddleOutMarker, Transform};
use serde::{Deserialize, Serialize};

use crate::tools::ToolDefinition;

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
    const COMPLETION_TAIL: &str;
    const MODELS_URL: &str;
    const ENDPOINTS_TAIL: &str;
    const API_KEY_NAME: &str;

    fn resolve_api_key() -> Result<String, std::env::VarError> {
        // 1. Check provider-specific env var if specified
        let key_name = Self::API_KEY_NAME;
        std::env::var(key_name)
    }

    fn endpoints_url_string(model_key: ModelKey) -> String {
        [Self::BASE_URL, model_key.id.as_str(), Self::ENDPOINTS_TAIL]
            .into_iter()
            .join("/")
    }
}

#[derive(Default, Copy, Clone, Debug)]
pub(crate) struct OpenRouter;

impl Router for OpenRouter {
    type CompletionFields = openrouter::ChatCompFields;
    const BASE_URL: &str = "https://openrouter.ai/api/v1";
    const COMPLETION_TAIL: &str = "chat/completions";
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
            messages: req.messages,
            prompt: req.prompt,
            model: req.model,
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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub(crate) struct ChatCompRequest<R: ApiRoute> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) messages: Option<Vec<RequestMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<String>,
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

#[cfg(test)]
mod tests {
    use std::time::Duration;


    use crate::{llm::LlmError, llm2::newtypes::ModelId};

    use super::*;
    #[test]
    fn show_openrouter_json2() {
        let req = ChatCompRequest::<openrouter::ChatCompFields> {
            messages: Some(vec![]),
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
    async fn test_simple_query_models() -> Result<()> {
        let url = OpenRouter::MODELS_URL;
        let key = OpenRouter::resolve_api_key()?;

        let response = Client::new()
            .post(url)
            .bearer_auth(key)
            .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;

        // AI: add a check for an environmental variable, "WRITE_MODE", and if it equals 1, then
        // write the response json to `MODELS_JSON_RAW` AI!
        Ok(())
    }
}
