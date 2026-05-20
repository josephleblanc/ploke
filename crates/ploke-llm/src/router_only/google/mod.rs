//! src/google.rs

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
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct ExtraBody {
    pub thinking_config: ThinkingConfig,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct ThinkingConfig {
    thinking_level: ThinkingLevel,
    include_thoughts: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
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
        self.0.to_string()
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
