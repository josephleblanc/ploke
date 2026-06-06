//! src/google.rs

use std::str::FromStr;

use google_cloud_auth::credentials::{AccessTokenCredentials, Builder as GoogleAuthBuilder};
use once_cell::sync::OnceCell;
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

static GOOGLE_OPENAPI_BASE_URL: OnceCell<String> = OnceCell::new();
static GOOGLE_COMPLETION_URL: OnceCell<String> = OnceCell::new();
static GOOGLE_ADC_CREDENTIALS: OnceCell<AccessTokenCredentials> = OnceCell::new();

fn google_project_id() -> Result<String, LlmError> {
    required_google_env("GOOGLE_PROJECT_ID")
}

fn google_region() -> Result<String, LlmError> {
    required_google_env("GOOGLE_REGION")
}

fn required_google_env(name: &'static str) -> Result<String, LlmError> {
    let value = std::env::var(name).map_err(|source| LlmError::Var {
        message: "required Google route environment variable is unset",
        original: format!("{name}: {source}"),
    })?;
    if value.trim().is_empty() {
        return Err(LlmError::Var {
            message: "required Google route environment variable is empty",
            original: name.to_string(),
        });
    }
    Ok(value)
}

fn build_google_openapi_base_url() -> Result<String, LlmError> {
    let project_id = google_project_id()?;
    let region = google_region()?;
    Ok(google_openapi_base_url(&project_id, &region))
}

fn google_openapi_base_url(project_id: &str, region: &str) -> String {
    format!(
        "{}/projects/{project_id}/locations/{region}/{}",
        Google::BASE_URL,
        Google::OPENAPI_ENDPOINT
    )
}

fn google_auth_error(error: impl std::fmt::Display) -> LlmError {
    LlmError::Var {
        message: "failed to resolve Google application default credentials",
        original: error.to_string(),
    }
}

fn google_adc_credentials() -> Result<&'static AccessTokenCredentials, LlmError> {
    GOOGLE_ADC_CREDENTIALS.get_or_try_init(|| {
        GoogleAuthBuilder::default()
            .with_scopes(["https://www.googleapis.com/auth/cloud-platform"])
            .build_access_token_credentials()
            .map_err(google_auth_error)
    })
}

async fn google_adc_bearer_token() -> Result<String, LlmError> {
    Ok(google_adc_credentials()?
        .access_token()
        .await
        .map_err(google_auth_error)?
        .token)
}

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
        let normalized_slug = normalize_google_model_slug(&model.id);
        let model_id = model.model_id();
        let supported_parameters = supported_parameters_for_google_model(&normalized_slug);

        Self {
            id: model_id.clone(),
            name: ModelName::new(normalized_slug.as_str()),
            created: model.created.unwrap_or_default(),
            description: ArcStr::from(
                "Direct Google catalog row for Vertex OpenAI-compatible chat completions.",
            ),
            architecture: google_openai_architecture(),
            top_provider: TopProvider::default(),
            pricing: unknown_pricing(),
            canonical: Some(model_id),
            context_length: google_context_length(&normalized_slug),
            hugging_face_id: None,
            per_request_limits: None,
            supported_parameters,
            route_source: ModelRouteSource::DirectGoogle,
        }
    }
}

fn google_model_id(slug: ModelSlug) -> ModelId {
    let slug = normalize_google_model_slug(&slug);
    ModelId {
        key: ModelKey {
            author: Author::new("google").expect("static Google author is valid"),
            slug,
        },
        variant: None,
    }
}

fn normalize_google_model_slug(slug: &ModelSlug) -> ModelSlug {
    let raw = slug.as_str();
    let normalized = raw.strip_prefix("models/").unwrap_or(raw);
    ModelSlug::new(normalized)
        .expect("Google model-list slug should remain valid after prefix strip")
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

fn google_context_length(model: &ModelSlug) -> Option<u32> {
    match model.as_str() {
        "gemini-2.5-flash" | "gemini-3.5-flash" => Some(1_048_576),
        _ => None,
    }
}

fn google_catalog_model(slug: &str) -> Model {
    Model {
        id: ModelSlug::new(slug).expect("static Google catalog slug is valid"),
        object: Some(ArcStr::from("model")),
        created: Some(0),
        owned_by: Some(ArcStr::from("google")),
    }
}

fn google_catalog_models_response() -> ModelsResponse {
    ModelsResponse {
        data: vec![
            google_catalog_model("gemini-2.5-flash"),
            google_catalog_model("gemini-3.5-flash"),
        ],
        object: Some(ArcStr::from("list")),
    }
}

impl HasModels for Google {
    type Response = ModelsResponse;
    type Models = Model;
    type Error = LlmError;

    #[allow(clippy::manual_async_fn)]
    fn fetch_models(
        _client: &reqwest::Client,
    ) -> impl std::future::Future<Output = color_eyre::Result<Self::Response>> + Send {
        async { Ok(google_catalog_models_response()) }
    }
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
        // Vertex OpenAI compatibility expects the Google provider prefix in
        // request bodies, e.g. `google/gemini-2.5-flash`.
        format!("{}/{}", self.0.author.as_str(), self.0.slug.as_str())
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

    const BASE_URL: &'static str = "https://aiplatform.googleapis.com/v1";
    const COMPLETION_URL: &'static str = concat!(
        "https://aiplatform.googleapis.com/v1/projects/",
        "{GOOGLE_PROJECT_ID}/locations/{GOOGLE_REGION}/endpoints/openapi/chat/completions"
    );
    const MODELS_URL: &'static str = "";
    const ENDPOINTS_TAIL: &'static str = "";
    const API_KEY_NAME: &'static str = "GOOGLE_API_KEY";
    const PROVIDERS_URL: &'static str = "";

    fn resolve_api_key() -> Result<String, LlmError> {
        let token = std::env::var(Self::API_KEY_NAME).map_err(LlmError::from)?;
        if token.trim().is_empty() {
            return Err(LlmError::Var {
                message: "required Google bearer token environment variable is empty",
                original: Self::API_KEY_NAME.to_string(),
            });
        }
        Ok(token)
    }

    #[allow(clippy::manual_async_fn)]
    fn resolve_bearer_token() -> impl std::future::Future<Output = Result<String, LlmError>> + Send
    {
        async { google_adc_bearer_token().await }
    }

    fn completion_url() -> Result<&'static str, LlmError> {
        Ok(GOOGLE_COMPLETION_URL
            .get_or_try_init(|| {
                Ok::<String, LlmError>(format!(
                    "{}/{}",
                    Self::openapi_base_url()?,
                    Self::COMPLETION_ENDPOINT
                ))
            })?
            .as_str())
    }

    fn models_url() -> Result<&'static str, LlmError> {
        Err(LlmError::Unknown(
            "Vertex OpenAI-compatible API does not expose an OpenAI-compatible models endpoint"
                .to_string(),
        ))
    }
}

impl Google {
    pub const OPENAPI_ENDPOINT: &'static str = "endpoints/openapi";
    pub const COMPLETION_ENDPOINT: &'static str = "chat/completions";
    pub const MODELS_ENDPOINT: &'static str = "models";
    pub const LOCAL_AUTH_HINTS_ENV: &'static str = "PLOKE_LOCAL_GOOGLE_AUTH_HINTS";
    pub const AUTH_PREFLIGHT_COMMAND: &'static str = "cargo xtask auth google --strict-live";

    pub fn openapi_base_url() -> Result<&'static str, LlmError> {
        Ok(GOOGLE_OPENAPI_BASE_URL
            .get_or_try_init(build_google_openapi_base_url)?
            .as_str())
    }

    pub fn adc_credentials_available() -> Result<(), LlmError> {
        google_adc_credentials().map(|_| ())
    }

    pub fn route_config_available() -> Result<(), LlmError> {
        google_project_id()?;
        google_region()?;
        Ok(())
    }

    pub fn auth_config_available() -> Result<(), LlmError> {
        Self::adc_credentials_available()
    }

    pub fn local_auth_preflight_hint() -> Option<&'static str> {
        let enabled = std::env::var(Self::LOCAL_AUTH_HINTS_ENV)
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"));
        enabled.then_some("local Google auth hint: run `cargo xtask auth google --strict-live`")
    }

    pub fn with_local_auth_preflight_hint(mut message: String) -> String {
        if let Some(hint) = Self::local_auth_preflight_hint() {
            message.push_str("; ");
            message.push_str(hint);
        }
        message
    }
}

#[cfg(test)]
mod tests {
    use super::{Google, google_openapi_base_url};

    use crate::{
        SupportsTools,
        router_only::{HasModelId, HasModels, Router},
    };
    use serde_json::json;
    use std::sync::{Mutex, OnceLock};

    #[cfg(feature = "live_api_tests")]
    use std::{collections::BTreeSet, env, time::Duration};

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

    fn google_auth_hint_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn local_auth_preflight_hint_is_opt_in_by_env() {
        let _lock = google_auth_hint_env_lock()
            .lock()
            .expect("Google auth hint env lock is available");
        let previous = std::env::var_os(Google::LOCAL_AUTH_HINTS_ENV);

        unsafe {
            std::env::remove_var(Google::LOCAL_AUTH_HINTS_ENV);
        }
        assert!(
            Google::local_auth_preflight_hint().is_none(),
            "local preflight hint should be absent unless explicitly enabled"
        );

        unsafe {
            std::env::set_var(Google::LOCAL_AUTH_HINTS_ENV, "1");
        }
        assert_eq!(
            Google::local_auth_preflight_hint(),
            Some("local Google auth hint: run `cargo xtask auth google --strict-live`")
        );

        match previous {
            Some(value) => unsafe {
                std::env::set_var(Google::LOCAL_AUTH_HINTS_ENV, value);
            },
            None => unsafe {
                std::env::remove_var(Google::LOCAL_AUTH_HINTS_ENV);
            },
        }
    }

    #[cfg(feature = "live_api_tests")]
    fn body_snippet(body: &str) -> String {
        const MAX: usize = 1_000;
        body.chars().take(MAX).collect()
    }

    #[cfg(feature = "live_api_tests")]
    fn strict_live_tests_requested() -> bool {
        env::var("PLOKE_RUN_LIVE_TESTS")
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
    }

    #[cfg(feature = "live_api_tests")]
    fn live_google_env_or_skip(test_name: &str) -> bool {
        let route_config_available = Google::route_config_available().is_ok();
        let auth_config_available = Google::auth_config_available().is_ok();

        if route_config_available && auth_config_available {
            return true;
        }

        let missing = match (route_config_available, auth_config_available) {
            (false, false) => "GOOGLE_PROJECT_ID/GOOGLE_REGION route config and Google ADC auth",
            (false, true) => "GOOGLE_PROJECT_ID/GOOGLE_REGION route config",
            (true, false) => "Google ADC auth",
            (true, true) => unreachable!("handled above"),
        };
        let message = Google::with_local_auth_preflight_hint(format!(
            "skipping {test_name}: missing {missing}; direct Google live route was not exercised"
        ));
        if strict_live_tests_requested() {
            panic!("{message}; PLOKE_RUN_LIVE_TESTS requested live execution");
        }
        eprintln!("{message}");
        false
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
    fn live_thinking_model() -> String {
        env::var("PLOKE_LIVE_GOOGLE_THINKING_MODEL")
            .unwrap_or_else(|_| "google/gemini-3.1-pro-preview".to_string())
    }

    #[cfg(feature = "live_api_tests")]
    fn google_slug_model(model: &str) -> String {
        let without_author = model.strip_prefix("google/").unwrap_or(model);
        without_author
            .strip_prefix("models/")
            .unwrap_or(without_author)
            .to_string()
    }

    #[cfg(feature = "live_api_tests")]
    fn endpoint_probe_regions() -> Vec<String> {
        let mut regions = BTreeSet::new();
        if let Ok(raw) = env::var("PLOKE_LIVE_GOOGLE_VERTEX_REGIONS") {
            for region in raw
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                regions.insert(region.to_string());
            }
        }
        if let Ok(region) = env::var("GOOGLE_REGION")
            && !region.trim().is_empty()
        {
            regions.insert(region);
        }
        regions.insert("global".to_string());
        regions.insert("us-central1".to_string());
        regions.into_iter().collect()
    }

    #[cfg(feature = "live_api_tests")]
    fn google_ai_studio_api_key() -> Option<(&'static str, String)> {
        [
            "PLOKE_GOOGLE_AI_STUDIO_API_KEY",
            "GEMINI_API_KEY",
            "GOOGLE_API_KEY",
        ]
        .into_iter()
        .find_map(|name| {
            env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|value| (name, value))
        })
    }

    #[cfg(feature = "live_api_tests")]
    fn google_error_field<'a>(response: &'a serde_json::Value, field: &str) -> Option<&'a str> {
        response
            .get("error")
            .or_else(|| response.as_array()?.first()?.get("error"))
            .and_then(|error| error.get(field))
            .and_then(|value| value.as_str())
    }

    #[cfg(feature = "live_api_tests")]
    #[derive(Debug, serde::Serialize)]
    struct EndpointProbeReport {
        label: String,
        auth: String,
        model: String,
        url: String,
        status: Option<u16>,
        resource_exhausted: bool,
        error_status: Option<String>,
        error_message: Option<String>,
        content: Option<String>,
        finish_reason: Option<String>,
        body_excerpt: String,
    }

    #[cfg(feature = "live_api_tests")]
    async fn send_endpoint_probe(
        label: String,
        auth: String,
        token: String,
        url: String,
        model: String,
    ) -> EndpointProbeReport {
        let request = json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": "Reply exactly: google-endpoint-probe-ok"
            }],
            "max_tokens": 64,
            "temperature": 0.0
        });
        let response = Client::new()
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/json")
            .json(&request)
            .timeout(Duration::from_secs(60))
            .send()
            .await;

        let Ok(response) = response else {
            return EndpointProbeReport {
                label,
                auth,
                model,
                url,
                status: None,
                resource_exhausted: false,
                error_status: None,
                error_message: response.err().map(|error| error.to_string()),
                content: None,
                finish_reason: None,
                body_excerpt: String::new(),
            };
        };

        let status = response.status();
        let response_text = response
            .text()
            .await
            .unwrap_or_else(|error| format!("<failed to read response body: {error}>"));
        let response_value = serde_json::from_str::<serde_json::Value>(&response_text).ok();
        let resource_exhausted = response_value.as_ref().is_some_and(is_resource_exhausted);
        let content = response_value
            .as_ref()
            .and_then(first_message_content)
            .map(str::to_string);
        let finish_reason = response_value
            .as_ref()
            .and_then(|value| value.get("choices"))
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(|reason| reason.as_str())
            .map(str::to_string);
        let error_status = response_value
            .as_ref()
            .and_then(|value| google_error_field(value, "status"))
            .map(str::to_string);
        let error_message = response_value
            .as_ref()
            .and_then(|value| google_error_field(value, "message"))
            .map(str::to_string);

        EndpointProbeReport {
            label,
            auth,
            model,
            url,
            status: Some(status.as_u16()),
            resource_exhausted,
            error_status,
            error_message,
            content,
            finish_reason,
            body_excerpt: body_snippet(&response_text),
        }
    }

    #[cfg(feature = "live_api_tests")]
    fn native_generate_content_body() -> serde_json::Value {
        json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": "Reply exactly: google-endpoint-probe-ok" }]
            }],
            "generationConfig": {
                "maxOutputTokens": 64,
                "temperature": 0.0
            }
        })
    }

    #[cfg(feature = "live_api_tests")]
    fn first_native_content(response: &serde_json::Value) -> Option<String> {
        let parts = response
            .get("candidates")?
            .as_array()?
            .first()?
            .get("content")?
            .get("parts")?
            .as_array()?;
        let content = parts
            .iter()
            .filter_map(|part| part.get("text").and_then(|text| text.as_str()))
            .collect::<Vec<_>>()
            .join("");
        (!content.trim().is_empty()).then_some(content)
    }

    #[cfg(feature = "live_api_tests")]
    fn first_native_finish_reason(response: &serde_json::Value) -> Option<String> {
        response
            .get("candidates")?
            .as_array()?
            .first()?
            .get("finishReason")?
            .as_str()
            .map(str::to_string)
    }

    #[cfg(feature = "live_api_tests")]
    async fn send_native_bearer_probe(
        label: String,
        auth: String,
        token: String,
        url: String,
        model: String,
    ) -> EndpointProbeReport {
        let response = Client::new()
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/json")
            .json(&native_generate_content_body())
            .timeout(Duration::from_secs(60))
            .send()
            .await;
        endpoint_report_from_response(label, auth, model, url, response).await
    }

    #[cfg(feature = "live_api_tests")]
    async fn send_native_api_key_probe(
        label: String,
        auth: String,
        key: String,
        url: String,
        model: String,
    ) -> EndpointProbeReport {
        let request_url = format!("{url}?key={key}");
        let response = Client::new()
            .post(&request_url)
            .header("Accept", "application/json")
            .json(&native_generate_content_body())
            .timeout(Duration::from_secs(60))
            .send()
            .await;
        endpoint_report_from_response(label, auth, model, url, response).await
    }

    #[cfg(feature = "live_api_tests")]
    async fn endpoint_report_from_response(
        label: String,
        auth: String,
        model: String,
        url: String,
        response: std::result::Result<reqwest::Response, reqwest::Error>,
    ) -> EndpointProbeReport {
        let Ok(response) = response else {
            return EndpointProbeReport {
                label,
                auth,
                model,
                url,
                status: None,
                resource_exhausted: false,
                error_status: None,
                error_message: response.err().map(|error| error.to_string()),
                content: None,
                finish_reason: None,
                body_excerpt: String::new(),
            };
        };

        let status = response.status();
        let response_text = response
            .text()
            .await
            .unwrap_or_else(|error| format!("<failed to read response body: {error}>"));
        let response_value = serde_json::from_str::<serde_json::Value>(&response_text).ok();
        let resource_exhausted = response_value.as_ref().is_some_and(is_resource_exhausted);
        let content = response_value
            .as_ref()
            .and_then(first_message_content)
            .map(str::to_string)
            .or_else(|| response_value.as_ref().and_then(first_native_content));
        let finish_reason = response_value
            .as_ref()
            .and_then(|value| value.get("choices"))
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(|reason| reason.as_str())
            .map(str::to_string)
            .or_else(|| response_value.as_ref().and_then(first_native_finish_reason));
        let error_status = response_value
            .as_ref()
            .and_then(|value| google_error_field(value, "status"))
            .map(str::to_string);
        let error_message = response_value
            .as_ref()
            .and_then(|value| google_error_field(value, "message"))
            .map(str::to_string);

        EndpointProbeReport {
            label,
            auth,
            model,
            url,
            status: Some(status.as_u16()),
            resource_exhausted,
            error_status,
            error_message,
            content,
            finish_reason,
            body_excerpt: body_snippet(&response_text),
        }
    }

    #[cfg(feature = "live_api_tests")]
    async fn send_chat_request(
        request: &ChatCompRequest<Google>,
    ) -> Result<(reqwest::StatusCode, serde_json::Value, String)> {
        let key = Google::resolve_bearer_token().await?;
        let url = Google::completion_url()?;
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
    fn first_message_has_google_thought(response: &serde_json::Value) -> bool {
        response
            .get("choices")
            .and_then(|choices| choices.as_array())
            .into_iter()
            .flatten()
            .filter_map(|choice| choice.get("message"))
            .any(|message| {
                message
                    .pointer("/extra_content/google/thought")
                    .and_then(|thought| thought.as_bool())
                    == Some(true)
            })
    }

    #[cfg(feature = "live_api_tests")]
    fn response_reports_reasoning_tokens(response: &serde_json::Value) -> bool {
        response
            .pointer("/usage/completion_tokens_details/reasoning_tokens")
            .and_then(|tokens| tokens.as_u64())
            .is_some_and(|tokens| tokens > 0)
    }

    #[cfg(feature = "live_api_tests")]
    fn response_has_thinking_evidence(response: &serde_json::Value) -> bool {
        first_message_reasoning(response).is_some()
            || first_message_has_google_thought(response)
            || response_reports_reasoning_tokens(response)
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
        assert_eq!(Google::BASE_URL, "https://aiplatform.googleapis.com/v1");
        assert_eq!(Google::OPENAPI_ENDPOINT, "endpoints/openapi");
        assert_eq!(Google::COMPLETION_ENDPOINT, "chat/completions");
        assert_eq!(Google::MODELS_ENDPOINT, "models");
        assert_eq!(Google::MODELS_URL, "");
        assert_eq!(Google::API_KEY_NAME, "GOOGLE_API_KEY");
    }

    #[test]
    fn openai_compatible_models_url_is_not_supported_by_vertex() {
        let error = Google::models_url().expect_err("Google models URL should be unsupported");
        assert!(
            error
                .to_string()
                .contains("does not expose an OpenAI-compatible models endpoint"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn vertex_openapi_base_url_uses_project_and_region_path() {
        assert_eq!(
            super::google_openapi_base_url("ploke-project", "us-central1"),
            "https://aiplatform.googleapis.com/v1/projects/ploke-project/locations/us-central1/endpoints/openapi"
        );
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
    fn google_models_response_strips_models_prefix_for_registry_identity() {
        let response: <Google as HasModels>::Response = serde_json::from_value(json!({
            "object": "list",
            "data": [
                {
                    "id": "models/gemini-2.5-flash",
                    "object": "model",
                    "created": 1710000000,
                    "owned_by": "google"
                }
            ]
        }))
        .expect("Google OpenAI-compatible models response parses");

        let model = response.into_iter().next().expect("one model");
        assert_eq!(model.model_id().to_string(), "google/gemini-2.5-flash");

        let item: crate::request::models::ResponseItem = model.into();
        assert_eq!(item.id.to_string(), "google/gemini-2.5-flash");
        assert_eq!(item.name.as_str(), "gemini-2.5-flash");
        assert!(item.supports_tools());
        assert!(item.route_source.is_direct_google());
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
    async fn google_catalog_fetch_returns_direct_flash_row() -> color_eyre::Result<()> {
        let typed_response = Google::fetch_models(&reqwest::Client::new()).await?;
        let items = typed_response
            .into_iter()
            .map(crate::request::models::ResponseItem::from)
            .collect::<Vec<_>>();

        assert_eq!(items.len(), 2);
        let flash = items
            .iter()
            .find(|item| item.id.to_string() == "google/gemini-2.5-flash")
            .expect("2.5 flash direct row");
        assert_eq!(flash.id.to_string(), "google/gemini-2.5-flash");
        assert_eq!(flash.context_length, Some(1_048_576));
        assert!(flash.supports_tools());
        assert!(flash.route_source.is_direct_google());
        let flash_35 = items
            .iter()
            .find(|item| item.id.to_string() == "google/gemini-3.5-flash")
            .expect("3.5 flash direct row");
        assert!(flash_35.route_source.is_direct_google());

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_google_chat_completions_smoke_success_or_quota() -> Result<()> {
        const TEST_NAME: &str = "live_google_chat_completions_smoke_success_or_quota";
        if !live_google_env_or_skip(TEST_NAME) {
            return Ok(());
        }

        let key = Google::resolve_bearer_token().await?;
        let url = Google::completion_url()?;
        let request = json!({
            "model": "google/gemini-2.5-flash",
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
    #[ignore = "live diagnostic: compares Vertex and AI Studio Google endpoint surfaces"]
    async fn live_google_endpoint_matrix_reports_vertex_vs_ai_studio_status() -> Result<()> {
        let full_model = live_chat_model();
        let slug_model = google_slug_model(&full_model);
        let mut reports = Vec::new();
        let mut candidates = 0_usize;

        match (
            env::var("GOOGLE_PROJECT_ID"),
            Google::resolve_bearer_token().await,
        ) {
            (Ok(project_id), Ok(token)) if !project_id.trim().is_empty() => {
                for region in endpoint_probe_regions() {
                    let openai_url = format!(
                        "{}/{}",
                        google_openapi_base_url(&project_id, &region),
                        Google::COMPLETION_ENDPOINT
                    );
                    for (model_label, model) in [
                        ("google-prefix-model", full_model.clone()),
                        ("slug-model", slug_model.clone()),
                    ] {
                        candidates += 1;
                        reports.push(
                            send_endpoint_probe(
                                format!("vertex-openai-default-host-{region}-{model_label}"),
                                "adc-cloud-platform".to_string(),
                                token.clone(),
                                openai_url.clone(),
                                model,
                            )
                            .await,
                        );
                    }

                    if region != "global" {
                        let regional_openai_url = format!(
                            "https://{region}-aiplatform.googleapis.com/v1/projects/{project_id}/locations/{region}/{}/{}",
                            Google::OPENAPI_ENDPOINT,
                            Google::COMPLETION_ENDPOINT
                        );
                        candidates += 1;
                        reports.push(
                            send_endpoint_probe(
                                format!("vertex-openai-regional-host-{region}-google-prefix-model"),
                                "adc-cloud-platform".to_string(),
                                token.clone(),
                                regional_openai_url,
                                full_model.clone(),
                            )
                            .await,
                        );
                    }

                    let native_url = format!(
                        "https://aiplatform.googleapis.com/v1/projects/{project_id}/locations/{region}/publishers/google/models/{slug_model}:generateContent"
                    );
                    candidates += 1;
                    reports.push(
                        send_native_bearer_probe(
                            format!("vertex-native-default-host-{region}-slug-model"),
                            "adc-cloud-platform".to_string(),
                            token.clone(),
                            native_url,
                            slug_model.clone(),
                        )
                        .await,
                    );

                    if region != "global" {
                        let regional_native_url = format!(
                            "https://{region}-aiplatform.googleapis.com/v1/projects/{project_id}/locations/{region}/publishers/google/models/{slug_model}:generateContent"
                        );
                        candidates += 1;
                        reports.push(
                            send_native_bearer_probe(
                                format!("vertex-native-regional-host-{region}-slug-model"),
                                "adc-cloud-platform".to_string(),
                                token.clone(),
                                regional_native_url,
                                slug_model.clone(),
                            )
                            .await,
                        );
                    }
                }
            }
            _ => eprintln!(
                "skipping Vertex endpoint probes: missing GOOGLE_PROJECT_ID or Google ADC bearer auth"
            ),
        }

        if let Some((key_env, key)) = google_ai_studio_api_key() {
            let openai_url =
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                    .to_string();
            for (model_label, model) in [
                ("slug-model", slug_model.clone()),
                ("google-prefix-model", full_model.clone()),
            ] {
                candidates += 1;
                reports.push(
                    send_endpoint_probe(
                        format!("ai-studio-openai-{model_label}"),
                        format!("api-key-bearer:{key_env}"),
                        key.clone(),
                        openai_url.clone(),
                        model,
                    )
                    .await,
                );
            }

            let native_url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{slug_model}:generateContent"
            );
            candidates += 1;
            reports.push(
                send_native_api_key_probe(
                    "ai-studio-native-slug-model".to_string(),
                    format!("api-key-query:{key_env}"),
                    key,
                    native_url,
                    slug_model.clone(),
                )
                .await,
            );
        } else {
            eprintln!(
                "skipping AI Studio endpoint probes: set PLOKE_GOOGLE_AI_STUDIO_API_KEY, GEMINI_API_KEY, or GOOGLE_API_KEY"
            );
        }

        if candidates == 0 {
            let message = "no Google endpoint probe candidates were configured";
            if strict_live_tests_requested() {
                bail!(message);
            }
            eprintln!(
                "skipping live_google_endpoint_matrix_reports_vertex_vs_ai_studio_status: {message}"
            );
            return Ok(());
        }

        println!(
            "{}",
            serde_json::to_string_pretty(&reports).expect("serialize endpoint probe reports")
        );

        if env::var("PLOKE_LIVE_GOOGLE_ENDPOINT_MATRIX_REQUIRE_SUCCESS")
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        {
            assert!(
                reports.iter().any(|report| report
                    .status
                    .is_some_and(|status| (200..300).contains(&status))),
                "expected at least one Google endpoint/model candidate to return HTTP success; reports={reports:#?}"
            );
        }

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    #[ignore = "requires Google ADC, GOOGLE_PROJECT_ID, GOOGLE_REGION, a live Google model with tool support, and quota"]
    async fn live_google_chat_step_forced_tool_call_success_or_quota() -> Result<()> {
        const TEST_NAME: &str = "live_google_chat_step_forced_tool_call_success_or_quota";
        if !live_google_env_or_skip(TEST_NAME) {
            return Ok(());
        }

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
        const TEST_NAME: &str = "live_google_thinking_request_returns_reasoning_success_or_quota";
        if !live_google_env_or_skip(TEST_NAME) {
            return Ok(());
        }

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
            .with_model_str(&live_thinking_model())?
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
            "expected assistant content in Google thinking response"
        );
        assert!(
            response_has_thinking_evidence(&response_value),
            "expected Google thinking evidence when include_thoughts=true"
        );

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "live_api_tests")]
    async fn live_google_cached_content_reaches_provider_as_rejected_resource() -> Result<()> {
        const TEST_NAME: &str = "live_google_cached_content_reaches_provider_as_rejected_resource";
        if !live_google_env_or_skip(TEST_NAME) {
            return Ok(());
        }

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
