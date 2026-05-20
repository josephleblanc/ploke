//! src/google.rs

use std::str::FromStr;

use crate::{LlmError, Router};

use serde::{Deserialize, Serialize};

use super::{ApiRoute, EndpointKey, ModelId, ModelKey, RouterModelId, RouterVariants};

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Hash, Eq, Default)]
pub struct Google;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct GoogleChatCompFields {
    // Google's OpenAI-compatible endpoint supports an `extra_body` field
    // for Gemini-specific settings (like thinking budgets/levels).
    // c.f. https://ai.google.dev/gemini-api/docs/openai
    // https://ai.google.dev/gemini-api/docs/openai#extra-body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_body: Option<ExtraBody>,
}

// TODO: fill out ExtraBody
//
// Parameter	Type	Endpoint	Description
// cached_content	Text	Chat	Corresponds to Gemini's general content cache.
// thinking_config	Object	Chat	Corresponds to Gemini's ThinkingConfig.
// aspect_ratio	Text	Images	Output aspect ratio (e.g., "16:9", "1:1", "9:16").
// generation_config	Object	Images	Gemini generation config object (e.g., {"responseModalities": ["IMAGE"], "candidateCount": 2}).
// safety_settings	List	Images	Custom safety threshold filters (e.g., [{"category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "BLOCK_NONE"}]).
// tools	List	Images	Enables grounding (e.g., [{"google_search": {}}]). Only for gemini-3-pro-image-preview.
// aspect_ratio	Text	Video	Dimensions of the output video (16:9 for landscape, 9:16 for portrait). Maps from size if not specified.
// resolution	Text	Video	Output resolution (720p, 1080p, 4K). Note: 1080p and 4K trigger upsampler pipeline.
// duration_seconds	Integer	Video	Generation length (values: 4, 6, 8). Must be 8 when using reference_images, interpolation, or extension.
// frame_rate	Text	Video	Frame rate for video output (e.g., "24").
// input_reference	Text	Video	Reference input for video generation.
// extend_video_id	Text	Video	ID of an existing video to extend.
// negative_prompt	Text	Video	Items to exclude (e.g., "shaky camera").
// seed	Integer	Video	Integer for deterministic generation.
// style	Text	Video	Visual styling (cinematic default, creative for social-media optimized).
// person_generation	Text	Video	Controls generation of people (allow_adult, allow_all, dont_allow).
// reference_images	List	Video	Up to 3 images for style/character reference (base64 assets).
// image	Text	Video	Base64-encoded initial input image to condition the video generation.
// last_frame	Object	Video	Final image for interpolation (requires image as first frame).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExtraBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google: Option<GoogleExtraBody>,
}

impl ExtraBody {
    pub fn with_google(mut self, google: GoogleExtraBody) -> Self {
        self.google = Some(google);
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GoogleExtraBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

impl GoogleExtraBody {
    pub fn with_thinking_config(mut self, thinking_config: ThinkingConfig) -> Self {
        self.thinking_config = Some(thinking_config);
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<ThinkingLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
}

impl ThinkingConfig {
    pub fn with_thinking_level(mut self, thinking_level: ThinkingLevel) -> Self {
        self.thinking_level = Some(thinking_level);
        self
    }

    pub fn with_include_thoughts(mut self, include_thoughts: bool) -> Self {
        self.include_thoughts = Some(include_thoughts);
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Low,
    #[default]
    Medium,
    High,
}

impl ApiRoute for GoogleChatCompFields {
    type Parent = RouterVariants;

    fn parent() -> Self::Parent {
        RouterVariants::Google(Google)
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Hash, Eq)]
pub struct GoogleModelId(ModelKey);

impl From<ModelId> for GoogleModelId {
    fn from(id: ModelId) -> Self {
        // Assuming ModelId has a `key` field based on your completion_core logic
        GoogleModelId(id.key)
    }
}

// TODO: Consider changing from `From` in the trait for `Router` now that we
// have more than one valid router, so we might want to use TryFrom instead.
impl From<EndpointKey> for GoogleModelId {
    fn from(key: EndpointKey) -> Self {
        // let EndpointKey { model, .. } = key;
        GoogleModelId(key.model)
    }
}

impl RouterModelId for GoogleModelId {
    fn into_key(self) -> ModelKey {
        self.0
    }

    fn key(&self) -> &ModelKey {
        &self.0
    }

    fn into_url_format(self) -> String {
        // Gemini doesn't use the OpenRouter {author}/{model} format in its URLs,
        // it just needs the raw string.
        self.0.slug.as_str().to_string()
    }

    fn model_id_from_request_string(model: &str) -> Result<ModelId, crate::IdError> {
        if model.contains('/') {
            ModelId::from_str(model)
        } else {
            ModelId::from_str(&format!("google/{model}"))
        }
    }
}

impl Router for Google {
    type CompletionFields = GoogleChatCompFields;
    type RouterModelId = GoogleModelId;

    // The official drop-in OpenAI-compatible endpoints
    const BASE_URL: &'static str = "https://generativelanguage.googleapis.com/v1beta/openai";
    const COMPLETION_URL: &'static str =
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions";
    const MODELS_URL: &'static str =
        "https://generativelanguage.googleapis.com/v1beta/openai/models";
    const ENDPOINTS_TAIL: &'static str = "";
    const API_KEY_NAME: &'static str = "GEMINI_API_KEY";
    const PROVIDERS_URL: &'static str = ""; // Not needed for direct API

    fn resolve_api_key() -> Result<String, LlmError> {
        std::env::var(Self::API_KEY_NAME).map_err(LlmError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::Google;

    use crate::router_only::Router;

    #[cfg(feature = "live_api_tests")]
    use std::time::Duration;

    #[cfg(feature = "live_api_tests")]
    use color_eyre::{Result, eyre::bail};
    #[cfg(feature = "live_api_tests")]
    use reqwest::Client;
    #[cfg(feature = "live_api_tests")]
    use serde_json::json;

    #[cfg(feature = "live_api_tests")]
    use crate::{HttpFailure, HttpSendFailure, LLM_TIMEOUT_SECS};

    #[cfg(feature = "live_api_tests")]
    fn body_snippet(body: &str) -> String {
        const MAX: usize = 1_000;
        body.chars().take(MAX).collect()
    }

    #[cfg(feature = "live_api_tests")]
    fn send_failure(url: &str, error: reqwest::Error) -> crate::LlmError {
        let phase = if error.is_timeout() {
            HttpSendFailure::Timeout
        } else {
            HttpSendFailure::Failed
        };
        crate::LlmError::Http(HttpFailure::send(
            Some(url.to_string()),
            None,
            error.to_string(),
            phase,
        ))
    }

    #[cfg(feature = "live_api_tests")]
    fn is_resource_exhausted(response: &serde_json::Value) -> bool {
        let error = response
            .get("error")
            .or_else(|| response.as_array()?.first()?.get("error"));

        error
            .and_then(|error| error.get("status"))
            .and_then(|status| status.as_str())
            == Some("RESOURCE_EXHAUSTED")
    }

    #[test]
    fn openai_compatible_route_constants_match_google() {
        assert_eq!(
            Google::BASE_URL,
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
        assert_eq!(
            Google::COMPLETION_URL,
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        );
        assert_eq!(
            Google::MODELS_URL,
            "https://generativelanguage.googleapis.com/v1beta/openai/models"
        );
        assert_eq!(Google::API_KEY_NAME, "GEMINI_API_KEY");
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_google_models_list_smoke() -> Result<()> {
        let key = Google::resolve_api_key()?;
        let url = Google::MODELS_URL;

        let response = Client::new()
            .get(url)
            .bearer_auth(key)
            .header("Accept", "application/json")
            .timeout(Duration::from_secs(LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|error| send_failure(url, error))?;

        let status = response.status();
        let response_text = response.text().await?;
        if !status.is_success() {
            bail!(
                "Google models list failed: status={} body={}",
                status,
                body_snippet(&response_text)
            );
        }

        let response_value: serde_json::Value = serde_json::from_str(&response_text)?;
        let models = response_value
            .get("data")
            .and_then(|value| value.as_array())
            .ok_or_else(|| color_eyre::eyre::eyre!("missing `data` array: {response_value}"))?;

        assert!(
            models
                .iter()
                .any(|model| model.get("id").and_then(|id| id.as_str()).is_some()),
            "expected at least one model id in Google models response: {response_value}"
        );

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_google_chat_completions_smoke_success_or_quota() -> Result<()> {
        let key = Google::resolve_api_key()?;
        let url = Google::COMPLETION_URL;
        let request = json!({
            "model": "gemini-2.5-flash",
            "messages": [
                {
                    "role": "user",
                    "content": "Reply with one short sentence."
                }
            ],
            "max_tokens": 32,
            "temperature": 0.0
        });

        let response = Client::new()
            .post(url)
            .bearer_auth(key)
            .json(&request)
            .timeout(Duration::from_secs(LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|error| send_failure(url, error))?;

        let status = response.status();
        let response_text = response.text().await?;
        let response_value: serde_json::Value = serde_json::from_str(&response_text)?;
        if !status.is_success() {
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS
                && is_resource_exhausted(&response_value)
            {
                // Authenticated quota exhaustion still verifies the live endpoint/model path.
                return Ok(());
            }

            bail!(
                "Google chat completion failed: status={} body={}",
                status,
                body_snippet(&response_text)
            );
        }

        let content = response_value
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .unwrap_or_default();

        assert!(
            !content.trim().is_empty(),
            "expected non-empty assistant content in Google completion response: {response_value}"
        );

        Ok(())
    }
}
