use std::str::FromStr;

use ploke_core::ArcStr;
use serde::{Deserialize, Serialize};

use crate::{
    EmbeddingModelName, EmbeddingResponseId, IdError, InputModality, LlmError, Modality, ModelId,
    ModelKey, ModelName, ModelSlug, OutputModality, SupportedParameters, Tokenizer,
    embeddings::{HasDims, HasEmbeddingModels, HasEmbeddings},
    request::{ModelPricing, models, models::ModelRouteSource},
    router_only::{HasModelId, openrouter::TopProvider},
    types::{model_types::Architecture, model_types::serialize_model_id_as_request_string},
};

use super::{ApiRoute, EndpointKey, HasModels, Router, RouterModelId, RouterVariants};

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Default, Hash, Eq)]
pub struct Nebius;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NebiusModelsResponse {
    pub data: Vec<NebiusModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ArcStr>,
}

impl IntoIterator for NebiusModelsResponse {
    type Item = NebiusModel;
    type IntoIter = std::vec::IntoIter<NebiusModel>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NebiusModel {
    pub id: ModelId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ArcStr>,
    #[serde(default)]
    pub created: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owned_by: Option<ArcStr>,
}

impl HasModelId for NebiusModel {
    fn model_id(&self) -> ModelId {
        self.id.clone()
    }
}

impl From<NebiusModel> for models::ResponseItem {
    fn from(model: NebiusModel) -> Self {
        let model_id = model.id.clone();
        let slug = model.id.key.slug.clone();
        let supported_parameters = supported_parameters_for_nebius_model(&slug);

        Self {
            id: model_id.clone(),
            name: ModelName::new(slug.as_str()),
            created: model.created.unwrap_or_default(),
            description: ArcStr::from("Direct Nebius Token Factory catalog row."),
            architecture: nebius_architecture(&slug),
            top_provider: TopProvider::default(),
            pricing: unknown_pricing(),
            canonical: Some(model_id),
            context_length: nebius_context_length(&slug),
            hugging_face_id: None,
            per_request_limits: None,
            supported_parameters,
            route_source: ModelRouteSource::DirectNebius,
        }
    }
}

impl HasModels for Nebius {
    type Response = NebiusModelsResponse;
    type Models = NebiusModel;
    type Error = LlmError;
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

fn nebius_architecture(slug: &ModelSlug) -> Architecture {
    if is_nebius_embedding_model(slug.as_str()) {
        return Architecture {
            input_modalities: vec![InputModality::Text],
            modality: Modality::TextToEmbeddings,
            output_modalities: vec![OutputModality::Embeddings],
            tokenizer: tokenizer_for_nebius_model(slug.as_str()),
            instruct_type: None,
        };
    }

    Architecture {
        input_modalities: vec![InputModality::Text],
        modality: Modality::TextToText,
        output_modalities: vec![OutputModality::Text],
        tokenizer: tokenizer_for_nebius_model(slug.as_str()),
        instruct_type: None,
    }
}

fn tokenizer_for_nebius_model(slug: &str) -> Tokenizer {
    let lower = slug.to_ascii_lowercase();
    if lower.contains("llama-3") {
        Tokenizer::Llama3
    } else if lower.contains("qwen3") {
        Tokenizer::Qwen3
    } else if lower.contains("qwen") {
        Tokenizer::Qwen
    } else if lower.contains("gemma") {
        Tokenizer::Gemma
    } else if lower.contains("mistral") {
        Tokenizer::Mistral
    } else if lower.contains("deepseek") {
        Tokenizer::DeepSeek
    } else {
        Tokenizer::Other
    }
}

fn supported_parameters_for_nebius_model(slug: &ModelSlug) -> Option<Vec<SupportedParameters>> {
    if !is_nebius_chat_model(slug.as_str()) {
        return None;
    }

    Some(vec![
        SupportedParameters::FrequencyPenalty,
        SupportedParameters::LogitBias,
        SupportedParameters::MaxCompletionTokens,
        SupportedParameters::MaxTokens,
        SupportedParameters::PresencePenalty,
        SupportedParameters::ResponseFormat,
        SupportedParameters::Seed,
        SupportedParameters::Stop,
        SupportedParameters::Temperature,
        SupportedParameters::ToolChoice,
        SupportedParameters::Tools,
        SupportedParameters::TopP,
    ])
}

fn is_nebius_chat_model(slug: &str) -> bool {
    !is_nebius_embedding_model(slug) && !is_nebius_rerank_model(slug)
}

fn is_nebius_embedding_model(slug: &str) -> bool {
    let lower = slug.to_ascii_lowercase();
    lower.contains("embed") || lower.contains("bge-multilingual")
}

fn is_nebius_rerank_model(slug: &str) -> bool {
    slug.to_ascii_lowercase().contains("rerank")
}

fn nebius_context_length(slug: &ModelSlug) -> Option<u32> {
    match slug.as_str() {
        "Meta-Llama-3.1-70B-Instruct" => Some(131_072),
        _ => None,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NebiusChatCompFields {
    /// OpenAI-compatible chat completion storage toggle documented by Nebius Token Factory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    /// Nebius exposes both `max_tokens` and `max_completion_tokens` on chat completions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    /// Stable end-user identifier accepted by the OpenAI-compatible request body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Provider-specific extension point shown as `extra_body` in the Nebius docs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_body: Option<serde_json::Value>,
    /// Nebius service tier setting; docs currently show `auto`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<NebiusServiceTier>,
}

impl NebiusChatCompFields {
    pub fn with_store(mut self, store: bool) -> Self {
        self.store = Some(store);
        self
    }

    pub fn with_max_completion_tokens(mut self, max_completion_tokens: u32) -> Self {
        self.max_completion_tokens = Some(max_completion_tokens);
        self
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn with_extra_body(mut self, extra_body: serde_json::Value) -> Self {
        self.extra_body = Some(extra_body);
        self
    }

    pub fn with_service_tier(mut self, service_tier: NebiusServiceTier) -> Self {
        self.service_tier = Some(service_tier);
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NebiusServiceTier {
    #[default]
    Auto,
}

impl Nebius {
    pub const COMPLETIONS_URL: &'static str = "https://api.tokenfactory.nebius.com/v1/completions";
    pub const CHAT_COMPLETIONS_URL: &'static str =
        "https://api.tokenfactory.nebius.com/v1/chat/completions";
    pub const EMBEDDINGS_URL: &'static str = "https://api.tokenfactory.nebius.com/v1/embeddings";
    pub const RERANK_URL: &'static str = "https://api.tokenfactory.nebius.com/v1/rerank";
    pub const RESPONSES_URL: &'static str = "https://api.tokenfactory.nebius.com/v1/responses";
    pub const GENERATE_URL: &'static str = "https://api.tokenfactory.nebius.com/v1/generate";
    pub const GENERATE_MODELS_URL: &'static str =
        "https://api.tokenfactory.nebius.com/v1/generate/models";
    pub const FILES_URL: &'static str = "https://api.tokenfactory.nebius.com/v1/files";
    pub const FINE_TUNING_JOBS_URL: &'static str =
        "https://api.tokenfactory.nebius.com/v1/fine-tuning/jobs";
    pub const DATASETS_URL: &'static str = "https://api.tokenfactory.nebius.com/v1/datasets";
    pub const DEDICATED_ENDPOINTS_URL: &'static str =
        "https://api.tokenfactory.nebius.com/v1/dedicated-endpoints";
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusCompletionRequest {
    #[serde(serialize_with = "serialize_model_id_as_request_string")]
    pub model: ModelId,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusResponsesRequest {
    #[serde(serialize_with = "serialize_model_id_as_request_string")]
    pub model: ModelId,
    pub input: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusGenerateRequest {
    #[serde(serialize_with = "serialize_model_id_as_request_string")]
    pub model: ModelId,
    pub prompt: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusRerankRequest {
    #[serde(serialize_with = "serialize_model_id_as_request_string")]
    pub model: ModelId,
    pub query: String,
    pub documents: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_documents: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NebiusRerankResponse {
    pub results: Vec<NebiusRerankResult>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NebiusRerankResult {
    pub index: u32,
    pub relevance_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusFileMetadata {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ArcStr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusFineTuningJob {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusDataset {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusDedicatedEndpoint {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusObjectResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ArcStr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<NebiusUsage>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NebiusUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
}

impl ApiRoute for NebiusChatCompFields {
    type Parent = Nebius;
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize, Hash, Eq)]
pub struct NebiusModelId(ModelKey);

impl From<ModelId> for NebiusModelId {
    fn from(id: ModelId) -> Self {
        Self(id.key)
    }
}

impl From<EndpointKey> for NebiusModelId {
    fn from(key: EndpointKey) -> Self {
        Self(key.model)
    }
}

impl RouterModelId for NebiusModelId {
    fn into_key(self) -> ModelKey {
        self.0
    }

    fn key(&self) -> &ModelKey {
        &self.0
    }

    fn into_url_format(self) -> String {
        format!("{}/{}", self.0.author.as_str(), self.0.slug.as_str())
    }

    fn model_id_from_request_string(model: &str) -> Result<ModelId, IdError> {
        ModelId::from_str(model)
    }
}

impl Router for Nebius {
    type CompletionFields = NebiusChatCompFields;
    type RouterModelId = NebiusModelId;

    const BASE_URL: &str = "https://api.tokenfactory.nebius.com/v1";
    const COMPLETION_URL: &str = "https://api.tokenfactory.nebius.com/v1/chat/completions";
    const MODELS_URL: &str = "https://api.tokenfactory.nebius.com/v1/models";
    const ENDPOINTS_TAIL: &str = "";
    const API_KEY_NAME: &str = "NEBIUS_API_KEY";
    const PROVIDERS_URL: &str = "";

    fn resolve_api_key() -> Result<String, LlmError> {
        let token = std::env::var(Self::API_KEY_NAME).map_err(LlmError::from)?;
        if token.trim().is_empty() {
            return Err(LlmError::Var {
                message: "required Nebius bearer token environment variable is empty",
                original: Self::API_KEY_NAME.to_string(),
            });
        }
        Ok(token)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusEmbeddingFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
}

impl NebiusEmbeddingFields {
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    pub fn with_input_type(mut self, input_type: impl Into<String>) -> Self {
        self.input_type = Some(input_type.into());
        self
    }
}

impl ApiRoute for NebiusEmbeddingFields {
    type Parent = Nebius;
}

impl HasEmbeddings for Nebius {
    type EmbeddingFields = NebiusEmbeddingFields;
    type EmbeddingsResponse = NebiusEmbeddingsResponse;
    type Error = LlmError;

    const EMBEDDINGS_URL: &str = Nebius::EMBEDDINGS_URL;
}

impl HasEmbeddingModels for Nebius {
    type Response = NebiusModelsResponse;
    type Models = NebiusModel;
    type Error = LlmError;

    const EMBEDDING_MODELS_URL: &str = Nebius::MODELS_URL;
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct NebiusEmbeddingsResponse {
    pub data: Vec<NebiusEmbeddingsData>,
    pub model: EmbeddingModelName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ArcStr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<EmbeddingResponseId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<NebiusUsage>,
}

impl HasDims for NebiusEmbeddingsResponse {
    fn dims(&self) -> Option<u64> {
        self.data.first().and_then(|data| match &data.embedding {
            NebiusEmbeddingVector::Float(items) => Some(items.len() as u64),
            NebiusEmbeddingVector::Base64(_) => None,
        })
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct NebiusEmbeddingsData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ArcStr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    pub embedding: NebiusEmbeddingVector,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum NebiusEmbeddingVector {
    Float(Vec<f64>),
    Base64(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusChatCompletionResponse {
    #[serde(flatten)]
    pub base: NebiusObjectResponse,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusCompletionResponse {
    #[serde(flatten)]
    pub base: NebiusObjectResponse,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusResponsesResponse {
    #[serde(flatten)]
    pub base: NebiusObjectResponse,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusGenerateResponse {
    #[serde(flatten)]
    pub base: NebiusObjectResponse,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NebiusListResponse<T> {
    pub data: Vec<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ArcStr>,
}

impl<T> IntoIterator for NebiusListResponse<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl TryFrom<RouterVariants> for Nebius {
    type Error = LlmError;

    fn try_from(value: RouterVariants) -> Result<Self, Self::Error> {
        match value {
            RouterVariants::Nebius(_) => Ok(Self),
            RouterVariants::OpenRouter(_) => Err(LlmError::Conversion(String::from(
                "Invalid conversion from OpenRouter to Nebius",
            ))),
            RouterVariants::Anthropic(_) => Err(LlmError::Conversion(String::from(
                "Invalid conversion from Anthropic to Nebius",
            ))),
            RouterVariants::Google(_) => Err(LlmError::Conversion(String::from(
                "Invalid conversion from Google to Nebius",
            ))),
        }
    }
}

impl From<Nebius> for RouterVariants {
    fn from(value: Nebius) -> Self {
        Self::Nebius(value)
    }
}

#[cfg(test)]
mod tests {
    use super::Nebius;
    use crate::{
        Router, SupportsTools,
        embeddings::{
            EmbeddingEncodingFormat, EmbeddingInput, EmbeddingRequest, HasDims, HasEmbeddings,
        },
        router_only::{HasModelId, HasModels},
    };
    use serde_json::json;

    #[cfg(feature = "live_api_tests")]
    use crate::{ChatHttpConfig, ChatStepOutcome, manager::RequestMessage};
    #[cfg(feature = "live_api_tests")]
    use color_eyre::Result;
    #[cfg(feature = "live_api_tests")]
    use reqwest::Client;
    #[cfg(feature = "live_api_tests")]
    use std::env;

    #[test]
    fn nebius_models_response_deserializes_and_adapts_to_direct_registry_item() {
        let response: <Nebius as HasModels>::Response = serde_json::from_value(json!({
            "object": "list",
            "data": [
                {
                    "id": "meta-llama/Meta-Llama-3.1-70B-Instruct",
                    "object": "model",
                    "created": 1710000000,
                    "owned_by": "nebius"
                }
            ]
        }))
        .expect("Nebius OpenAI-compatible models response parses");

        let model = response.into_iter().next().expect("one model");
        assert_eq!(
            model.model_id().to_string(),
            "meta-llama/Meta-Llama-3.1-70B-Instruct"
        );

        let item: crate::request::models::ResponseItem = model.into();
        assert_eq!(
            item.id.to_string(),
            "meta-llama/Meta-Llama-3.1-70B-Instruct"
        );
        assert_eq!(item.name.as_str(), "Meta-Llama-3.1-70B-Instruct");
        assert!(item.supports_tools());
        assert!(item.route_source.is_direct_nebius());
        assert_eq!(item.canonical.as_ref(), Some(&item.id));
    }

    #[test]
    fn nebius_embedding_model_adapter_does_not_advertise_tool_support() {
        let response: <Nebius as HasModels>::Response = serde_json::from_value(json!({
            "object": "list",
            "data": [
                {
                    "id": "BAAI/bge-multilingual-gemma2",
                    "object": "model",
                    "owned_by": "nebius"
                }
            ]
        }))
        .expect("Nebius OpenAI-compatible models response parses");

        let model = response.into_iter().next().expect("one model");
        let item: crate::request::models::ResponseItem = model.into();

        assert_eq!(item.id.to_string(), "BAAI/bge-multilingual-gemma2");
        assert!(!item.supports_tools());
        assert!(item.route_source.is_direct_nebius());
    }

    #[test]
    fn nebius_embeddings_request_serializes_openai_compatible_fields() {
        let request = EmbeddingRequest::<Nebius> {
            model: "BAAI/bge-multilingual-gemma2".parse().expect("model id"),
            input: EmbeddingInput::Single("hello".to_string()),
            encoding_format: Some(EmbeddingEncodingFormat::Float),
            user: Some("stable-user".to_string()),
            router: super::NebiusEmbeddingFields::default()
                .with_dimensions(1024)
                .with_input_type("query"),
        };

        assert_eq!(
            serde_json::to_value(&request).expect("serialize request"),
            json!({
                "model": "BAAI/bge-multilingual-gemma2",
                "input": "hello",
                "encoding_format": "float",
                "user": "stable-user",
                "dimensions": 1024,
                "input_type": "query"
            })
        );
    }

    #[test]
    fn nebius_embeddings_response_reports_vector_dims() {
        let response: <Nebius as HasEmbeddings>::EmbeddingsResponse =
            serde_json::from_value(json!({
                "object": "list",
                "model": "BAAI/bge-multilingual-gemma2",
                "data": [
                    { "object": "embedding", "index": 0, "embedding": [0.1, 0.2, 0.3] }
                ],
                "usage": { "prompt_tokens": 2, "total_tokens": 2 }
            }))
            .expect("Nebius embeddings response parses");

        assert_eq!(response.dims(), Some(3));
        assert_eq!(response.data.len(), 1);
        assert_eq!(
            response.usage.as_ref().map(|usage| usage.total_tokens),
            Some(2)
        );
    }

    #[test]
    fn nebius_endpoint_constants_cover_documented_safe_paths() {
        assert_eq!(Nebius::BASE_URL, "https://api.tokenfactory.nebius.com/v1");
        assert_eq!(
            Nebius::COMPLETION_URL,
            "https://api.tokenfactory.nebius.com/v1/chat/completions"
        );
        assert_eq!(
            Nebius::EMBEDDINGS_URL,
            "https://api.tokenfactory.nebius.com/v1/embeddings"
        );
        assert_eq!(
            Nebius::RERANK_URL,
            "https://api.tokenfactory.nebius.com/v1/rerank"
        );
        assert_eq!(
            Nebius::RESPONSES_URL,
            "https://api.tokenfactory.nebius.com/v1/responses"
        );
        assert_eq!(
            Nebius::FILES_URL,
            "https://api.tokenfactory.nebius.com/v1/files"
        );
        assert_eq!(
            Nebius::FINE_TUNING_JOBS_URL,
            "https://api.tokenfactory.nebius.com/v1/fine-tuning/jobs"
        );
        assert_eq!(
            Nebius::DATASETS_URL,
            "https://api.tokenfactory.nebius.com/v1/datasets"
        );
        assert_eq!(
            Nebius::DEDICATED_ENDPOINTS_URL,
            "https://api.tokenfactory.nebius.com/v1/dedicated-endpoints"
        );
    }

    #[test]
    fn nebius_rerank_skeleton_serializes_safe_json() {
        let request = super::NebiusRerankRequest {
            model: "BAAI/bge-reranker-v2-m3".parse().expect("model id"),
            query: "rust tests".to_string(),
            documents: vec!["TDD first".to_string(), "Implement later".to_string()],
            top_n: Some(1),
            return_documents: Some(false),
        };

        assert_eq!(
            serde_json::to_value(request).expect("serialize rerank skeleton"),
            json!({
                "model": "BAAI/bge-reranker-v2-m3",
                "query": "rust tests",
                "documents": ["TDD first", "Implement later"],
                "top_n": 1,
                "return_documents": false
            })
        );
    }

    #[cfg(feature = "live_api_tests")]
    fn strict_nebius_live_tests_requested() -> bool {
        env::var("PLOKE_RUN_LIVE_TESTS")
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
    }

    #[cfg(feature = "live_api_tests")]
    fn live_nebius_env_or_skip(test_name: &str) -> bool {
        let api_key_available = env::var(Nebius::API_KEY_NAME)
            .ok()
            .is_some_and(|value| !value.trim().is_empty());
        if api_key_available && strict_nebius_live_tests_requested() {
            return true;
        }

        let message = if api_key_available {
            format!(
                "skipping {test_name}: PLOKE_RUN_LIVE_TESTS is not enabled; direct Nebius live route was not exercised"
            )
        } else {
            format!(
                "skipping {test_name}: missing {}; direct Nebius live route was not exercised",
                Nebius::API_KEY_NAME
            )
        };
        if strict_nebius_live_tests_requested() {
            panic!("{message}; PLOKE_RUN_LIVE_TESTS requested live execution");
        }
        eprintln!("{message}");
        false
    }

    #[cfg(feature = "live_api_tests")]
    fn live_nebius_chat_model() -> String {
        env::var("PLOKE_LIVE_NEBIUS_CHAT_MODEL")
            .unwrap_or_else(|_| "meta-llama/Meta-Llama-3.1-70B-Instruct".to_string())
    }

    #[cfg(feature = "live_api_tests")]
    #[tokio::test]
    async fn nebius_live_models_smoke() -> Result<()> {
        if !live_nebius_env_or_skip("nebius_live_models_smoke") {
            return Ok(());
        }

        let response = Nebius::fetch_models(&Client::new()).await?;
        let models: Vec<_> = response.into_iter().collect();
        assert!(
            !models.is_empty(),
            "Nebius models response should not be empty"
        );
        assert!(
            models
                .iter()
                .any(|model| !model.model_id().to_string().is_empty()),
            "Nebius models response should contain non-empty model ids"
        );
        Ok(())
    }

    #[cfg(feature = "live_api_tests")]
    #[tokio::test]
    async fn nebius_live_chat_completion_smoke() -> Result<()> {
        if !live_nebius_env_or_skip("nebius_live_chat_completion_smoke") {
            return Ok(());
        }

        let model = live_nebius_chat_model();
        let request = Nebius::default_chat_completion()
            .with_model_str(&model)?
            .with_message(RequestMessage::new_user("Reply exactly: ok".to_string()))
            .with_max_tokens(8)
            .with_temperature(0.0);
        let step = crate::chat_step(&Client::new(), &request, &ChatHttpConfig::default()).await?;

        let content = match step.outcome {
            ChatStepOutcome::Content { content, .. } => content,
            ChatStepOutcome::ToolCalls { content, .. } => content,
        }
        .expect("Nebius live chat response should contain content");
        assert!(
            content.to_ascii_lowercase().contains("ok"),
            "expected Nebius live chat response to contain 'ok', got: {content}"
        );
        Ok(())
    }
}
