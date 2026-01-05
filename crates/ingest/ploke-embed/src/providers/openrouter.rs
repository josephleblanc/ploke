use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::{Arc, OnceLock},
    time::Duration,
};

use tokio::sync::{Mutex, Semaphore};
use tokio::time::{self, MissedTickBehavior};
use tracing::instrument;

use crate::{
    cancel_token::CancellationListener,
    config::{OpenRouterConfig, TruncatePolicy},
    error::{truncate_string, EmbedError},
};
use ploke_llm::request::models as openrouter_models;

use ploke_llm::embeddings::{EmbeddingEncodingFormat, EmbeddingInput, EmbeddingRequest};
use ploke_llm::router_only::openrouter::embed::{
    OpenRouterEmbedEnv, OpenRouterEmbeddingError, OpenRouterEmbeddingFields,
};
use ploke_llm::router_only::openrouter::{embed::OpenRouterEmbeddingVector, OpenRouter};
use ploke_llm::ModelId;

#[derive(Debug, Clone)]
struct RetryConfig {
    max_attempts: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl RetryConfig {
    fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        // attempt is 1-based; attempt=1 => initial backoff.
        let shift = attempt.saturating_sub(1).min(16) as u32;
        let mul = 1u64 << shift;
        let backoff = self.initial_backoff.saturating_mul(mul as u32);
        std::cmp::min(backoff, self.max_backoff)
    }
}

/// OpenRouter embeddings backend implemented via `ploke-llm` typed request/response.
#[derive(Debug)]
pub struct OpenRouterBackend {
    pub model: ModelId,
    /// the expected vector length the backend will enforce. It’s used for response validation
    /// (rejects mismatches)
    pub dimensions: usize,
    /// the optional request hint sent to OpenRouter to ask for truncated vectors. It’s not
    /// guaranteed to be honored, and many providers ignore it.
    request_dimensions: Option<u32>,
    input_type: Option<String>,
    truncate_policy: TruncatePolicy,

    client: reqwest::Client,
    in_flight: Arc<Semaphore>,
    rps_limiter: Option<Arc<Mutex<time::Interval>>>,
    retry: RetryConfig,
    embed_env_override: Option<OpenRouterEmbedEnv>,
}

impl OpenRouterBackend {
    pub fn new(cfg: &OpenRouterConfig) -> Result<Self, EmbedError> {
        Self::new_with_override(cfg, None)
    }

    pub fn new_with_env(
        cfg: &OpenRouterConfig,
        embed_env: OpenRouterEmbedEnv,
    ) -> Result<Self, EmbedError> {
        Self::new_with_override(cfg, Some(embed_env))
    }

    fn new_with_override(
        cfg: &OpenRouterConfig,
        embed_env_override: Option<OpenRouterEmbedEnv>,
    ) -> Result<Self, EmbedError> {
        let model = ModelId::from_str(&cfg.model)
            .map_err(|e| EmbedError::Config(format!("invalid OpenRouter model id: {e}")))?;
        let dims = cfg.dimensions.ok_or_else(|| {
            EmbedError::Config(
                "OpenRouterConfig.dimensions must be set (dims inference not wired yet)".into(),
            )
        })?;

        let builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs))
            .connect_timeout(Duration::from_secs(10));
        // Keep defaults otherwise; OpenRouter auth + headers handled in `ploke-llm`.
        let client = builder
            .build()
            .map_err(|e| EmbedError::Network(e.to_string()))?;

        let rps_limiter = cfg.requests_per_second.map(|rps| {
            let per = if rps == 0 {
                Duration::from_secs(1)
            } else {
                Duration::from_secs_f64(1.0 / (rps as f64))
            };
            let mut interval = time::interval(per);
            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
            Arc::new(Mutex::new(interval))
        });

        Ok(Self {
            model,
            dimensions: dims,
            request_dimensions: cfg.request_dimensions.map(|d| d as u32),
            input_type: cfg.input_type.clone(),
            truncate_policy: cfg.truncate_policy,
            client,
            in_flight: Arc::new(Semaphore::new(cfg.max_in_flight.max(1))),
            rps_limiter,
            retry: RetryConfig {
                max_attempts: cfg.max_attempts.max(1),
                initial_backoff: Duration::from_millis(cfg.initial_backoff_ms.max(1)),
                max_backoff: Duration::from_millis(cfg.max_backoff_ms.max(cfg.initial_backoff_ms)),
            },
            embed_env_override,
        })
    }

    async fn wait_cancel_or_sleep(
        cancel: Option<&CancellationListener>,
        dur: Duration,
    ) -> Result<(), EmbedError> {
        if let Some(cancel) = cancel {
            tokio::select! {
                _ = cancel.cancelled() => {
                    Err(EmbedError::Cancelled("OpenRouter embeddings cancelled".into()))
                }
                _ = time::sleep(dur) => Ok(()),
            }
        } else {
            time::sleep(dur).await;
            Ok(())
        }
    }

    async fn acquire_permit(
        &self,
        cancel: Option<&CancellationListener>,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, EmbedError> {
        if let Some(cancel) = cancel {
            tokio::select! {
                _ = cancel.cancelled() => {
                    Err(EmbedError::Cancelled("OpenRouter embeddings cancelled".into()))
                }
                permit = self.in_flight.clone().acquire_owned() => {
                    permit.map_err(|_| EmbedError::Cancelled("OpenRouter limiter closed".into()))
                }
            }
        } else {
            self.in_flight
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| EmbedError::Cancelled("OpenRouter limiter closed".into()))
        }
    }

    async fn rps_tick(&self, cancel: Option<&CancellationListener>) -> Result<(), EmbedError> {
        let Some(limiter) = &self.rps_limiter else {
            return Ok(());
        };
        let mut interval = limiter.lock().await;
        if let Some(cancel) = cancel {
            tokio::select! {
                _ = cancel.cancelled() => Err(EmbedError::Cancelled("OpenRouter embeddings cancelled".into())),
                _ = interval.tick() => Ok(()),
            }
        } else {
            interval.tick().await;
            Ok(())
        }
    }

    fn resolve_env(&self) -> Result<OpenRouterEmbedEnv, EmbedError> {
        if let Some(env) = &self.embed_env_override {
            return Ok(env.clone());
        }
        OpenRouterEmbedEnv::from_env().map_err(|e| EmbedError::Config(e.to_string()))
    }

    // TODO:ploke-remote 2025-12-15
    // Either here or elsewhere add a method for `with_provider`, so we can propogate the
    // configuration that the user might make to use specific provider preferences
    // - c.f. `EmbeddingProviderPrefs` in ploke-llm/src/router_only/openrouter/mod.rs
    fn build_request(&self, snippets: Vec<String>) -> EmbeddingRequest<OpenRouter> {
        EmbeddingRequest::<OpenRouter>::default()
            .with_model(self.model.clone())
            .with_input(EmbeddingInput::Batch(snippets))
            .with_encoding_format(EmbeddingEncodingFormat::Float)
            .with_router_bundle(OpenRouterEmbeddingFields {
                dimensions: self.request_dimensions,
                input_type: self.input_type.clone(),
                // provider: todo!(),
                ..Default::default()
            })
        // req.model = self.model.clone();
        // req.input = EmbeddingInput::Batch(snippets);
        // req.encoding_format = Some(EmbeddingEncodingFormat::Float);
        // req.router.dimensions = self.request_dimensions;
        // req.router.input_type = self.input_type.clone();
    }

    fn validate_and_reorder(
        &self,
        req: &EmbeddingRequest<OpenRouter>,
        resp: ploke_llm::router_only::openrouter::embed::OpenRouterEmbeddingsResponse,
        expected_len: usize,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        if resp.data.len() != expected_len {
            return Err(EmbedError::Embedding(format!(
                "OpenRouter response length mismatch: expected {}, got {}",
                expected_len,
                resp.data.len()
            )));
        }

        if !resp.model.matches_request(&req.model) {
            tracing::warn!(
                "OpenRouter response model mismatch: requested={}, got={}",
                req.model,
                resp.model
            );
        }

        let mut seen: HashSet<usize> = HashSet::with_capacity(expected_len);
        let mut ordered: Vec<Option<Vec<f32>>> = vec![None; expected_len];

        for item in resp.data {
            let idx_f = item.index.ok_or_else(|| {
                EmbedError::Embedding("OpenRouter response missing `index` field".into())
            })?;
            let idx_u = idx_f as usize;
            if (idx_u as f64) != idx_f || idx_u >= expected_len {
                return Err(EmbedError::Embedding(format!(
                    "OpenRouter response has invalid index={idx_f} for batch_len={expected_len}"
                )));
            }
            if !seen.insert(idx_u) {
                return Err(EmbedError::Embedding(format!(
                    "OpenRouter response contains duplicate index {idx_u}"
                )));
            }

            let floats = match item.embedding {
                OpenRouterEmbeddingVector::Float(v) => v,
                OpenRouterEmbeddingVector::Base64(_) => {
                    return Err(EmbedError::Embedding(
                        "OpenRouter returned base64 embeddings but float was requested".into(),
                    ))
                }
            };

            if floats.is_empty() {
                return Err(EmbedError::Embedding(
                    "OpenRouter returned an empty embedding vector".into(),
                ));
            }
            if floats.len() != self.dimensions {
                return Err(EmbedError::DimensionMismatch {
                    expected: self.dimensions,
                    actual: floats.len(),
                });
            }

            let mut out = Vec::with_capacity(floats.len());
            for f in floats {
                if !f.is_finite() {
                    return Err(EmbedError::Embedding(
                        "OpenRouter returned non-finite float in embedding vector".into(),
                    ));
                }
                out.push(f as f32);
            }
            ordered[idx_u] = Some(out);
        }

        if seen.len() != expected_len {
            return Err(EmbedError::Embedding(format!(
                "OpenRouter response missing indices: expected {}, got {} unique indices",
                expected_len,
                seen.len()
            )));
        }

        ordered
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                v.ok_or_else(|| {
                    EmbedError::Embedding(format!(
                        "OpenRouter response missing embedding for index {i}"
                    ))
                })
            })
            .collect()
    }

    #[instrument(skip_all, fields(expected_len), target = "embed-pipeline")]
    pub async fn compute_batch(
        &self,
        snippets: Vec<String>,
        cancel: Option<&CancellationListener>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        if snippets.is_empty() {
            return Ok(Vec::new());
        }

        let snippets = self.apply_truncation_policy(snippets)?;

        // Concurrency gate (in-flight) + optional RPS limiter.
        let _permit = self.acquire_permit(cancel).await?;
        self.rps_tick(cancel).await?;

        let expected_len = snippets.len();
        let req = self.build_request(snippets);
        let env = self.resolve_env()?;

        let mut last_err: Option<EmbedError> = None;
        for attempt in 1..=self.retry.max_attempts {
            let result = OpenRouter::fetch_embeddings_with_env(&self.client, &req, &env).await;

            match result {
                Ok(resp) => return self.validate_and_reorder(&req, resp, expected_len),
                Err(report) => {
                    // Attempt to interpret OpenRouter-specific errors for bounded retry behavior.
                    if let Some(kind) = report.downcast_ref::<OpenRouterEmbeddingError>() {
                        match kind {
                            OpenRouterEmbeddingError::RateLimited { retry_after, url } => {
                                let backoff = retry_after
                                    .unwrap_or_else(|| self.retry.backoff_for_attempt(attempt));
                                tracing::warn!(
                                    "OpenRouter rate limited (attempt {}/{}); sleeping {:?} (url={})",
                                    attempt,
                                    self.retry.max_attempts,
                                    backoff,
                                    url
                                );
                                last_err = Some(EmbedError::HttpError {
                                    status: 429,
                                    body: "rate limited".into(),
                                    url: url.clone(),
                                });
                                Self::wait_cancel_or_sleep(cancel, backoff).await?;
                                continue;
                            }
                            OpenRouterEmbeddingError::ProviderOverloaded { url } => {
                                let backoff = self.retry.backoff_for_attempt(attempt);
                                tracing::warn!(
                                    "OpenRouter provider overloaded (attempt {}/{}); sleeping {:?} (url={})",
                                    attempt,
                                    self.retry.max_attempts,
                                    backoff,
                                    url
                                );
                                last_err = Some(EmbedError::HttpError {
                                    status: 529,
                                    body: "provider overloaded".into(),
                                    url: url.clone(),
                                });
                                Self::wait_cancel_or_sleep(cancel, backoff).await?;
                                continue;
                            }
                            OpenRouterEmbeddingError::BadRequest { detail, url } => {
                                return Err(EmbedError::HttpError {
                                    status: 400,
                                    body: detail.clone(),
                                    url: url.clone(),
                                });
                            }
                            OpenRouterEmbeddingError::Unauthorized { url } => {
                                return Err(EmbedError::HttpError {
                                    status: 401,
                                    body: "unauthorized".into(),
                                    url: url.clone(),
                                });
                            }
                            OpenRouterEmbeddingError::PaymentRequired { url } => {
                                return Err(EmbedError::HttpError {
                                    status: 402,
                                    body: "payment required".into(),
                                    url: url.clone(),
                                });
                            }
                            OpenRouterEmbeddingError::NotFound { model, url } => {
                                return Err(EmbedError::HttpError {
                                    status: 404,
                                    body: format!("model not found: {model}"),
                                    url: url.clone(),
                                });
                            }
                            OpenRouterEmbeddingError::Transport { message, url } => {
                                last_err = Some(EmbedError::Network(format!(
                                    "OpenRouter transport error: {} (url={})",
                                    truncate_string(message, 120),
                                    url
                                )));
                            }
                            OpenRouterEmbeddingError::ApiError {
                                url,
                                http_status,
                                api_code,
                                request_id,
                                content_type,
                                message,
                                body_snippet,
                            } => {
                                // Backwards-compat safety valve: if we're only sending `dimensions`
                                // redundantly (request == expected), retry once without it.
                                if attempt == 1
                                    && api_code == &Some(404)
                                    && message.contains("No successful provider responses")
                                    && self.request_dimensions == Some(self.dimensions as u32)
                                {
                                    tracing::warn!(
                                        "OpenRouter reported no provider responses; retrying once without request `dimensions` (model={}, expected_dims={})",
                                        self.model,
                                        self.dimensions
                                    );
                                    let mut retry_req = req.clone();
                                    retry_req.router.dimensions = None;
                                    let retry_res = OpenRouter::fetch_embeddings_with_env(
                                        &self.client,
                                        &retry_req,
                                        &env,
                                    )
                                    .await;
                                    if let Ok(resp) = retry_res {
                                        return self.validate_and_reorder(
                                            &retry_req,
                                            resp,
                                            expected_len,
                                        );
                                    }
                                }
                                let status = api_code.unwrap_or(*http_status);
                                let mut body = format!(
                                    "model={} {}",
                                    req.model,
                                    truncate_string(message, 200)
                                );
                                if let Some(snippet) = body_snippet.as_ref() {
                                    body.push_str("; body_snippet=");
                                    body.push_str(&truncate_string(snippet, 200));
                                }
                                if request_id.is_some() || content_type.is_some() {
                                    body.push_str(&format!(
                                        " (request_id={:?}, content_type={:?})",
                                        request_id, content_type
                                    ));
                                }
                                last_err = Some(EmbedError::HttpError {
                                    status,
                                    body,
                                    url: url.clone(),
                                });
                            }
                            OpenRouterEmbeddingError::Decode {
                                message,
                                url,
                                status,
                                request_id,
                                content_type,
                                body_snippet,
                            } => {
                                last_err = Some(EmbedError::Network(format!(
                                    "OpenRouter decode error: {} (status={}, request_id={:?}, content_type={:?}, body_snippet={}) (url={})",
                                    truncate_string(message, 160),
                                    status,
                                    request_id,
                                    content_type,
                                    truncate_string(body_snippet, 200),
                                    url
                                )));
                            }
                            OpenRouterEmbeddingError::Unexpected { status, url, body } => {
                                last_err = Some(EmbedError::HttpError {
                                    status: *status,
                                    body: truncate_string(body, 200),
                                    url: url.clone(),
                                });
                            }
                        }
                    } else {
                        last_err = Some(EmbedError::Network(truncate_string(
                            &report.to_string(),
                            200,
                        )));
                    }

                    if attempt >= self.retry.max_attempts {
                        break;
                    }
                    let backoff = self.retry.backoff_for_attempt(attempt);
                    Self::wait_cancel_or_sleep(cancel, backoff).await?;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            EmbedError::Network("OpenRouter embedding request failed (unknown error)".into())
        }))
    }

    fn apply_truncation_policy(
        &self,
        mut snippets: Vec<String>,
    ) -> Result<Vec<String>, EmbedError> {
        if self.truncate_policy == TruncatePolicy::PassThrough {
            return Ok(snippets);
        }
        let max_chars = self.max_snippet_chars();
        let Some(max_chars) = max_chars else {
            tracing::warn!(
                target: "embed-pipeline",
                model = %self.model,
                "No context length available; skipping snippet truncation"
            );
            return Ok(snippets);
        };

        match self.truncate_policy {
            TruncatePolicy::Truncate => {
                let truncated = truncate_snippets_in_place(&mut snippets, max_chars);
                if truncated > 0 {
                    tracing::warn!(
                        target: "embed-pipeline",
                        truncated_snippets = truncated,
                        max_chars = max_chars,
                        "Truncated snippets to max length"
                    );
                }
                Ok(snippets)
            }
            TruncatePolicy::Reject => {
                if let Some((idx, len)) = snippets.iter().enumerate().find_map(|(idx, s)| {
                    let len = s.chars().count();
                    (len > max_chars).then_some((idx, len))
                }) {
                    return Err(EmbedError::Embedding(format!(
                        "snippet {idx} exceeds max length: {len} > {max_chars}"
                    )));
                }
                Ok(snippets)
            }
            TruncatePolicy::PassThrough => Ok(snippets),
        }
    }

    fn max_snippet_chars(&self) -> Option<usize> {
        let model_id = self.model.to_string();
        if let Some(tokens) = openrouter_embedding_context_length(&model_id) {
            return Some((tokens as usize).saturating_mul(4));
        }
        let max_chars = self.dimensions.saturating_mul(24);
        tracing::warn!(
            target: "embed-pipeline",
            model = %model_id,
            dims = self.dimensions,
            max_chars = max_chars,
            "No OpenRouter context length found; using dims-based fallback"
        );
        Some(max_chars)
    }
}

fn openrouter_embedding_context_length(model_id: &str) -> Option<u32> {
    static MODEL_CONTEXT: OnceLock<HashMap<String, u32>> = OnceLock::new();
    let map = MODEL_CONTEXT.get_or_init(|| {
        let json = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/openrouter/embeddings_models.json"
        ));
        let parsed: Result<openrouter_models::Response, _> = serde_json::from_str(json);
        match parsed {
            Ok(resp) => resp
                .data
                .into_iter()
                .filter_map(|item| {
                    let ctx = item.context_length.or(item.top_provider.context_length)?;
                    Some((item.id.to_string(), ctx))
                })
                .collect(),
            Err(err) => {
                tracing::warn!(
                    target: "embed-pipeline",
                    error = %err,
                    "Failed to parse OpenRouter embeddings model fixture"
                );
                HashMap::new()
            }
        }
    });
    map.get(model_id).copied()
}

fn truncate_snippets_in_place(snippets: &mut [String], max_chars: usize) -> usize {
    let mut truncated = 0;
    for (idx, snippet) in snippets.iter_mut().enumerate() {
        let original_chars = snippet.chars().count();
        if original_chars <= max_chars {
            continue;
        }
        let truncate_idx = snippet
            .char_indices()
            .nth(max_chars)
            .map(|(idx, _)| idx)
            .unwrap_or(snippet.len());
        snippet.truncate(truncate_idx);
        truncated += 1;
        tracing::warn!(
            target: "embed-pipeline",
            snippet_index = idx,
            original_chars = original_chars,
            max_chars = max_chars,
            "Truncated snippet for embedding"
        );
    }
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use once_cell::sync::Lazy;
    use tokio::sync::Mutex as TokioMutex;

    static ENV_MUTEX: Lazy<TokioMutex<()>> = Lazy::new(|| TokioMutex::new(()));

    fn test_env(url: &str) -> OpenRouterEmbedEnv {
        OpenRouterEmbedEnv::from_parts("test-key", url.to_string())
    }

    fn cfg(model: &str, dims: usize) -> OpenRouterConfig {
        OpenRouterConfig {
            model: model.to_string(),
            dimensions: Some(dims),
            request_dimensions: None,
            max_in_flight: 4,
            requests_per_second: None,
            max_attempts: 1,
            initial_backoff_ms: 1,
            max_backoff_ms: 1,
            input_type: Some("code-snippet".into()),
            timeout_secs: 5,
            truncate_policy: TruncatePolicy::Truncate,
        }
    }

    #[tokio::test]
    async fn parses_float_vectors_and_reorders_by_index() {
        let _guard = ENV_MUTEX.lock().await;
        let server = MockServer::start();
        let body = serde_json::json!({
            "data": [
                { "index": 1, "embedding": [0.4, 0.5, 0.6] },
                { "index": 0, "embedding": [0.1, 0.2, 0.3] }
            ],
            "model": "openai/text-embedding-3-small",
            "id": "req-123"
        })
        .to_string();
        let _m = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(200).body(body);
        });
        let env = test_env(&server.url("/v1/embeddings"));

        let backend =
            OpenRouterBackend::new_with_env(&cfg("openai/text-embedding-3-small", 3), env).unwrap();
        let out = backend
            .compute_batch(vec!["a".into(), "b".into()], None)
            .await
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], vec![0.1, 0.2, 0.3]);
        assert_eq!(out[1], vec![0.4, 0.5, 0.6]);
    }

    #[tokio::test]
    async fn rejects_base64_when_float_requested() {
        let _guard = ENV_MUTEX.lock().await;
        let server = MockServer::start();
        let body = serde_json::json!({
            "data": [
                { "index": 0, "embedding": "AAAA" }
            ],
            "model": "openai/text-embedding-3-small"
        })
        .to_string();
        let _m = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(200).body(body);
        });
        let env = test_env(&server.url("/v1/embeddings"));

        let backend =
            OpenRouterBackend::new_with_env(&cfg("openai/text-embedding-3-small", 3), env).unwrap();
        let err = backend
            .compute_batch(vec!["a".into()], None)
            .await
            .expect_err("expected base64 rejection");
        let msg = err.to_string();
        assert!(msg.contains("base64"), "unexpected error: {msg}");
    }

    #[tokio::test]
    async fn validates_index_is_present_and_in_range() {
        let _guard = ENV_MUTEX.lock().await;
        let server = MockServer::start();
        let body = serde_json::json!({
            "data": [
                { "embedding": [0.1, 0.2, 0.3] }
            ],
            "model": "openai/text-embedding-3-small"
        })
        .to_string();
        let _m = server.mock(|when, then| {
            when.method(POST).path("/v1/embeddings");
            then.status(200).body(body);
        });
        let env = test_env(&server.url("/v1/embeddings"));

        let backend =
            OpenRouterBackend::new_with_env(&cfg("openai/text-embedding-3-small", 3), env).unwrap();
        let err = backend
            .compute_batch(vec!["a".into()], None)
            .await
            .expect_err("missing index should fail");
        assert!(
            err.to_string().contains("index"),
            "unexpected error: {}",
            err
        );
    }

    #[tokio::test]
    async fn cancels_before_sending_request() {
        let _guard = ENV_MUTEX.lock().await;
        let server = MockServer::start();
        // Any attempt to hit the server would fail the test because no mock is registered.
        let env = test_env(&server.url("/v1/embeddings"));

        let backend =
            OpenRouterBackend::new_with_env(&cfg("openai/text-embedding-3-small", 3), env).unwrap();
        let (token, handle) = crate::cancel_token::CancellationToken::new();
        let listener = token.listener();
        handle.cancel();

        let err = backend
            .compute_batch(vec!["unused".into()], Some(&listener))
            .await
            .expect_err("cancelled listener should short-circuit the request");
        assert!(
            matches!(err, EmbedError::Cancelled(_)),
            "expected cancellation error, got {err:?}"
        );
    }

    #[test]
    fn truncates_long_snippet_using_model_context() {
        let cfg = OpenRouterConfig {
            model: "mistralai/codestral-embed-2505".to_string(),
            dimensions: Some(1536),
            request_dimensions: None,
            max_in_flight: 1,
            requests_per_second: None,
            max_attempts: 1,
            initial_backoff_ms: 1,
            max_backoff_ms: 1,
            input_type: Some("code-snippet".into()),
            timeout_secs: 5,
            truncate_policy: TruncatePolicy::Truncate,
        };
        let backend = OpenRouterBackend::new_with_env(&cfg, test_env("http://example.invalid"))
            .expect("backend init");
        let max_chars = backend
            .max_snippet_chars()
            .expect("fixture should define context length");

        let snippet = "a".repeat(max_chars + 10);
        let out = backend
            .apply_truncation_policy(vec![snippet])
            .expect("truncation should succeed");
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].chars().count(),
            max_chars,
            "snippet should be truncated to max chars"
        );
    }

    #[cfg(feature = "live_api_tests")]
    mod live_api_tests {
        use super::*;
        use serde::{Deserialize, Serialize};
        use std::{
            collections::HashMap,
            fs,
            path::{Path, PathBuf},
            time::{Duration, Instant, SystemTime, UNIX_EPOCH},
        };

        #[derive(Debug, Deserialize)]
        struct FixtureModels {
            data: Vec<FixtureModel>,
        }

        #[derive(Debug, Deserialize)]
        struct FixtureModel {
            id: String,
            architecture: FixtureArchitecture,
        }

        #[derive(Debug, Deserialize)]
        struct FixtureArchitecture {
            modality: String,
            input_modalities: Vec<String>,
            output_modalities: Vec<String>,
        }

        #[derive(Debug, Clone, Copy)]
        struct LiveModelCase {
            model: &'static str,
            dims: usize,
        }

        const LIVE_MODEL_CASES: &[LiveModelCase] = &[
            LiveModelCase {
                model: "sentence-transformers/all-minilm-l6-v2",
                dims: 384,
            },
            LiveModelCase {
                model: "thenlper/gte-base",
                dims: 768,
            },
            LiveModelCase {
                model: "intfloat/e5-large-v2",
                dims: 1024,
            },
        ];

        #[derive(Debug, Serialize)]
        struct LiveRunArtifact {
            model: String,
            dims: usize,
            batch_size: usize,
            elapsed_ms: u128,
            vectors_head8: Vec<Vec<f32>>,
        }

        fn require_live_gate() {
            let key_ok = std::env::var("OPENROUTER_API_KEY")
                .ok()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if !key_ok {
                panic!(
                    "live gate not satisfied: set OPENROUTER_API_KEY (live embeddings hit OpenRouter)"
                );
            }
            if std::env::var("OPENROUTER_EMBEDDINGS_URL").is_ok() {
                panic!(
                    "live gate not satisfied: OPENROUTER_EMBEDDINGS_URL is set; unset it to hit real OpenRouter"
                );
            }
        }

        fn repo_root() -> PathBuf {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .canonicalize()
                .expect("repo root canonicalization failed")
        }

        fn fixture_path() -> PathBuf {
            repo_root().join("fixtures/openrouter/embeddings_models.json")
        }

        fn artifact_dir() -> PathBuf {
            repo_root()
                .join("target")
                .join("test-output")
                .join("openrouter_backend_live")
        }

        fn ts_slug() -> String {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0));
            format!("{}-{}", now.as_secs(), now.subsec_nanos())
        }

        fn sanitize_path_component(raw: &str) -> String {
            raw.replace('/', "_slash_").replace('\\', "_")
        }

        fn load_fixture_models() -> Result<FixtureModels, Box<dyn std::error::Error>> {
            let bytes = fs::read(fixture_path())?;
            let fixture: FixtureModels = serde_json::from_slice(&bytes)?;
            Ok(fixture)
        }

        fn openrouter_live_cfg(model: &str, dims: usize) -> OpenRouterConfig {
            OpenRouterConfig {
                model: model.to_string(),
                dimensions: Some(dims),
                request_dimensions: None,
                max_in_flight: 2,
                requests_per_second: None,
                max_attempts: 4,
                initial_backoff_ms: 500,
                max_backoff_ms: 10_000,
                input_type: Some("code-snippet".into()),
                timeout_secs: 40,
                truncate_policy: TruncatePolicy::Truncate,
            }
        }

        fn write_artifact(
            path: &Path,
            artifact: &LiveRunArtifact,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let bytes = serde_json::to_vec_pretty(artifact)?;
            fs::write(path, bytes)?;
            Ok(())
        }
    }
}
