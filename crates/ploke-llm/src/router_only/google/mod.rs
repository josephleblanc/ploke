//! src/google.rs

use std::str::FromStr;

use ploke_core::ArcStr;

use crate::{
    Author, InputModality, LlmError, Modality, ModelName, ModelSlug, OutputModality, Router,
    SupportedParameters, Tokenizer,
    request::{ModelPricing, models, models::ModelRouteSource},
    router_only::{HasModelId, HasModels, openrouter::TopProvider},
    types::model_types::Architecture,
};

use serde::{Deserialize, Serialize};

use super::{ApiRoute, EndpointKey, ModelId, ModelKey, RouterModelId, RouterVariants};

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Hash, Eq, Default)]
pub struct Google;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<Model>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ArcStr>,
}

impl IntoIterator for ModelsResponse {
    type Item = Model;
    type IntoIter = std::vec::IntoIter<Model>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Model {
    pub id: ModelSlug,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ArcStr>,
    #[serde(default)]
    pub created: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owned_by: Option<ArcStr>,
}

impl HasModelId for Model {
    fn model_id(&self) -> ModelId {
        google_model_id(self.id.clone())
    }
}

impl From<Model> for models::ResponseItem {
    fn from(model: Model) -> Self {
        let model_id = model.model_id();
        let supported_parameters = supported_parameters_for_google_model(&model.id);

        Self {
            id: model_id.clone(),
            name: ModelName::new(model.id.as_str()),
            created: model.created.unwrap_or_default(),
            description: ArcStr::from(
                "Google OpenAI-compatible model metadata; pricing and full capability metadata are not included by /openai/models.",
            ),
            architecture: google_openai_architecture(),
            top_provider: TopProvider::default(),
            pricing: unknown_pricing(),
            canonical: Some(model_id),
            context_length: None,
            hugging_face_id: None,
            per_request_limits: None,
            supported_parameters,
            route_source: ModelRouteSource::DirectGoogle,
        }
    }
}

fn google_model_id(slug: ModelSlug) -> ModelId {
    ModelId {
        key: ModelKey {
            author: Author::new("google").expect("static Google author is valid"),
            slug,
        },
        variant: None,
    }
}

fn google_openai_architecture() -> Architecture {
    Architecture {
        input_modalities: vec![InputModality::Text],
        modality: Modality::TextToText,
        output_modalities: vec![OutputModality::Text],
        tokenizer: Tokenizer::Gemini,
        instruct_type: None,
    }
}

fn unknown_pricing() -> ModelPricing {
    ModelPricing {
        prompt: 0.0,
        completion: 0.0,
        audio: None,
        image: None,
        image_output: None,
        input_cache_read: None,
        input_cache_write: None,
        internal_reasoning: None,
        request: None,
        web_search: None,
        discount: None,
    }
}

fn supported_parameters_for_google_model(model: &ModelSlug) -> Option<Vec<SupportedParameters>> {
    if !is_google_openai_chat_model(model.as_str()) {
        return None;
    }

    Some(vec![
        SupportedParameters::MaxTokens,
        SupportedParameters::ResponseFormat,
        SupportedParameters::Stop,
        SupportedParameters::Temperature,
        SupportedParameters::ToolChoice,
        SupportedParameters::Tools,
        SupportedParameters::TopP,
    ])
}

fn is_google_openai_chat_model(slug: &str) -> bool {
    slug.starts_with("gemini-")
        && !slug.contains("embedding")
        && !slug.contains("image")
        && !slug.starts_with("imagen")
        && !slug.starts_with("veo-")
        && !slug.starts_with("lyria")
        && !slug.contains("tts")
}

impl HasModels for Google {
    type Response = ModelsResponse;
    type Models = Model;
    type Error = LlmError;
}

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
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
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

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct GoogleExtraBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

impl GoogleExtraBody {
    pub fn with_cached_content(mut self, cached_content: impl Into<String>) -> Self {
        self.cached_content = Some(cached_content.into());
        self
    }

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

    use crate::{
        SupportsTools,
        router_only::{HasModelId, HasModels, Router},
    };
    use serde_json::json;

    #[cfg(feature = "live_api_tests")]
    use std::{env, time::Duration};

    #[cfg(feature = "live_api_tests")]
    use crate::{
        ChatHttpConfig, ChatStepOutcome, HttpFailure, HttpSendFailure, LLM_TIMEOUT_SECS,
        manager::RequestMessage,
        request::endpoint::{ToolChoice, ToolChoiceFunction},
        router_only::{
            ChatCompRequest,
            google::{
                ExtraBody, GoogleChatCompFields, GoogleExtraBody, ThinkingConfig, ThinkingLevel,
            },
        },
    };
    #[cfg(feature = "live_api_tests")]
    use color_eyre::{Result, eyre::bail};
    #[cfg(feature = "live_api_tests")]
    use ploke_core::tool_types::{FunctionMarker, ToolDefinition, ToolFunctionDef, ToolName};
    #[cfg(feature = "live_api_tests")]
    use reqwest::Client;

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

    #[cfg(feature = "live_api_tests")]
    fn live_chat_model() -> String {
        env::var("PLOKE_LIVE_GOOGLE_CHAT_MODEL")
            .unwrap_or_else(|_| "google/gemini-2.5-flash".to_string())
    }

    #[cfg(feature = "live_api_tests")]
    async fn send_chat_request(
        request: &ChatCompRequest<Google>,
    ) -> Result<(reqwest::StatusCode, serde_json::Value, String)> {
        let key = Google::resolve_api_key()?;
        let url = Google::COMPLETION_URL;
        let response = Client::new()
            .post(url)
            .bearer_auth(key)
            .header("Accept", "application/json")
            .json(request)
            .timeout(Duration::from_secs(LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|error| send_failure(url, error))?;

        let status = response.status();
        let response_text = response.text().await?;
        let response_value: serde_json::Value = serde_json::from_str(&response_text)?;

        Ok((status, response_value, response_text))
    }

    #[cfg(feature = "live_api_tests")]
    fn first_message_content(response: &serde_json::Value) -> Option<&str> {
        response
            .get("choices")?
            .as_array()?
            .iter()
            .filter_map(|choice| {
                choice
                    .get("message")
                    .and_then(|message| message.get("content"))
                    .and_then(|content| content.as_str())
            })
            .find(|content| !content.trim().is_empty())
    }

    #[cfg(feature = "live_api_tests")]
    fn first_message_reasoning(response: &serde_json::Value) -> Option<&str> {
        response
            .get("choices")?
            .as_array()?
            .iter()
            .filter_map(|choice| {
                choice
                    .get("message")
                    .and_then(|message| message.get("reasoning"))
                    .and_then(|reasoning| reasoning.as_str())
            })
            .find(|reasoning| !reasoning.trim().is_empty())
    }

    #[cfg(feature = "live_api_tests")]
    fn json_contains_text(value: &serde_json::Value, needle: &str) -> bool {
        match value {
            serde_json::Value::String(text) => text.to_ascii_lowercase().contains(needle),
            serde_json::Value::Array(items) => {
                items.iter().any(|item| json_contains_text(item, needle))
            }
            serde_json::Value::Object(fields) => fields
                .values()
                .any(|field| json_contains_text(field, needle)),
            _ => false,
        }
    }

    #[cfg(feature = "live_api_tests")]
    fn is_google_quota_error(error: &crate::LlmError) -> bool {
        let text = format!("{error:?}");
        text.contains("RESOURCE_EXHAUSTED") || text.contains("429")
    }

    #[cfg(feature = "live_api_tests")]
    fn list_dir_tool_definition() -> ToolDefinition {
        ToolFunctionDef {
            name: ToolName::ListDir,
            description: "List entries under a relative directory path.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative directory path to list."
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        }
        .into()
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

    #[test]
    fn google_models_response_deserializes_and_adapts_to_shared_registry_item() {
        let response: <Google as HasModels>::Response = serde_json::from_value(json!({
            "object": "list",
            "data": [
                {
                    "id": "gemini-2.5-flash",
                    "object": "model",
                    "created": 1710000000,
                    "owned_by": "google"
                }
            ]
        }))
        .expect("Google OpenAI-compatible models response parses");

        let mut models = response.into_iter();
        let model = models.next().expect("one model");
        assert_eq!(model.model_id().to_string(), "google/gemini-2.5-flash");

        let item: crate::request::models::ResponseItem = model.into();
        assert_eq!(item.id.to_string(), "google/gemini-2.5-flash");
        assert_eq!(item.name.as_str(), "gemini-2.5-flash");
        assert_eq!(item.created, 1710000000);
        assert!(item.supports_tools());
        assert_eq!(item.architecture.tokenizer, crate::Tokenizer::Gemini);
    }

    #[test]
    fn google_models_fixture_adapts_to_direct_registry_rows() {
        static RAW: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/test_data/google/openai_models.json"
        ));
        let response: <Google as HasModels>::Response =
            serde_json::from_str(RAW).expect("Google OpenAI-compatible models fixture parses");

        let items = response
            .into_iter()
            .map(crate::request::models::ResponseItem::from)
            .collect::<Vec<_>>();

        assert_eq!(items.len(), 5);
        assert!(
            items
                .iter()
                .all(|item| item.route_source.is_direct_google()),
            "Google fixture rows must retain direct route provenance: {items:#?}"
        );

        let flash = items
            .iter()
            .find(|item| item.id.to_string() == "google/gemini-2.5-flash")
            .expect("flash model");
        assert!(flash.supports_tools());
        assert_eq!(flash.canonical.as_ref(), Some(&flash.id));
        assert_eq!(flash.pricing.prompt, 0.0);

        for item in items.iter().filter(|item| {
            let id = item.id.to_string();
            id.contains("embedding") || id.contains("imagen") || id.contains("veo-")
        }) {
            assert!(
                !item.supports_tools(),
                "{} should not advertise tools",
                item.id
            );
        }
    }

    #[test]
    fn google_embedding_model_adapter_does_not_advertise_tool_support() {
        let response: <Google as HasModels>::Response = serde_json::from_value(json!({
            "object": "list",
            "data": [
                {
                    "id": "text-embedding-004",
                    "object": "model",
                    "owned_by": "google"
                }
            ]
        }))
        .expect("Google OpenAI-compatible models response parses");

        let model = response.into_iter().next().expect("one model");
        let item: crate::request::models::ResponseItem = model.into();

        assert_eq!(item.id.to_string(), "google/text-embedding-004");
        assert!(!item.supports_tools());
    }

    #[test]
    fn google_non_chat_openai_models_do_not_advertise_tool_support() {
        for id in [
            "gemini-embedding-2-preview",
            "gemini-2.5-flash-image",
            "veo-3.1-generate-preview",
            "imagen-4.0-generate-preview",
        ] {
            let response: <Google as HasModels>::Response = serde_json::from_value(json!({
                "object": "list",
                "data": [
                    {
                        "id": id,
                        "object": "model",
                        "owned_by": "google"
                    }
                ]
            }))
            .expect("Google OpenAI-compatible models response parses");

            let model = response.into_iter().next().expect("one model");
            let item: crate::request::models::ResponseItem = model.into();

            assert!(!item.supports_tools(), "{id} should not advertise tools");
        }
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
        let typed_response: <Google as HasModels>::Response =
            serde_json::from_value(response_value.clone())?;
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
        assert!(
            typed_response
                .into_iter()
                .map(crate::request::models::ResponseItem::from)
                .any(|item| item.route_source.is_direct_google()),
            "expected live Google models response to adapt into direct route rows"
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

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    #[ignore = "requires GEMINI_API_KEY, a live Google model with tool support, and quota"]
    async fn live_google_chat_step_forced_tool_call_success_or_quota() -> Result<()> {
        let request = ChatCompRequest::<Google>::default()
            .with_model_str(&live_chat_model())?
            .with_message(RequestMessage::new_user(
                "Call the list_dir tool exactly once for path \".\". Do not answer in prose."
                    .to_string(),
            ))
            .with_max_tokens(128)
            .with_temperature(0.0)
            .with_tools(Some(vec![list_dir_tool_definition()]))
            .with_tool_choice(Some(ToolChoice::Function {
                r#type: FunctionMarker,
                function: ToolChoiceFunction {
                    name: ToolName::ListDir.as_str().to_string(),
                },
            }));

        let client = Client::new();
        let cfg = ChatHttpConfig::default();
        let step = match crate::chat_step(&client, &request, &cfg).await {
            Ok(step) => step,
            Err(error) if is_google_quota_error(&error) => return Ok(()),
            Err(error) => return Err(error.into()),
        };

        match step.outcome {
            ChatStepOutcome::ToolCalls { calls, .. } => {
                assert_eq!(calls.len(), 1, "expected exactly one Google tool call");
                assert_eq!(calls[0].function.name, ToolName::ListDir);
                assert!(
                    calls[0].function.arguments.contains("\"path\""),
                    "expected list_dir arguments to include path: {}",
                    calls[0].function.arguments
                );
            }
            other => {
                bail!("expected forced Google tool call through chat_step, got {other:?}");
            }
        }

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_google_thinking_request_returns_reasoning_success_or_quota() -> Result<()> {
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
            .with_model_str(&live_chat_model())?
            .with_message(RequestMessage::new_user(
                "Explain why 13 is prime in one short paragraph.".to_string(),
            ))
            .with_max_tokens(256)
            .with_temperature(0.0)
            .with_router_bundle(router);

        let (status, response_value, response_text) = send_chat_request(&request).await?;
        if !status.is_success() {
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS
                && is_resource_exhausted(&response_value)
            {
                return Ok(());
            }

            bail!(
                "Google thinking chat completion failed: status={} body={}",
                status,
                body_snippet(&response_text)
            );
        }

        assert!(
            first_message_content(&response_value).is_some(),
            "expected assistant content in Google thinking response: {response_value}"
        );
        assert!(
            first_message_reasoning(&response_value).is_some(),
            "expected non-empty message.reasoning when include_thoughts=true: {response_value}"
        );

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_google_cached_content_reaches_provider_as_rejected_resource() -> Result<()> {
        let missing_cache = "cachedContents/ploke-live-test-missing-cache";
        let router = GoogleChatCompFields {
            extra_body: Some(
                ExtraBody::default()
                    .with_google(GoogleExtraBody::default().with_cached_content(missing_cache)),
            ),
        };
        let request = ChatCompRequest::<Google>::default()
            .with_model_str(&live_chat_model())?
            .with_message(RequestMessage::new_user(
                "Reply with the word ok.".to_string(),
            ))
            .with_max_tokens(8)
            .with_temperature(0.0)
            .with_router_bundle(router);

        let (status, response_value, response_text) = send_chat_request(&request).await?;
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS
            && is_resource_exhausted(&response_value)
        {
            return Ok(());
        }

        assert!(
            !status.is_success(),
            "expected missing cached_content resource to be rejected, got success: {response_value}"
        );
        assert!(
            json_contains_text(&response_value, "cached")
                || json_contains_text(&response_value, "cache")
                || json_contains_text(&response_value, missing_cache),
            "expected cache-related error for missing cached_content, status={} body={}",
            status,
            body_snippet(&response_text)
        );

        Ok(())
    }
}
