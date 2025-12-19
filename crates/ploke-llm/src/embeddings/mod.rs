use std::time::Duration;

use color_eyre::eyre::eyre;
use serde::{Deserialize, Serialize};

use crate::{
    HTTP_REFERER, HTTP_TITLE, ModelId, Router,
    request::models,
    router_only::{ApiRoute, HasModelId},
    types::model_types::serialize_model_id_as_request_string,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EmbClientConfig {
    referer: &'static str,
    title: &'static str,
    timeout: Duration,
}
impl Default for EmbClientConfig {
    fn default() -> Self {
        Self {
            referer: HTTP_REFERER,
            title: HTTP_TITLE,
            // NOTE:ploke-llm 2025-12-14
            // Setting to 15 secs for now, try using it and getting a feel for the right default
            // timing
            timeout: Duration::from_secs(15),
        }
    }
}

pub trait HasDims {
    fn dims(&self) -> Option<u64>;
}

pub trait HasEmbeddings: Router {
    /// Router-specific fields for the `/embeddings` request body (often empty).
    type EmbeddingFields: ApiRoute + Serialize + Default;

    /// Typed response for the `/embeddings` endpoint.
    type EmbeddingsResponse: for<'a> Deserialize<'a> + Send + Sync + HasDims;

    type Error;

    /// Full URL for POST embeddings.
    const EMBEDDINGS_URL: &str;

    fn default_embeddings() -> EmbeddingRequest<Self>
    where
        Self: Sized,
    {
        EmbeddingRequest::<Self> {
            router: Self::EmbeddingFields::default(),
            ..Default::default()
        }
    }

    fn fetch_embeddings(
        client: &reqwest::Client,
        req: &EmbeddingRequest<Self>,
    ) -> impl std::future::Future<Output = color_eyre::Result<Self::EmbeddingsResponse>> + Send
    where
        Self: Sized,
        <Self as HasEmbeddings>::EmbeddingFields: std::marker::Sync,
    {
        async move {
            let api_key = Self::resolve_api_key()?;

            let resp = client
                .post(Self::EMBEDDINGS_URL)
                .bearer_auth(api_key)
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
                .header("X-Title", "Ploke TUI")
                .json(req)
                .send()
                .await?
                .error_for_status()?;

            Ok(resp.json::<Self::EmbeddingsResponse>().await?)
        }
    }

    fn fetch_validate_dims(
        client: &reqwest::Client,
        req: &EmbeddingRequest<Self>,
    ) -> impl std::future::Future<Output = color_eyre::Result<u64>> + Send
    where
        Self: Sized,
        <Self as HasEmbeddings>::EmbeddingFields: std::marker::Sync,
    {
        async move {
            let resp = <Self as HasEmbeddings>::fetch_embeddings(client, req).await?;
            let dims = resp
                .dims()
                .ok_or_else(|| eyre!("Received empty vector embedding"))?;
            // .map_err(|e| {
            //     LlmError::Embedding(e.wrap_err("invalid embedding response"))
            // })?;
            Ok(dims)
        }
    }
}

pub trait HasEmbeddingModels: Router {
    type Response: for<'a> Deserialize<'a> + IntoIterator<Item = Self::Models>;
    type Models: for<'a> Deserialize<'a> + HasModelId + Into<models::ResponseItem>;
    type Error;

    /// Full URL for GET embeddings model list.
    const EMBEDDING_MODELS_URL: &str;

    fn fetch_embedding_models(
        client: &reqwest::Client,
        cfg: EmbClientConfig,
    ) -> impl std::future::Future<Output = color_eyre::Result<Self::Response>> + Send {
        async move {
            let api_key = Self::resolve_api_key()?;

            let resp = client
                .get(Self::EMBEDDING_MODELS_URL)
                .bearer_auth(api_key)
                .header("Accept", "application/json")
                .header("HTTP-Referer", cfg.referer)
                .header("X-Title", cfg.title)
                .timeout(cfg.timeout)
                .send()
                .await?
                .error_for_status()?;

            Ok(resp.json::<Self::Response>().await?)
        }
    }
}

// TODO:ploke-llm:someday 2025-12-14
// Add support for non-text embedding
// (OpenRouter supports images and more structured ways to represent the content sent as well)
// - see https://openrouter.ai/docs/api/api-reference/embeddings/create-embeddings
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct EmbeddingRequest<R>
where
    R: HasEmbeddings,
    R::EmbeddingFields: ApiRoute + Serialize,
{
    #[serde(serialize_with = "serialize_model_id_as_request_string")]
    pub model: ModelId,

    #[serde(rename = "input")]
    pub input: EmbeddingInput,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<EmbeddingEncodingFormat>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    #[serde(flatten)]
    pub router: R::EmbeddingFields,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum EmbeddingInput {
    Single(String),
    Batch(Vec<String>),
}

impl Default for EmbeddingInput {
    fn default() -> Self {
        Self::Single(String::new())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, Copy)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingEncodingFormat {
    #[default]
    Float,
    Base64,
}

impl<R> EmbeddingRequest<R>
where
    R: HasEmbeddings,
    R::EmbeddingFields: ApiRoute + Serialize,
{
    pub fn with_model(mut self, model: ModelId) -> Self {
        self.model = model;
        self
    }

    pub fn with_input(mut self, input: EmbeddingInput) -> Self {
        self.input = input;
        self
    }

    // if empty input, returns an empty batch (for now)
    pub fn with_texts<I, S>(mut self, texts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut it = texts.into_iter();

        match it.next() {
            None => {
                self.input = EmbeddingInput::Batch(Vec::new());
            }
            Some(first) => {
                let first = first.into();

                // Only allocate a Vec if there's at least a second item.
                match it.next() {
                    None => {
                        self.input = EmbeddingInput::Single(first);
                    }
                    Some(second) => {
                        let mut v = Vec::new();
                        v.push(first);
                        v.push(second.into());
                        v.extend(it.map(Into::into));
                        self.input = EmbeddingInput::Batch(v);
                    }
                }
            }
        }

        self
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn with_encoding_format(mut self, f: EmbeddingEncodingFormat) -> Self {
        self.encoding_format = Some(f);
        self
    }

    pub fn with_router_bundle(mut self, router: R::EmbeddingFields) -> Self {
        self.router = router;
        self
    }
}

#[cfg(test)]
mod embedding_request_generic_field_tests {
    use serde::Serialize;
    use serde_json::json;

    use crate::{
        ModelId,
        embeddings::{EmbeddingEncodingFormat, EmbeddingInput, EmbeddingRequest},
        router_only::openrouter::{OpenRouter, embed::OpenRouterEmbeddingFields},
    };

    fn req(input: EmbeddingInput) -> EmbeddingRequest<OpenRouter> {
        EmbeddingRequest {
            input,
            model: "openai/text-embedding-3-small".parse::<ModelId>().unwrap(),
            encoding_format: None,
            user: None,
            router: OpenRouterEmbeddingFields::default(),
        }
    }

    fn assert_json_eq(got: &impl Serialize, expected: serde_json::Value) {
        let got = serde_json::to_value(got).unwrap();
        assert_eq!(got, expected);
    }

    // --- input ---
    #[test]
    fn embedding_field_input_single() {
        let r = req(EmbeddingInput::Single("hello".into()));
        assert_json_eq(
            &r,
            json!({
                "input": "hello",
                "model": "openai/text-embedding-3-small"
            }),
        );
    }

    #[test]
    fn embedding_field_input_batch() {
        let r = req(EmbeddingInput::Batch(vec!["a".into(), "b".into()]));
        assert_json_eq(
            &r,
            json!({
                "input": ["a", "b"],
                "model": "openai/text-embedding-3-small"
            }),
        );
    }

    // --- model ---
    #[test]
    fn embedding_field_model() {
        let mut r = req(EmbeddingInput::Single("x".into()));
        r.model = "qwen/qwen3-embedding-8b".parse::<ModelId>().unwrap();
        assert_json_eq(
            &r,
            json!({
                "input": "x",
                "model": "qwen/qwen3-embedding-8b"
            }),
        );
    }

    // --- encoding_format ---
    #[test]
    fn embedding_field_encoding_format_omitted_when_none() {
        let r = req(EmbeddingInput::Single("x".into()));
        assert_json_eq(
            &r,
            json!({
                "input": "x",
                "model": "openai/text-embedding-3-small"
            }),
        );
    }

    #[test]
    fn embedding_field_encoding_format_base64() {
        let mut r = req(EmbeddingInput::Single("x".into()));
        r.encoding_format = Some(EmbeddingEncodingFormat::Base64);
        assert_json_eq(
            &r,
            json!({
                "input": "x",
                "model": "openai/text-embedding-3-small",
                "encoding_format": "base64"
            }),
        );
    }

    // --- user ---
    #[test]
    fn embedding_field_user_included_when_some() {
        let mut r = req(EmbeddingInput::Single("x".into()));
        r.user = Some("user_123".into());
        assert_json_eq(
            &r,
            json!({
                "input": "x",
                "model": "openai/text-embedding-3-small",
                "user": "user_123"
            }),
        );
    }
}
