use std::{fmt, sync::OnceLock, time::Duration};

use tracing::instrument;

use crate::{cancel_token::CancellationListener, config::OpenAIConfig, error::EmbedError};

const OPENAI_EMBEDDINGS_ENDPOINT: &str = "https://api.openai.com/v1/embeddings";
const OPENAI_ERROR_BODY: &str = "OpenAI response body omitted";
const MAX_BATCH_SIZE: usize = 100;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
static OPENAI_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

// OpenAI backend implementation.
pub struct OpenAIBackend {
    pub api_key: String,
    pub model: String,
    pub dimensions: usize,
}

impl fmt::Debug for OpenAIBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAIBackend")
            .field("api_key", &"<redacted>")
            .field("model", &self.model)
            .field("dimensions", &self.dimensions)
            .finish()
    }
}

impl OpenAIBackend {
    pub fn new(config: &OpenAIConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            dimensions: expected_dimensions(&config.model),
        }
    }

    pub async fn compute_batch(&self, snippets: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.compute_batch_with_cancel(snippets, None).await
    }

    #[instrument(
        skip_all,
        fields(expected_len = snippets.len()),
        target = "embed-pipeline"
    )]
    pub async fn compute_batch_with_cancel(
        &self,
        snippets: Vec<String>,
        cancel: Option<&CancellationListener>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.compute_batch_with(
            openai_client(),
            OPENAI_EMBEDDINGS_ENDPOINT,
            snippets,
            cancel,
        )
        .await
    }

    async fn compute_batch_with(
        &self,
        client: &reqwest::Client,
        endpoint: &str,
        snippets: Vec<String>,
        cancel: Option<&CancellationListener>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        if snippets.is_empty() {
            return Ok(Vec::new());
        }

        let expected_len = snippets.len();
        let mut embeddings = Vec::with_capacity(expected_len);
        let mut snippets = snippets.into_iter();

        loop {
            let batch: Vec<String> = snippets.by_ref().take(MAX_BATCH_SIZE).collect();
            if batch.is_empty() {
                break;
            }

            let result = match cancel {
                Some(cancel) => {
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => {
                            Err(EmbedError::Cancelled("OpenAI embeddings cancelled".into()))
                        }
                        result = self.fetch_batch(client, endpoint, batch) => result,
                    }
                }
                None => self.fetch_batch(client, endpoint, batch).await,
            }?;
            embeddings.extend(result);
        }

        if embeddings.len() != expected_len {
            return Err(EmbedError::Embedding(format!(
                "OpenAI batch result length mismatch: expected {expected_len}, got {}",
                embeddings.len()
            )));
        }

        Ok(embeddings)
    }

    async fn fetch_batch(
        &self,
        client: &reqwest::Client,
        endpoint: &str,
        snippets: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        let expected_len = snippets.len();
        let request = OpenAIEmbedRequest {
            model: self.model.clone(),
            input: snippets,
        };

        let response = client
            .post(endpoint)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(EmbedError::HttpError {
                status,
                body: OPENAI_ERROR_BODY.into(),
                url: endpoint.to_string(),
            });
        }

        let response = response
            .json::<OpenAIEmbedResponse>()
            .await
            .map_err(|error| {
                let detail: String = error.to_string().chars().take(120).collect();
                EmbedError::Network(format!("OpenAI response deserialization failed: {detail}"))
            })?;
        self.validate_and_reorder(response, expected_len)
    }

    #[allow(clippy::result_large_err)] // Preserve the crate's existing public EmbedError contract.
    fn validate_and_reorder(
        &self,
        response: OpenAIEmbedResponse,
        expected_len: usize,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        if response.data.len() != expected_len {
            return Err(EmbedError::Embedding(format!(
                "OpenAI response length mismatch: expected {expected_len}, got {}",
                response.data.len()
            )));
        }

        let mut ordered: Vec<Option<Vec<f32>>> = vec![None; expected_len];
        for item in response.data {
            if expected_len <= item.index {
                return Err(EmbedError::Embedding(format!(
                    "OpenAI response has invalid index {} for batch length {expected_len}",
                    item.index
                )));
            }
            if ordered[item.index].is_some() {
                return Err(EmbedError::Embedding(format!(
                    "OpenAI response contains duplicate index {}",
                    item.index
                )));
            }
            if item.embedding.len() != self.dimensions {
                return Err(EmbedError::DimensionMismatch {
                    expected: self.dimensions,
                    actual: item.embedding.len(),
                });
            }
            if item.embedding.iter().any(|value| !value.is_finite()) {
                return Err(EmbedError::Embedding(
                    "OpenAI returned non-finite float in embedding vector".into(),
                ));
            }
            ordered[item.index] = Some(item.embedding);
        }

        let mut embeddings = Vec::with_capacity(expected_len);
        for (index, embedding) in ordered.into_iter().enumerate() {
            let Some(embedding) = embedding else {
                return Err(EmbedError::Embedding(format!(
                    "OpenAI response missing embedding for index {index}"
                )));
            };
            embeddings.push(embedding);
        }
        Ok(embeddings)
    }
}

fn openai_client() -> &'static reqwest::Client {
    OPENAI_CLIENT.get_or_init(|| build_client(REQUEST_TIMEOUT))
}

fn build_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .expect("reqwest client with standard TLS configuration should build")
}

fn expected_dimensions(model: &str) -> usize {
    match model {
        "text-embedding-3-large" => 3_072,
        _ => 1_536,
    }
}

// Request structs for openAI
#[derive(serde::Serialize)]
struct OpenAIEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(serde::Deserialize)]
struct OpenAIEmbedding {
    index: usize,
    embedding: Vec<f32>,
}

#[derive(serde::Deserialize)]
struct OpenAIEmbedResponse {
    data: Vec<OpenAIEmbedding>,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use httpmock::prelude::*;

    use super::*;
    use crate::cancel_token::{CancellationListener, CancellationToken};

    fn config(model: &str, api_key: &str) -> OpenAIConfig {
        OpenAIConfig {
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    fn backend(dimensions: usize) -> OpenAIBackend {
        let mut backend = OpenAIBackend::new(&config("text-embedding-3-small", "test-key"));
        backend.dimensions = dimensions;
        backend
    }

    async fn compute_mock(
        backend: &OpenAIBackend,
        server: &MockServer,
        snippets: Vec<String>,
        cancel: Option<&CancellationListener>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        let client = build_client(Duration::from_secs(5));
        let endpoint = server.url("/v1/embeddings");
        backend
            .compute_batch_with(&client, &endpoint, snippets, cancel)
            .await
    }

    fn response(data: Vec<serde_json::Value>) -> String {
        serde_json::json!({ "data": data }).to_string()
    }

    fn assert_generic_error(error: EmbedError) {
        match error {
            EmbedError::HttpError { body, .. } => assert_eq!(body, OPENAI_ERROR_BODY),
            error => panic!("expected HTTP error, got {error:?}"),
        }
    }

    #[test]
    fn debug_redacts_api_key_and_preserves_struct_literal() {
        let source = crate::indexer::EmbeddingSource::OpenAI(OpenAIBackend {
            api_key: "secret-openai-key".into(),
            model: "text-embedding-3-small".into(),
            dimensions: 1_536,
        });

        let debug = format!("{source:?}");

        assert!(!debug.contains("secret-openai-key"), "debug leaked API key");
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn maps_known_openai_model_dimensions() {
        let small = OpenAIBackend::new(&config("text-embedding-3-small", "test-key"));
        let ada = OpenAIBackend::new(&config("text-embedding-ada-002", "test-key"));
        let large = OpenAIBackend::new(&config("text-embedding-3-large", "test-key"));

        assert_eq!(small.dimensions, 1_536);
        assert_eq!(ada.dimensions, 1_536);
        assert_eq!(large.dimensions, 3_072);
    }

    #[tokio::test]
    async fn reorders_indexed_vectors() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(200).body(response(vec![
                serde_json::json!({ "index": 1, "embedding": [0.4, 0.5, 0.6] }),
                serde_json::json!({ "index": 0, "embedding": [0.1, 0.2, 0.3] }),
            ]));
        });
        let backend = backend(3);

        let embeddings = compute_mock(
            &backend,
            &server,
            vec!["first".into(), "second".into()],
            None,
        )
        .await
        .expect("valid response");

        assert_eq!(embeddings[0], vec![0.1, 0.2, 0.3]);
        assert_eq!(embeddings[1], vec![0.4, 0.5, 0.6]);
        mock.assert_hits(1);
    }

    #[test]
    fn rejects_invalid_response_shape() {
        let backend = backend(2);

        let count = backend
            .validate_and_reorder(
                OpenAIEmbedResponse {
                    data: vec![OpenAIEmbedding {
                        index: 0,
                        embedding: vec![0.1, 0.2],
                    }],
                },
                2,
            )
            .expect_err("missing response item must fail");
        assert!(count.to_string().contains("length mismatch"));

        let duplicate = backend
            .validate_and_reorder(
                OpenAIEmbedResponse {
                    data: vec![
                        OpenAIEmbedding {
                            index: 0,
                            embedding: vec![0.1, 0.2],
                        },
                        OpenAIEmbedding {
                            index: 0,
                            embedding: vec![0.3, 0.4],
                        },
                    ],
                },
                2,
            )
            .expect_err("duplicate response index must fail");
        assert!(duplicate.to_string().contains("duplicate index"));

        let out_of_range = backend
            .validate_and_reorder(
                OpenAIEmbedResponse {
                    data: vec![OpenAIEmbedding {
                        index: 1,
                        embedding: vec![0.1, 0.2],
                    }],
                },
                1,
            )
            .expect_err("out-of-range response index must fail");
        assert!(out_of_range.to_string().contains("invalid index"));
    }

    #[test]
    fn rejects_invalid_embedding_values() {
        let backend = backend(2);

        let dimensions = backend
            .validate_and_reorder(
                OpenAIEmbedResponse {
                    data: vec![OpenAIEmbedding {
                        index: 0,
                        embedding: vec![0.1],
                    }],
                },
                1,
            )
            .expect_err("wrong dimensions must fail");
        assert!(matches!(
            dimensions,
            EmbedError::DimensionMismatch {
                expected: 2,
                actual: 1
            }
        ));

        let non_finite = backend
            .validate_and_reorder(
                OpenAIEmbedResponse {
                    data: vec![OpenAIEmbedding {
                        index: 0,
                        embedding: vec![0.1, f32::NAN],
                    }],
                },
                1,
            )
            .expect_err("non-finite values must fail");
        assert!(non_finite.to_string().contains("non-finite"));
    }

    #[tokio::test]
    async fn omits_adversarial_non_success_body() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(401).json_body(serde_json::json!({
                "error": {
                    "message": "request exposed secret-openai-key and private input",
                    "type": "secret_type_ABC123",
                    "code": "secret_code_XYZ789"
                }
            }));
        });
        let backend = backend(2);

        let error = compute_mock(&backend, &server, vec!["private input".into()], None)
            .await
            .expect_err("non-success response must fail");
        let message = error.to_string();

        assert!(!message.contains("secret-openai-key"));
        assert!(!message.contains("private input"));
        assert!(!message.contains("secret_type_ABC123"));
        assert!(!message.contains("secret_code_XYZ789"));
        assert_generic_error(error);
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn omits_oversized_malformed_error_body() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(500).body(format!(
                "{{malformed-secret:{}",
                "upstream-secret".repeat(10_000)
            ));
        });
        let backend = backend(2);

        let error = compute_mock(&backend, &server, vec!["private input".into()], None)
            .await
            .expect_err("malformed provider error must fail safely");
        let message = error.to_string();

        assert!(!message.contains("malformed-secret"));
        assert!(!message.contains("upstream-secret"));
        assert_generic_error(error);
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn cancellation_interrupts_in_flight_request() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(200)
                .delay(Duration::from_secs(2))
                .body(response(vec![
                    serde_json::json!({ "index": 0, "embedding": [0.1, 0.2] }),
                ]));
        });
        let backend = backend(2);
        let (token, handle) = CancellationToken::new();
        let listener = token.listener();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            handle.cancel();
        });

        let error = tokio::time::timeout(
            Duration::from_millis(500),
            compute_mock(&backend, &server, vec!["first".into()], Some(&listener)),
        )
        .await
        .expect("cancellation should interrupt request promptly")
        .expect_err("cancelled request must fail");

        assert!(matches!(error, EmbedError::Cancelled(_)));
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn pre_cancelled_request_does_not_start() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(200).body(response(vec![
                serde_json::json!({ "index": 0, "embedding": [0.1, 0.2] }),
            ]));
        });
        let backend = backend(2);
        let (token, handle) = CancellationToken::new();
        let listener = token.listener();
        handle.cancel();

        let error = compute_mock(&backend, &server, vec!["first".into()], Some(&listener))
            .await
            .expect_err("pre-cancelled request must fail");

        assert!(matches!(error, EmbedError::Cancelled(_)));
        mock.assert_hits(0);
    }

    #[tokio::test]
    async fn request_timeout_is_bounded() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(200)
                .delay(Duration::from_millis(250))
                .body(response(vec![
                    serde_json::json!({ "index": 0, "embedding": [0.1, 0.2] }),
                ]));
        });
        let backend = backend(2);
        let client = build_client(Duration::from_millis(20));
        let endpoint = server.url("/v1/embeddings");

        let error = backend
            .compute_batch_with(&client, &endpoint, vec!["first".into()], None)
            .await
            .expect_err("request must respect configured client timeout");

        assert!(matches!(error, EmbedError::Network(_)));
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn splits_oversized_batches() {
        let server = MockServer::start();
        let first_input: Vec<String> = (0..100).map(|index| format!("item-{index}")).collect();
        let first_data: Vec<serde_json::Value> = (0..100)
            .map(|index| serde_json::json!({ "index": index, "embedding": [0.1, 0.2] }))
            .collect();
        let first = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings").json_body(
                serde_json::json!({ "model": "text-embedding-3-small", "input": first_input }),
            );
            then.status(200).body(response(first_data));
        });
        let second = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/embeddings")
                .json_body(serde_json::json!({
                    "model": "text-embedding-3-small",
                    "input": ["item-100"]
                }));
            then.status(200).body(response(vec![
                serde_json::json!({ "index": 0, "embedding": [0.3, 0.4] }),
            ]));
        });
        let backend = backend(2);
        let snippets: Vec<String> = (0..101).map(|index| format!("item-{index}")).collect();

        let embeddings = compute_mock(&backend, &server, snippets, None)
            .await
            .expect("split batches should succeed");

        assert_eq!(embeddings.len(), 101);
        assert_eq!(embeddings[100], vec![0.3, 0.4]);
        first.assert_hits(1);
        second.assert_hits(1);
    }
}
