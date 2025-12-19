use crate::{
    EmbeddingModelName, EmbeddingResponseId, LlmError, ModelId,
    embeddings::{EmbeddingRequest, HasDims, HasEmbeddingModels, HasEmbeddings},
    router_only::{ApiRoute, Router, openrouter::EmbeddingProviderPrefs},
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

impl EmbeddingModelName {
    /// OpenRouter may echo only the slug (e.g., `text-embedding-3-small`) even if the request used
    /// a fully qualified `author/slug[:variant]`. Accept any of the observed forms.
    pub fn matches_request(&self, requested: &ModelId) -> bool {
        let raw = self.as_str();
        raw == requested.to_request_string()
            || raw == requested.to_string()
            || raw == requested.key.to_string()
            || raw == requested.key.slug.as_str()
    }
}

impl HasEmbeddings for super::OpenRouter {
    type EmbeddingFields = OpenRouterEmbeddingFields;
    type EmbeddingsResponse = OpenRouterEmbeddingsResponse;
    type Error = LlmError;

    const EMBEDDINGS_URL: &str = "https://openrouter.ai/api/v1/embeddings";

    #[tracing::instrument(skip(client), fields(req), target = "embed-pipeline")]
    async fn fetch_embeddings(
        client: &reqwest::Client,
        req: &EmbeddingRequest<Self>,
    ) -> color_eyre::Result<Self::EmbeddingsResponse>
    where
        Self: Sized,
        <Self as HasEmbeddings>::EmbeddingFields: std::marker::Sync,
    {
        let env = OpenRouterEmbedEnv::from_env()?;
        Self::fetch_embeddings_with_env(client, req, &env).await
    }
}

/// Authentication + endpoint details for hitting OpenRouter embeddings.
#[derive(Clone, Debug)]
pub struct OpenRouterEmbedEnv {
    pub api_key: String,
    pub embeddings_url: String,
}

impl OpenRouterEmbedEnv {
    pub fn from_env() -> Result<Self, std::env::VarError> {
        let api_key = super::OpenRouter::resolve_api_key()?;
        Ok(Self {
            api_key,
            embeddings_url: std::env::var("OPENROUTER_EMBEDDINGS_URL")
                .unwrap_or_else(|_| super::OpenRouter::EMBEDDINGS_URL.to_string()),
        })
    }

    pub fn from_parts(api_key: impl Into<String>, embeddings_url: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            embeddings_url: embeddings_url.into(),
        }
    }

    pub fn from_overrides(
        api_key: Option<String>,
        embeddings_url: Option<String>,
    ) -> Result<Self, std::env::VarError> {
        let key = match api_key {
            Some(k) => k,
            None => super::OpenRouter::resolve_api_key()?,
        };
        let url = embeddings_url.unwrap_or_else(|| {
            std::env::var("OPENROUTER_EMBEDDINGS_URL")
                .unwrap_or_else(|_| super::OpenRouter::EMBEDDINGS_URL.to_string())
        });
        Ok(Self {
            api_key: key,
            embeddings_url: url,
        })
    }
}

impl super::OpenRouter {
    pub async fn fetch_embeddings_with_env(
        client: &reqwest::Client,
        req: &EmbeddingRequest<Self>,
        env: &OpenRouterEmbedEnv,
    ) -> color_eyre::Result<OpenRouterEmbeddingsResponse>
    where
        Self: Sized,
        <Self as HasEmbeddings>::EmbeddingFields: std::marker::Sync,
    {
        fn snippet_lossy(bytes: &[u8], max: usize) -> String {
            let end = bytes.len().min(max);
            String::from_utf8_lossy(&bytes[..end]).to_string()
        }

        let url = &env.embeddings_url;
        let resp = client
            .post(url)
            .bearer_auth(&env.api_key)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/ploke-ai/ploke")
            .header("X-Title", "Ploke TUI")
            .json(req)
            .send()
            .await
            .map_err(|e| OpenRouterEmbeddingError::Transport {
                message: e.to_string(),
                url: url.clone(),
            })?;

        let status = resp.status();
        let request_id = resp
            .headers()
            .get("x-request-id")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        if !status.is_success() {
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .map(Duration::from_secs);
            let body = resp.text().await.unwrap_or_default();
            let err = OpenRouterEmbeddingError::from_status(
                status,
                body,
                url.clone(),
                req.model.clone(),
                request_id,
                retry_after,
            );
            return Err(err.into());
        }

        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| OpenRouterEmbeddingError::Transport {
                message: e.to_string(),
                url: url.clone(),
            })?;

        let parsed = parse_openrouter_embeddings_response(
            &bytes,
            status.as_u16(),
            url,
            request_id.clone(),
            content_type,
        )
        .map_err(|e| {
            // Preserve the rich decode metadata when we can, so callers get actionable output.
            match e {
                OpenRouterEmbeddingError::Decode { .. } => e,
                other => other.with_body_snippet(snippet_lossy(&bytes, 8 * 1024)),
            }
        })?;

        Ok(parsed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenRouterErrorEnvelope {
    error: OpenRouterErrorPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenRouterErrorPayload {
    message: String,
    #[serde(default)]
    code: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum OpenRouterEmbeddingsEnvelope {
    Ok(OpenRouterEmbeddingsResponse),
    Err(OpenRouterErrorEnvelope),
}

fn parse_openrouter_embeddings_response(
    bytes: &[u8],
    http_status: u16,
    url: &str,
    request_id: Option<String>,
    content_type: Option<String>,
) -> Result<OpenRouterEmbeddingsResponse, OpenRouterEmbeddingError> {
    let envelope = serde_json::from_slice::<OpenRouterEmbeddingsEnvelope>(bytes).map_err(|e| {
        OpenRouterEmbeddingError::Decode {
            message: e.to_string(),
            url: url.to_string(),
            status: http_status,
            request_id: request_id.clone(),
            content_type: content_type.clone(),
            body_snippet: String::from_utf8_lossy(&bytes[..bytes.len().min(8 * 1024)]).to_string(),
        }
    })?;

    match envelope {
        OpenRouterEmbeddingsEnvelope::Ok(ok) => Ok(ok),
        OpenRouterEmbeddingsEnvelope::Err(err) => Err(OpenRouterEmbeddingError::ApiError {
            url: url.to_string(),
            http_status,
            api_code: err.error.code,
            request_id,
            content_type,
            message: err.error.message,
            body_snippet: None,
        }),
    }
}

impl HasEmbeddingModels for super::OpenRouter {
    type Response = crate::request::models::Response;
    type Models = crate::request::models::ResponseItem;
    type Error = LlmError;

    const EMBEDDING_MODELS_URL: &str = "https://openrouter.ai/api/v1/embeddings/models";
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
// AI: Please expand the data structure below to reflect the documentation in the
// `openrouter_docs.md` I am including in the conversation. You can see where the primary trait
// definition is located in `embeddings/mod.rs` as well, and I would like you to extend this
// structure to cover the remaining fields so we can correctly structure our request
pub struct OpenRouterEmbeddingFields {
    // encapsulating EmbeddingRequest<R> covers:
    // - input
    // - model
    // - encoding_format
    // - user
    /// OpenRouter supports `dimensions` for some embedding models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,

    /// OpenRouter supports `provider` routing for embeddings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<EmbeddingProviderPrefs>,

    /// Optional hint about the input type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
}

impl ApiRoute for OpenRouterEmbeddingFields {
    type Parent = super::OpenRouter;
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct OpenRouterEmbeddingsResponse {
    // pub object: ListMarker,
    pub data: Vec<OpenRouterEmbeddingsData>,
    pub model: EmbeddingModelName,
    // pub model: ModelId,
    pub id: Option<EmbeddingResponseId>,
    pub usage: Option<Usage>,
}

impl HasDims for OpenRouterEmbeddingsResponse {
    fn dims(&self) -> Option<u64> {
        use OpenRouterEmbeddingVector::*;
        self.data.first().map(|data| match &data.embedding {
            Float(items) => items.len() as u64,
            Base64(base_64) => {
                let len = base_64.len() / 4;
                len as u64
            }
        })
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct OpenRouterEmbeddingsData {
    // pub object: EmbeddingMarker,
    pub index: Option<f64>,
    pub embedding: OpenRouterEmbeddingVector,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum OpenRouterEmbeddingVector {
    Float(Vec<f64>),
    Base64(String),
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub struct Usage {
    prompt_tokens: f64,
    total_tokens: f64,
    cost: Option<f64>,
}

#[cfg(test)]
mod error_mapping_tests {
    use super::*;
    use crate::{
        embeddings::{EmbeddingInput, EmbeddingRequest},
        router_only::openrouter::OpenRouter,
    };
    use httpmock::prelude::*;
    use once_cell::sync::Lazy;
    use reqwest::Client;
    use std::str::FromStr;
    use tokio::sync::Mutex;

    static TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn base_request() -> EmbeddingRequest<OpenRouter> {
        EmbeddingRequest::<OpenRouter> {
            model: ModelId::from_str("openai/text-embedding-3-small").unwrap(),
            input: EmbeddingInput::Single("hello".into()),
            ..Default::default()
        }
    }

    fn test_env(url: &str) -> OpenRouterEmbedEnv {
        OpenRouterEmbedEnv::from_parts("test-key", url.to_string())
    }

    async fn expect_error<F>(status: u16, body: &str, assert: F)
    where
        F: Fn(&OpenRouterEmbeddingError),
    {
        let _guard = TEST_MUTEX.lock().await;
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(status).body(body);
        });
        let env = test_env(&server.url("/v1/embeddings"));
        let req = base_request();
        let err = OpenRouter::fetch_embeddings_with_env(&Client::new(), &req, &env)
            .await
            .expect_err("expected error");
        let kind = err
            .downcast_ref::<OpenRouterEmbeddingError>()
            .expect("typed openrouter embedding error");
        assert(kind);
    }

    #[tokio::test]
    async fn maps_bad_request() {
        expect_error(400, "bad input", |e| match e {
            OpenRouterEmbeddingError::BadRequest { detail, .. } => {
                assert_eq!(detail, "bad input");
            }
            other => panic!("unexpected error variant {other:?}"),
        })
        .await;
    }

    #[tokio::test]
    async fn maps_unauthorized() {
        expect_error(401, "unauthorized", |e| match e {
            OpenRouterEmbeddingError::Unauthorized { .. } => {}
            other => panic!("unexpected error variant {other:?}"),
        })
        .await;
    }

    #[tokio::test]
    async fn maps_payment_required() {
        expect_error(402, "payment", |e| match e {
            OpenRouterEmbeddingError::PaymentRequired { .. } => {}
            other => panic!("unexpected error variant {other:?}"),
        })
        .await;
    }

    #[tokio::test]
    async fn maps_not_found() {
        expect_error(404, "missing model", |e| match e {
            OpenRouterEmbeddingError::NotFound { model, .. } => {
                assert_eq!(model.to_string(), "openai/text-embedding-3-small");
            }
            other => panic!("unexpected error variant {other:?}"),
        })
        .await;
    }

    #[tokio::test]
    async fn maps_rate_limited_with_retry_after() {
        let _guard = TEST_MUTEX.lock().await;
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(429).header("Retry-After", "2").body("too many");
        });
        let env = test_env(&server.url("/v1/embeddings"));
        let req = base_request();
        let err = OpenRouter::fetch_embeddings_with_env(&Client::new(), &req, &env)
            .await
            .expect_err("expected rate limit error");
        let kind = err
            .downcast_ref::<OpenRouterEmbeddingError>()
            .expect("typed openrouter embedding error");
        match kind {
            OpenRouterEmbeddingError::RateLimited { retry_after, .. } => {
                assert_eq!(retry_after.as_ref().map(|d| d.as_secs()), Some(2));
            }
            other => panic!("unexpected error variant {other:?}"),
        }
    }

    #[tokio::test]
    async fn maps_provider_overloaded() {
        expect_error(529, "overloaded", |e| match e {
            OpenRouterEmbeddingError::ProviderOverloaded { .. } => {}
            other => panic!("unexpected error variant {other:?}"),
        })
        .await;
    }

    #[tokio::test]
    async fn maps_unexpected() {
        expect_error(500, "server error", |e| match e {
            OpenRouterEmbeddingError::Unexpected { status, body, .. } => {
                assert_eq!(*status, 500);
                assert_eq!(body, "server error");
            }
            other => panic!("unexpected error variant {other:?}"),
        })
        .await;
    }

    #[tokio::test]
    async fn parses_success() {
        let _guard = TEST_MUTEX.lock().await;
        let server = MockServer::start();
        let body = serde_json::json!({
            "data": [{
                "index": 0,
                "embedding": [0.1, 0.2, 0.3]
            }],
            "model": "text-embedding-3-small",
            "id": "req-123",
            "usage": {
                "prompt_tokens": 5.0,
                "total_tokens": 5.0,
                "cost": null
            }
        })
        .to_string();
        let _m = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(200).body(body);
        });
        let env = test_env(&server.url("/v1/embeddings"));
        let req = base_request();
        let resp = OpenRouter::fetch_embeddings_with_env(&Client::new(), &req, &env)
            .await
            .expect("success");
        assert_eq!(resp.data.len(), 1);
        assert!(resp.model.matches_request(&req.model));
        assert_eq!(resp.id.as_ref().map(|i| i.as_str()), Some("req-123"));
    }
}

#[derive(Debug, Error)]
pub enum OpenRouterEmbeddingError {
    #[error("invalid embedding request: {detail} (url={url})")]
    BadRequest { detail: String, url: String },
    #[error("unauthorized embedding request (url={url})")]
    Unauthorized { url: String },
    #[error("payment required for embeddings (url={url})")]
    PaymentRequired { url: String },
    #[error("embedding model not found: {model} (url={url})")]
    NotFound { model: ModelId, url: String },
    #[error("rate limited for embeddings (url={url}, retry_after={retry_after:?})")]
    RateLimited {
        url: String,
        retry_after: Option<Duration>,
    },
    #[error("provider overloaded (url={url})")]
    ProviderOverloaded { url: String },
    #[error("transport error for embeddings (url={url}): {message}")]
    Transport { message: String, url: String },
    #[error(
        "OpenRouter embedding API error (url={url}, http_status={http_status}, api_code={api_code:?}, request_id={request_id:?}, content_type={content_type:?}): {message}; body_snippet={body_snippet:?}"
    )]
    ApiError {
        url: String,
        http_status: u16,
        api_code: Option<u16>,
        request_id: Option<String>,
        content_type: Option<String>,
        message: String,
        body_snippet: Option<String>,
    },
    #[error(
        "error decoding embeddings response (url={url}, status={status}, request_id={request_id:?}, content_type={content_type:?}): {message}; body_snippet={body_snippet}"
    )]
    Decode {
        message: String,
        url: String,
        status: u16,
        request_id: Option<String>,
        content_type: Option<String>,
        body_snippet: String,
    },
    #[error("unexpected embedding error status={status} url={url} body={body}")]
    Unexpected {
        status: u16,
        url: String,
        body: String,
    },
}

impl OpenRouterEmbeddingError {
    fn with_body_snippet(mut self, snippet: String) -> Self {
        if let OpenRouterEmbeddingError::ApiError { body_snippet, .. } = &mut self {
            if body_snippet.is_none() {
                *body_snippet = Some(snippet);
            }
        }
        self
    }

    fn from_status(
        status: StatusCode,
        body: String,
        url: String,
        model: ModelId,
        _request_id: Option<String>,
        retry_after: Option<Duration>,
    ) -> Self {
        match status.as_u16() {
            400 => Self::BadRequest {
                detail: body.trim().to_string(),
                url,
            },
            401 => Self::Unauthorized { url },
            402 => Self::PaymentRequired { url },
            404 => Self::NotFound { model, url },
            429 => Self::RateLimited { url, retry_after },
            529 => Self::ProviderOverloaded { url },
            other => Self::Unexpected {
                status: other,
                url,
                body: body.trim().to_string(),
            },
        }
    }
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn parses_error_envelope_with_http_200() {
        let body = br#"{"error":{"message":"No successful provider responses.","code":404}}"#;
        let err = parse_openrouter_embeddings_response(
            body,
            200,
            "https://openrouter.ai/api/v1/embeddings",
            None,
            Some("application/json".to_string()),
        )
        .expect_err("should return api error");

        match err {
            OpenRouterEmbeddingError::ApiError {
                http_status,
                api_code,
                message,
                ..
            } => {
                assert_eq!(http_status, 200);
                assert_eq!(api_code, Some(404));
                assert_eq!(message, "No successful provider responses.");
            }
            other => panic!("unexpected error variant {other:?}"),
        }
    }
}

#[cfg(test)]
#[cfg(feature = "live_api_tests")]
mod tests {
    use std::{str::FromStr, time::Duration};

    use super::*;
    use crate::{
        ModelId, ProviderSlug,
        embeddings::{
            EmbClientConfig, EmbeddingEncodingFormat, EmbeddingInput, EmbeddingRequest,
            HasEmbeddingModels,
        },
        router_only::openrouter::{OpenRouter, ProviderPreferences},
        utils::{
            const_settings::test_consts::EMBEDDING_MODELS_JSON_FULL, test_helpers::openrouter_env,
        },
    };
    use color_eyre::Result;
    use fxhash::FxHashSet as HashSet;
    use once_cell::sync::Lazy;
    use once_cell::sync::OnceCell;
    use ploke_test_utils::workspace_root;
    use reqwest::Client;
    use serde_json::json;
    use tracing::error;

    static EMBEDDING_MODELS_FIXTURE: Lazy<serde_json::Value> = Lazy::new(|| {
        let mut read_file = workspace_root();
        read_file.push(EMBEDDING_MODELS_JSON_FULL);
        let file_string = std::fs::read_to_string(read_file).expect("fixture must exist");
        serde_json::from_str(&file_string).expect("valid embeddings models fixture")
    });

    static EMBEDDING_MODELS_ONCE: OnceCell<crate::request::models::Response> = OnceCell::new();

    fn fixture_models_response() -> crate::request::models::Response {
        serde_json::from_value(EMBEDDING_MODELS_FIXTURE.clone())
            .expect("fixture should deserialize into models::Response")
    }

    fn cheap_model_id() -> ModelId {
        ModelId::from_str("thenlper/gte-base").expect("fixture contains thenlper/gte-base")
    }

    fn fixture_model_ids() -> Vec<String> {
        EMBEDDING_MODELS_FIXTURE["data"]
            .as_array()
            .expect("fixture data array")
            .iter()
            .map(|entry| {
                let id_value = entry.get("id").expect("fixture entry has id field").clone();
                match id_value.as_str() {
                    Some(id) => id.to_string(),
                    None => {
                        let parsed: ModelId = serde_json::from_value(id_value)
                            .expect("fixture id parses into ModelId");
                        parsed.to_string()
                    }
                }
            })
            .collect()
    }

    fn base_embedding_request() -> EmbeddingRequest<OpenRouter> {
        EmbeddingRequest::<OpenRouter> {
            model: cheap_model_id(),
            input: EmbeddingInput::Single("ploke embed smoke".into()),
            ..Default::default()
        }
    }

    fn serialize_request(req: &EmbeddingRequest<OpenRouter>) -> serde_json::Value {
        serde_json::to_value(req).expect("embedding request serializes")
    }

    /// Queries and writes the response formatted from serde's Value defaults into a file as json.
    ///
    /// Basic test that the endpoint is correct and response is well-formed json.
    #[tokio::test]
    #[ignore = "use this to generate the fixture"]
    async fn test_simple_query_embedding_models() -> Result<()> {
        let url = OpenRouter::EMBEDDING_MODELS_URL;

        let response = Client::new()
            .get(url)
            // auth not required for this request
            .timeout(Duration::from_secs(crate::LLM_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| LlmError::Request {
                message: e.to_string(),
                url: Some(url.to_string()),
                is_timeout: e.is_timeout(),
            })?;

        let response_json = response.text().await?;
        let pretty: serde_json::Value = serde_json::from_str(&response_json)?;
        // println!("{pretty:#?}");
        let pretty_string = serde_json::to_string_pretty(&pretty)?;

        let mut out_file = workspace_root();
        out_file.push(EMBEDDING_MODELS_JSON_FULL);
        let dir = out_file
            .parent()
            .expect("expect target save dir has parent");
        std::fs::create_dir_all(dir)?;
        std::fs::write(out_file, pretty_string)?;

        Ok(())
    }

    #[tokio::test]
    async fn embedding_models_basic_fetch() -> Result<()> {
        let _env = openrouter_env().unwrap_or_else(|| {
            error!("OPENROUTER_API_KEY missing; live gate not satisfied");
            panic!("OPENROUTER_API_KEY missing");
        });

        let resp = <OpenRouter as HasEmbeddingModels>::fetch_embedding_models(
            &Client::new(),
            EmbClientConfig::default(),
        )
        .await
        .inspect_err(|err| error!(?err, "unable to reach OpenRouter for embedding models"))?;
        assert!(
            !resp.data.is_empty(),
            "live embedding model list should not be empty"
        );
        Ok(())
    }

    #[test]
    fn embedding_models_basic_deser() {
        let resp = fixture_models_response();
        assert!(
            !resp.data.is_empty(),
            "fixture embedding models should include entries"
        );
    }

    #[test]
    fn embedding_models_basic_roundtrip() {
        let resp = fixture_models_response();
        let fixture_ids = fixture_model_ids();
        for (typed_item, fixture_id) in resp.data.iter().zip(fixture_ids.iter()) {
            assert_eq!(typed_item.id.to_string(), *fixture_id);
        }
    }

    #[test]
    fn embedding_models_basic_reverse_roundtrip() {
        let fixture_ids = fixture_model_ids();
        let replayed: Vec<ModelId> = fixture_ids
            .iter()
            .map(|id| ModelId::from_str(id).expect("fixture ids parse into ModelId"))
            .collect();
        let resp = fixture_models_response();
        for (typed, parsed_again) in resp.data.iter().zip(replayed.iter()) {
            assert_eq!(&typed.id, parsed_again);
        }
    }

    #[test]
    fn check_once_cell_models() {
        let first = EMBEDDING_MODELS_ONCE.get_or_init(fixture_models_response);
        let second = EMBEDDING_MODELS_ONCE
            .get()
            .expect("OnceCell should already be initialized");
        assert!(
            std::ptr::eq(first, second),
            "OnceCell must return the same allocation"
        );
        assert!(!first.data.is_empty());
    }

    // --- Basic functionality tests for `fetch_embedding_models`
    // - basic_fetch: fetches models using trait method
    // - basic_deser: check the values are deserialized without errors
    // - basic_roundtrip: check the values are deserialized and serialized without errors
    // - basic_reverse_roundtrip: check the values are serialized and deserialized without errors

    // --- models fetch helpers test
    // - check_once_cell_models: sanity check, verifies that models are being read correctly with
    //   `get_values_once()` into expected type through deserialization

    // The following diagnostic tests are for the values of the generic encapsulating
    // struct `embeddings::EmbeddingRequest<R>`
    // - input .................... embedding_field_input
    // - model .................... embedding_field_model
    // - encoding_format .......... embedding_field_encode_format
    // - user ..................... embedding_field_user
    #[tokio::test]
    async fn embedding_field_input() -> Result<()> {
        // single input
        let mut single = base_embedding_request();
        single.input = EmbeddingInput::Single("single value input".into());
        let serialized = serialize_request(&single);
        assert_eq!(serialized["input"], json!("single value input"));

        // multi input
        let mut batch = base_embedding_request();
        batch.input = EmbeddingInput::Batch(vec!["first".into(), "second".into()]);
        let serialized = serialize_request(&batch);
        assert_eq!(serialized["input"], json!(["first", "second"]));

        // empty iterator should serialize to []
        let mut empty = base_embedding_request();
        empty.input = EmbeddingInput::Batch(Vec::new());
        let serialized = serialize_request(&empty);
        assert_eq!(serialized["input"], json!([]));
        Ok(())
    }
    #[tokio::test]
    async fn embedding_field_model() -> Result<()> {
        let req = base_embedding_request();
        let serialized = serialize_request(&req);
        assert_eq!(serialized["model"], json!("thenlper/gte-base"));

        let mut variant_req = base_embedding_request();
        variant_req.model = ModelId::from_str("deepseek/deepseek-r1:free")?;
        let serialized = serialize_request(&variant_req);
        assert_eq!(serialized["model"], json!("deepseek/deepseek-r1:free"));
        Ok(())
    }
    #[tokio::test]
    async fn embedding_field_encode_format() -> Result<()> {
        let request = base_embedding_request();
        let serialized = serialize_request(&request);
        assert!(
            serialized.get("encoding_format").is_none(),
            "default format should omit encoding_format"
        );

        let mut base64 = base_embedding_request();
        base64.encoding_format = Some(EmbeddingEncodingFormat::Base64);
        let serialized = serialize_request(&base64);
        assert_eq!(serialized["encoding_format"], json!("base64"));
        Ok(())
    }
    #[tokio::test]
    async fn embedding_field_user() -> Result<()> {
        let mut request = base_embedding_request();
        request.user = Some("ploke-test-user".into());
        let serialized = serialize_request(&request);
        assert_eq!(serialized["user"], json!("ploke-test-user"));

        request.user = None;
        let serialized = serialize_request(&request);
        assert!(
            serialized.get("user").is_none(),
            "user field should be omitted when None"
        );
        Ok(())
    }

    // The following diagnostic tests are for the values specific to OpenRouter
    // struct `embeddings::EmbeddingRequest<R>`
    // - dimensions ............... embedding_field_dimensions
    // - input_type ............... embedding_field_input_type
    #[tokio::test]
    async fn embedding_field_dimensions() -> Result<()> {
        let mut request = base_embedding_request();
        request.router.dimensions = Some(256);
        let serialized = serialize_request(&request);
        assert_eq!(serialized["dimensions"], json!(256));

        request.router.dimensions = None;
        let serialized = serialize_request(&request);
        assert!(
            serialized.get("dimensions").is_none(),
            "dimensions should be omitted when unset"
        );
        Ok(())
    }

    #[tokio::test]
    async fn embedding_field_input_type() -> Result<()> {
        let mut request = base_embedding_request();
        request.router.input_type = Some("code-snippet".into());
        let serialized = serialize_request(&request);
        assert_eq!(serialized["input_type"], json!("code-snippet"));

        request.router.input_type = None;
        let serialized = serialize_request(&request);
        assert!(
            serialized.get("input_type").is_none(),
            "input_type omitted when None"
        );
        Ok(())
    }

    // more tests here for provider, which will need more thorough handling that covers both the
    // encapsulated provider preferences and the new fields
    // - all basic provider fields
    // - new fields
    #[tokio::test]
    async fn embedding_field_provider_preferences() -> Result<()> {
        let mut only = HashSet::default();
        only.insert(ProviderSlug::new("openai"));

        let prefs = ProviderPreferences {
            allow_fallbacks: Some(false),
            order: Some(vec![
                ProviderSlug::new("openai"),
                ProviderSlug::new("anthropic"),
            ]),
            only: Some(only),
            ..Default::default()
        };

        let provider = EmbeddingProviderPrefs {
            base_provider_prefs: prefs,
            min_throughput: Some(45.0),
            max_latency: Some(1.2),
        };

        let mut request = base_embedding_request();
        request.router.provider = Some(provider);
        let serialized = serialize_request(&request);

        let provider_json = serialized
            .get("provider")
            .expect("provider field serialized");
        assert_eq!(provider_json["min_throughput"], json!(45.0));
        assert_eq!(provider_json["max_latency"], json!(1.2));
        assert_eq!(provider_json["allow_fallbacks"], json!(false));
        assert_eq!(
            provider_json["order"],
            json!(["openai", "anthropic"]),
            "order preserves insertion order"
        );
        let only = provider_json["only"]
            .as_array()
            .expect("only serializes to array");
        assert_eq!(only.len(), 1);
        assert_eq!(only[0], json!("openai"));
        Ok(())
    }

    #[derive(Clone, Debug)]
    struct EmbeddingModelCaps {
        id: ModelId,
        context_length: Option<u32>,
    }

    fn fixture_model_caps() -> Vec<EmbeddingModelCaps> {
        fixture_models_response()
            .data
            .into_iter()
            .map(|item| EmbeddingModelCaps {
                id: item.id.clone(),
                context_length: item.context_length.or(item.top_provider.context_length),
            })
            .collect()
    }

    fn ensure_model_by_id(models: &[EmbeddingModelCaps], id: &str) -> EmbeddingModelCaps {
        models
            .iter()
            .find(|caps| caps.id.to_string() == id)
            .cloned()
            .unwrap_or_else(|| {
                error!(model = id, "model not found in embeddings fixture");
                panic!("model {id} not present in embeddings fixture");
            })
    }

    fn first_model_with_min_context(models: &[EmbeddingModelCaps], min_context: u32) -> ModelId {
        models
            .iter()
            .filter_map(|caps| {
                let ctx = caps.context_length?;
                (ctx >= min_context).then_some(caps.id.clone())
            })
            .next()
            .unwrap_or_else(|| {
                error!(
                    min_context,
                    "no embedding model meets context length requirement"
                );
                panic!("no embedding model meets context length requirement");
            })
    }

    fn require_openrouter_env() {
        let _env = openrouter_env().unwrap_or_else(|| {
            error!("OPENROUTER_API_KEY missing; cannot run live embedding tests");
            panic!("OPENROUTER_API_KEY missing; live tests require API key");
        });
    }

    fn batch_len(input: &EmbeddingInput) -> usize {
        match input {
            EmbeddingInput::Single(_) => 1,
            EmbeddingInput::Batch(v) => v.len(),
        }
    }

    #[tokio::test]
    async fn live_batch_embeddings_with_dimensions() -> Result<()> {
        require_openrouter_env();
        let models = fixture_model_caps();
        let openai_small = ensure_model_by_id(&models, "openai/text-embedding-3-small");

        let snippets = vec![
            "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            "struct Point { x: f64, y: f64 }\nimpl Point { fn norm(&self) -> f64 { (self.x * self.x + self.y * self.y).sqrt() } }".to_string(),
            "async fn fetch_url(url: &str) -> Result<String, reqwest::Error> { reqwest::get(url).await?.text().await }".to_string(),
        ];

        let request = EmbeddingRequest::<OpenRouter> {
            model: openai_small.id.clone(),
            input: EmbeddingInput::Batch(snippets.clone()),
            router: OpenRouterEmbeddingFields {
                dimensions: Some(256),
                input_type: Some("code-snippet".into()),
                ..Default::default()
            },
            ..Default::default()
        };

        let resp = <OpenRouter as HasEmbeddings>::fetch_embeddings(&Client::new(), &request)
            .await
            .inspect_err(|err| error!(?err, "live batch embeddings (dimensions) failed"))?;

        assert_eq!(
            resp.data.len(),
            batch_len(&request.input),
            "response entries should match batch size"
        );
        for embedding in resp.data.iter().map(|d| &d.embedding) {
            let floats = match embedding {
                OpenRouterEmbeddingVector::Float(v) => v,
                OpenRouterEmbeddingVector::Base64(_) => {
                    error!("expected float embeddings but received base64 payload");
                    panic!("expected float embeddings");
                }
            };
            assert_eq!(
                floats.len(),
                256,
                "embedding length must honor requested dimensions"
            );
        }
        assert!(
            resp.model.matches_request(&request.model),
            "response model should align with request (got {}, expected {})",
            resp.model,
            request.model
        );
        Ok(())
    }

    #[tokio::test]
    async fn live_batch_embeddings_long_context() -> Result<()> {
        require_openrouter_env();
        let models = fixture_model_caps();
        let long_context_model = first_model_with_min_context(&models, 20_000);

        let snippets = vec![
            "// simulate large code block\nfn main() {\n    let mut acc = 0;\n    for i in 0..10_000 {\n        acc += i;\n        if acc % 17 == 0 { println!(\"{acc}\"); }\n    }\n}\n"
                .to_string(),
            "pub fn tokenize(input: &str) -> Vec<String> {\n    input\n        .split_whitespace()\n        .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())\n        .filter(|s| !s.is_empty())\n        .collect()\n}"
            .to_string(),
        ];

        let request = EmbeddingRequest::<OpenRouter> {
            model: long_context_model.clone(),
            input: EmbeddingInput::Batch(snippets.clone()),
            router: OpenRouterEmbeddingFields {
                input_type: Some("code-snippet".into()),
                ..Default::default()
            },
            ..Default::default()
        };

        let resp = <OpenRouter as HasEmbeddings>::fetch_embeddings(&Client::new(), &request)
            .await
            .inspect_err(
                |err| error!(?err, model = %request.model, "long-context batch embeddings failed"),
            )?;

        assert_eq!(
            resp.data.len(),
            batch_len(&request.input),
            "response entries should match batch size"
        );
        let first_len = match &resp
            .data
            .first()
            .expect("embedding response not empty")
            .embedding
        {
            OpenRouterEmbeddingVector::Float(v) => v.len(),
            OpenRouterEmbeddingVector::Base64(_) => {
                error!("expected float embeddings but received base64 payload");
                panic!("expected float embeddings");
            }
        };
        assert!(
            first_len > 0,
            "embedding vectors must contain at least one value"
        );
        for embedding in resp.data.iter().map(|d| &d.embedding) {
            let floats = match embedding {
                OpenRouterEmbeddingVector::Float(v) => v,
                OpenRouterEmbeddingVector::Base64(_) => {
                    error!("expected float embeddings but received base64 payload");
                    panic!("expected float embeddings");
                }
            };
            assert_eq!(
                floats.len(),
                first_len,
                "all embeddings in batch must share a consistent length"
            );
        }
        assert!(
            resp.model.matches_request(&request.model),
            "response model should align with request (got {}, expected {})",
            resp.model,
            request.model
        );
        Ok(())
    }
}
