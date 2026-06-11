#![allow(
    dead_code,
    unused_variables,
    reason = "evolving api surface, may be useful, written 2025-12-15"
)]

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::{env, fmt};

use chrono::{DateTime, Utc};
use ploke_core::ArcStr;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::info;
use tracing::warn;

use crate::HTTP_REFERER;
use crate::HTTP_TITLE;
use crate::error::ApiErrorSource;
use crate::error::{HttpBodyFailure, HttpFailure, HttpReceivePhase, HttpSendFailure};
use crate::manager::builders::attempt::{
    AttemptBuilder, NonStreaming, ProviderAttempt, ProviderFailurePhase, ProviderRetryDecision,
};
use crate::registry::calibration::{AttemptTimeout, ProviderTiming, RetryTuning};
use crate::response::FinishReason;
use crate::response::OpenAiResponse;
use crate::response::ToolCall;
use crate::router_only::openrouter::providers::ProviderName;
use crate::router_only::{ChatCompRequest, Router};

use super::LlmError;

#[derive(Debug, PartialEq)]
pub enum ChatStepOutcome {
    Content {
        content: Option<ArcStr>,
        reasoning: Option<ArcStr>,
    },
    ToolCalls {
        calls: Vec<ToolCall>,
        content: Option<ArcStr>,
        reasoning: Option<ArcStr>,
        finish_reason: FinishReason,
    },
}

#[derive(Debug, Deserialize)]
struct ProviderResponseObservation {
    #[serde(default)]
    provider: Option<ProviderName>,
    #[serde(default)]
    error: Option<ProviderEmbeddedError>,
}

impl ProviderResponseObservation {
    fn from_body(body_text: &str) -> Option<Self> {
        serde_json::from_str(body_text).ok()
    }

    fn provider_slug(&self) -> Option<ArcStr> {
        self.provider
            .as_ref()
            .and_then(ProviderName::to_slug)
            .map(|slug| ArcStr::from(slug.as_str()))
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ProviderEmbeddedError {
    Record(ProviderEmbeddedErrorRecord),
    Ignored(serde::de::IgnoredAny),
}

impl ProviderEmbeddedError {
    fn message(&self) -> &str {
        match self {
            Self::Record(record) => record.message(),
            Self::Ignored(_) => "Unknown provider error",
        }
    }

    fn api_code(&self) -> Option<ArcStr> {
        match self {
            Self::Record(record) => record.api_code(),
            Self::Ignored(_) => None,
        }
    }

    fn status(&self) -> u16 {
        match self {
            Self::Record(record) => record.status(),
            Self::Ignored(_) => 200,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ProviderEmbeddedErrorRecord {
    message: Option<String>,
    #[serde(default)]
    code: Option<ProviderApiCodeProjection>,
    #[serde(default)]
    status: Option<ProviderStatusProjection>,
}

impl ProviderEmbeddedErrorRecord {
    fn message(&self) -> &str {
        self.message.as_deref().unwrap_or("Unknown provider error")
    }

    fn api_code(&self) -> Option<ArcStr> {
        self.code
            .as_ref()
            .and_then(ProviderApiCodeProjection::arcstr)
    }

    fn status(&self) -> u16 {
        self.status
            .as_ref()
            .and_then(ProviderStatusProjection::u16)
            .unwrap_or(200)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ProviderApiCodeProjection {
    String(String),
    Signed(i64),
    Unsigned(u64),
    Ignored(serde::de::IgnoredAny),
}

impl ProviderApiCodeProjection {
    fn arcstr(&self) -> Option<ArcStr> {
        match self {
            Self::String(value) => Some(ArcStr::from(value.as_str())),
            Self::Signed(value) => Some(ArcStr::from(value.to_string())),
            Self::Unsigned(value) => Some(ArcStr::from(value.to_string())),
            Self::Ignored(_) => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ProviderStatusProjection {
    Number(u64),
    String(String),
    Ignored(serde::de::IgnoredAny),
}

impl ProviderStatusProjection {
    fn u16(&self) -> Option<u16> {
        match self {
            Self::Number(value) => u16::try_from(*value).ok(),
            Self::String(value) => value.parse().ok(),
            Self::Ignored(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatHttpConfig {
    referer: &'static str,
    title: &'static str,
    pub attempt_timeout: AttemptTimeout,
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    /// Optional total wall-clock budget for the retry sequence of a single
    /// chat step. `None` means the sequence is bounded only by `max_attempts`
    /// and `attempt_timeout`; `Some(budget)` additionally (a) stops *scheduling*
    /// further retries once the request has spent `budget` since it began, and
    /// (b) clamps each retry attempt's own timeout to the remaining budget (see
    /// [`effective_attempt_timeout`]).
    ///
    /// The budget does **not** bound the first/initial attempt: that attempt
    /// always runs out its full `attempt_timeout`, so a normal single attempt is
    /// never truncated. This matters because `budget` can be smaller than one
    /// `attempt_timeout` — as on the direct-Google path, where the budget is
    /// ~60s while `attempt_timeout` defaults to `LLM_TIMEOUT_SECS` — and clamping
    /// the first attempt to `budget` would cut off a legitimate single attempt.
    ///
    /// Retries (`attempt >= 2`) are bounded to the remaining budget, so they
    /// cannot extend the sequence past `budget`. The only way a chat step
    /// overshoots `budget` is therefore the first attempt running long: the
    /// worst-case wall-clock is one first-attempt `attempt_timeout` (not an
    /// additional full `attempt_timeout` per retry).
    pub max_total_elapsed: Option<Duration>,
    pub retry: RetryTuning,
}

impl Default for ChatHttpConfig {
    fn default() -> Self {
        Self {
            referer: HTTP_REFERER,
            title: HTTP_TITLE,
            attempt_timeout: AttemptTimeout::default(),
            max_attempts: 1,
            initial_backoff: Duration::from_millis(250),
            max_backoff: Duration::from_secs(2),
            max_total_elapsed: None,
            retry: RetryTuning::default(),
        }
    }
}

impl From<&ProviderTiming> for ChatHttpConfig {
    fn from(timing: &ProviderTiming) -> Self {
        Self {
            attempt_timeout: timing.attempt_timeout.clone(),
            max_attempts: timing.max_attempts,
            initial_backoff: timing.initial_backoff,
            max_backoff: timing.max_backoff,
            max_total_elapsed: timing.max_total_elapsed,
            retry: timing.retry.clone(),
            ..Self::default()
        }
    }
}

static NEXT_CHAT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
static CHAT_HTTP_STDERR: OnceLock<bool> = OnceLock::new();

fn chat_http_stderr_enabled() -> bool {
    *CHAT_HTTP_STDERR.get_or_init(|| {
        env::var("PLOKE_PROTOCOL_DEBUG").is_ok_and(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty() && normalized != "0" && normalized != "false"
        })
    })
}

fn emit_chat_http_stderr_line(payload: impl Serialize) {
    if chat_http_stderr_enabled()
        && let Ok(line) = serde_json::to_string(&payload)
    {
        eprintln!("{line}");
    }
}

#[derive(Debug, Serialize)]
struct ChatHttpStatusErrorObservation<'a> {
    event: &'static str,
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    url: &'a str,
    status: u16,
    retry_after_ms: Option<u64>,
    elapsed_ms: u64,
}

#[derive(Debug, Serialize)]
struct ChatHttpRequestErrorObservation<'a> {
    event: &'static str,
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    phase: &'a str,
    url: &'a str,
    status: Option<u16>,
    failure: &'a str,
    receive_phase: Option<&'a str>,
    body_failure: Option<&'a str>,
    elapsed_ms: u64,
    is_timeout: bool,
    raw_error: &'a str,
}

#[derive(Debug, Serialize)]
struct ChatHttpRetryScheduledObservation<'a> {
    event: &'static str,
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    phase: &'a str,
    url: &'a str,
    status: Option<u16>,
    backoff_ms: u64,
    elapsed_ms: u64,
}

#[derive(Debug, Serialize)]
struct ChatHttpRetrySuppressedObservation<'a> {
    event: &'static str,
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    phase: &'a str,
    url: &'a str,
    status: Option<u16>,
    body_failure: Option<&'a str>,
    reason: &'static str,
    elapsed_ms: u64,
}

pub async fn chat_step<R: Router>(
    client: &reqwest::Client,
    req: &ChatCompRequest<R>,
    cfg: &ChatHttpConfig,
) -> Result<ChatStepData, LlmError> {
    chat_step_with_attempts(client, req, cfg)
        .await
        .map_err(|error| error.source)
}

pub async fn chat_step_with_attempts<R: Router>(
    client: &reqwest::Client,
    req: &ChatCompRequest<R>,
    cfg: &ChatHttpConfig,
) -> Result<ChatStepData, ChatStepError> {
    let url = R::completion_url().map_err(|e| {
        ChatStepError::new(LlmError::Http(HttpFailure::send(
            None,
            None,
            format!("failed to resolve completion url: {e}"),
            HttpSendFailure::Failed,
        )))
    })?;
    let request_id = NEXT_CHAT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let mut provider_attempts = Vec::new();
    let api_key = R::resolve_bearer_token().await.map_err(|e| {
        ChatStepError::new(LlmError::Http(HttpFailure::send(
            None,
            None,
            format!("failed to resolve bearer token: {e}"),
            HttpSendFailure::Failed,
        )))
    })?;

    let request_json = serde_json::to_string_pretty(req).ok();
    let request_bytes = request_json.as_ref().map_or(0, |body| body.len());
    let message_count = req.core.messages.len();
    let tool_count = req.tools.as_ref().map_or(0, Vec::len);
    let max_attempts = cfg.max_attempts.max(1);
    if let Some(body) = request_json.as_ref() {
        let _ = log_api_request_json(url, body);
    }
    let chat_step_start = Instant::now();
    for attempt in 1..=max_attempts {
        let attempt_timeout =
            effective_attempt_timeout(cfg, attempt, chat_step_start.elapsed());
        let mut attempt_record =
            AttemptBuilder::<NonStreaming<R>>::non_streaming_from(chat_step_start);
        trace_chat_http_start(
            request_id,
            attempt,
            max_attempts,
            url,
            &req.core.model.to_string(),
            attempt_timeout,
            message_count,
            tool_count,
            request_bytes,
        );

        let request_builder = client
            .post(url)
            .bearer_auth(&api_key)
            .header("Accept", "application/json")
            .header("HTTP-Referer", cfg.referer)
            .header("X-Title", cfg.title)
            .json(req)
            .timeout(attempt_timeout);
        attempt_record = attempt_record.request_sent();
        let resp = match request_builder.send().await {
            Ok(resp) => {
                attempt_record = attempt_record.headers_received();
                resp
            }
            Err(error) => {
                attempt_record = attempt_record.failed();
                let attempt_elapsed = attempt_record
                    .failed_elapsed()
                    .unwrap_or_else(|| attempt_record.current_elapsed());
                let send_failure = if error.is_timeout() {
                    HttpSendFailure::Timeout
                } else {
                    HttpSendFailure::Failed
                };
                trace_chat_http_error(ChatHttpErrorTrace {
                    request_id,
                    attempt,
                    max_attempts,
                    phase: "send",
                    url,
                    status: None,
                    failure: send_failure.as_str(),
                    receive_phase: None,
                    body_failure: None,
                    elapsed: attempt_elapsed,
                    is_timeout: error.is_timeout(),
                    raw_error: &error.to_string(),
                });
                let failure = LlmError::Http(HttpFailure::send(
                    Some(url.to_string()),
                    Some(attempt_elapsed.as_millis()),
                    format!("sending request to {url}: {error}"),
                    send_failure.clone(),
                ));
                let should_retry = should_retry_send_error(&error, &cfg.retry);
                if should_retry
                    && attempt < max_attempts
                    && let Some(backoff) =
                        schedule_retry_backoff(cfg, attempt, None, chat_step_start.elapsed())
                {
                    trace_chat_http_retry_scheduled(
                        request_id,
                        attempt,
                        max_attempts,
                        url,
                        "send",
                        None,
                        backoff,
                        attempt_elapsed,
                    );
                    provider_attempts.push(
                        attempt_record
                            .failure_phase(ProviderFailurePhase::Send)
                            .retry_decision(ProviderRetryDecision::Scheduled)
                            .backoff(backoff)
                            .finish_traced(request_id, attempt, max_attempts),
                    );
                    sleep(backoff).await;
                    continue;
                }
                let retry_decision = if should_retry {
                    ProviderRetryDecision::Exhausted
                } else {
                    ProviderRetryDecision::NotRetryable
                };
                provider_attempts.push(
                    attempt_record
                        .failure_phase(ProviderFailurePhase::Send)
                        .retry_decision(retry_decision)
                        .finish_traced(request_id, attempt, max_attempts),
                );
                return Err(ChatStepError::with_provider_attempts(
                    failure,
                    provider_attempts,
                ));
            }
        };
        let headers_elapsed = attempt_record
            .headers_received_elapsed()
            .expect("headers must be marked after a successful send");

        let resp_url = resp.url().to_string();
        let status = resp.status().as_u16();
        let retry_after = parse_retry_after(resp.headers());
        trace_chat_http_headers(
            request_id,
            attempt,
            max_attempts,
            &resp_url,
            status,
            retry_after,
            headers_elapsed,
        );

        let body = match resp.text().await {
            Ok(body) => {
                attempt_record = attempt_record
                    .body_received()
                    .status(status)
                    .response_bytes(body.len());
                body
            }
            Err(error) => {
                attempt_record = attempt_record.failed();
                let attempt_elapsed = attempt_record
                    .failed_elapsed()
                    .unwrap_or_else(|| attempt_record.current_elapsed());
                let body_failure = classify_body_failure(&error);
                trace_chat_http_error(ChatHttpErrorTrace {
                    request_id,
                    attempt,
                    max_attempts,
                    phase: "body",
                    url: &resp_url,
                    status: Some(status),
                    failure: "receive",
                    receive_phase: Some("body"),
                    body_failure: Some(body_failure.as_str()),
                    elapsed: attempt_elapsed,
                    is_timeout: error.is_timeout(),
                    raw_error: &error.to_string(),
                });
                let failure = LlmError::Http(HttpFailure::receive(
                    Some(resp_url.clone()),
                    Some(attempt_elapsed.as_millis()),
                    Some(status),
                    format!("while reading response body (status {status}): {error}"),
                    HttpReceivePhase::Body(body_failure.clone()),
                ));
                let should_retry =
                    should_retry_body_failure(status, &body_failure, attempt, &cfg.retry);
                if should_retry && attempt < max_attempts {
                    if let Some(backoff) =
                        schedule_retry_backoff(cfg, attempt, retry_after, chat_step_start.elapsed())
                    {
                        trace_chat_http_retry_scheduled(
                            request_id,
                            attempt,
                            max_attempts,
                            &resp_url,
                            "body",
                            Some(status),
                            backoff,
                            attempt_elapsed,
                        );
                        provider_attempts.push(
                            attempt_record
                                .status(status)
                                .failure_phase(ProviderFailurePhase::Body)
                                .body_failure(body_failure.clone())
                                .retry_decision(ProviderRetryDecision::Scheduled)
                                .backoff(backoff)
                                .finish_traced(request_id, attempt, max_attempts),
                        );
                        sleep(backoff).await;
                        continue;
                    }
                    // Retry budget exhausted: fall through to the Exhausted
                    // decision below rather than scheduling another attempt.
                } else if attempt < max_attempts {
                    trace_chat_http_retry_suppressed(
                        request_id,
                        attempt,
                        max_attempts,
                        &resp_url,
                        "body",
                        Some(status),
                        Some(body_failure.as_str()),
                        attempt_elapsed,
                    );
                }
                let retry_decision = if should_retry {
                    ProviderRetryDecision::Exhausted
                } else if attempt < max_attempts {
                    ProviderRetryDecision::Suppressed
                } else {
                    ProviderRetryDecision::NotRetryable
                };
                provider_attempts.push(
                    attempt_record
                        .status(status)
                        .failure_phase(ProviderFailurePhase::Body)
                        .body_failure(body_failure)
                        .retry_decision(retry_decision)
                        .finish_traced(request_id, attempt, max_attempts),
                );
                return Err(ChatStepError::with_provider_attempts(
                    failure,
                    provider_attempts,
                ));
            }
        };
        let body_elapsed = attempt_record
            .output_completed_elapsed()
            .expect("body completion must be marked after response text is read");

        trace_chat_http_response_body(
            request_id,
            attempt,
            max_attempts,
            &resp_url,
            status,
            body.len(),
            body_elapsed,
        );

        let _ = log_api_raw_response(&resp_url, status, &body);

        if let Ok(parsed) = &serde_json::from_str(&body) {
            let _ = log_api_parsed_json_response(&resp_url, status, parsed).await;
        } else {
            let _ = log_api_raw_response(url, status, &body);
        }

        if !(200..300).contains(&status) {
            trace_chat_http_status_error(
                request_id,
                attempt,
                max_attempts,
                &resp_url,
                status,
                retry_after,
                body_elapsed,
            );
            let should_retry = should_retry_status(status, &cfg.retry);
            if should_retry
                && attempt < max_attempts
                && let Some(backoff) =
                    schedule_retry_backoff(cfg, attempt, retry_after, chat_step_start.elapsed())
            {
                trace_chat_http_retry_scheduled(
                    request_id,
                    attempt,
                    max_attempts,
                    &resp_url,
                    "status",
                    Some(status),
                    backoff,
                    body_elapsed,
                );
                provider_attempts.push(
                    attempt_record
                        .failure_phase(ProviderFailurePhase::Status)
                        .retry_decision(ProviderRetryDecision::Scheduled)
                        .backoff(backoff)
                        .finish_traced(request_id, attempt, max_attempts),
                );
                sleep(backoff).await;
                continue;
            }
            let retry_decision = if should_retry {
                ProviderRetryDecision::Exhausted
            } else {
                ProviderRetryDecision::NotRetryable
            };
            provider_attempts.push(
                attempt_record
                    .failure_phase(ProviderFailurePhase::Status)
                    .retry_decision(retry_decision)
                    .finish_traced(request_id, attempt, max_attempts),
            );
            return Err(ChatStepError::with_provider_attempts(
                LlmError::Api {
                    status,
                    message: body.clone(),
                    url: Some(resp_url),
                    body_snippet: Some(truncate_for_error(&body, 4_096)),
                    api_code: extract_api_code_from_body(&body),
                    provider_name: extract_provider_name_from_body(&body)
                        .map(|name| ArcStr::from(name.as_str())),
                    provider_slug: extract_provider_slug_from_body(&body),
                    error_source: ApiErrorSource::HttpStatusBody,
                },
                provider_attempts,
            ));
        }

        let parsed = match parse_chat_outcome(&body) {
            Ok(parsed) => parsed,
            Err(error) => {
                provider_attempts.push(attempt_record.finish_traced(
                    request_id,
                    attempt,
                    max_attempts,
                ));
                return Err(ChatStepError::with_provider_attempts(
                    error,
                    provider_attempts,
                ));
            }
        };
        trace_chat_http_completed(
            request_id,
            attempt,
            max_attempts,
            &resp_url,
            status,
            body_elapsed,
        );
        provider_attempts.push(attempt_record.finish_traced(request_id, attempt, max_attempts));
        return Ok(parsed.with_provider_attempts(provider_attempts));
    }

    unreachable!("chat_step retry loop should always return")
}

async fn log_api_parsed_json_response(
    url: &str,
    status: u16,
    parsed: &OpenAiResponse,
) -> color_eyre::Result<()> {
    let payload: String = serde_json::to_string_pretty(parsed)?;
    tracing::info!(target: "api_json", "\n// URL: {url}\n// Status: {status}\n{payload}\n");
    Ok(())
}

fn log_api_raw_response(url: &str, status: u16, body: &str) -> color_eyre::Result<()> {
    tracing::info!(target: "api_json", "\n// URL: {url}\n// Status: {status}\n{body}\n");
    Ok(())
}

fn log_api_request_json(url: &str, payload: &str) -> color_eyre::Result<()> {
    tracing::info!(target: "api_json", "\n// URL: {url}\n// Request\n{payload}\n");
    Ok(())
}

fn trace_chat_http_start(
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    url: &str,
    model: &str,
    timeout: Duration,
    message_count: usize,
    tool_count: usize,
    request_bytes: usize,
) {
    emit_chat_http_stderr_line(serde_json::json!({
        "event": "chat_http_request_start",
        "request_id": request_id,
        "attempt": attempt,
        "max_attempts": max_attempts,
        "url": url,
        "model": model,
        "timeout_secs": timeout.as_secs(),
        "message_count": message_count,
        "tool_count": tool_count,
        "request_bytes": request_bytes
    }));
    tracing::info!(
        target: "chat_http",
        event = "chat_http_request_start",
        request_id,
        attempt,
        max_attempts,
        url,
        model,
        timeout_secs = timeout.as_secs(),
        message_count,
        tool_count,
        request_bytes
    );
}

fn trace_chat_http_headers(
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    url: &str,
    status: u16,
    retry_after: Option<Duration>,
    elapsed: Duration,
) {
    emit_chat_http_stderr_line(serde_json::json!({
        "event": "chat_http_response_headers",
        "request_id": request_id,
        "attempt": attempt,
        "max_attempts": max_attempts,
        "url": url,
        "status": status,
        "retry_after_ms": retry_after.map(|value| value.as_millis() as u64),
        "elapsed_ms": elapsed.as_millis()
    }));
    tracing::info!(
        target: "chat_http",
        event = "chat_http_response_headers",
        request_id,
        attempt,
        max_attempts,
        url,
        status,
        retry_after_ms = retry_after.map(|value| value.as_millis() as u64),
        elapsed_ms = elapsed.as_millis()
    );
}

fn trace_chat_http_response_body(
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    url: &str,
    status: u16,
    response_bytes: usize,
    elapsed: Duration,
) {
    emit_chat_http_stderr_line(serde_json::json!({
        "event": "chat_http_response_body",
        "request_id": request_id,
        "attempt": attempt,
        "max_attempts": max_attempts,
        "url": url,
        "status": status,
        "response_bytes": response_bytes,
        "elapsed_ms": elapsed.as_millis()
    }));
    tracing::info!(
        target: "chat_http",
        event = "chat_http_response_body",
        request_id,
        attempt,
        max_attempts,
        url,
        status,
        response_bytes,
        elapsed_ms = elapsed.as_millis()
    );
}

fn trace_chat_http_status_error(
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    url: &str,
    status: u16,
    retry_after: Option<Duration>,
    elapsed: Duration,
) {
    emit_chat_http_stderr_line(ChatHttpStatusErrorObservation {
        event: "chat_http_response_error_status",
        request_id,
        attempt,
        max_attempts,
        url,
        status,
        retry_after_ms: retry_after.map(duration_ms),
        elapsed_ms: duration_ms(elapsed),
    });
    tracing::warn!(
        target: "chat_http",
        event = "chat_http_response_error_status",
        request_id,
        attempt,
        max_attempts,
        url,
        status,
        retry_after_ms = retry_after.map(|value| value.as_millis() as u64),
        elapsed_ms = elapsed.as_millis()
    );
}

fn trace_chat_http_retry_scheduled(
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    url: &str,
    phase: &str,
    status: Option<u16>,
    backoff: Duration,
    elapsed: Duration,
) {
    emit_chat_http_stderr_line(ChatHttpRetryScheduledObservation {
        event: "chat_http_retry_scheduled",
        request_id,
        attempt,
        max_attempts,
        phase,
        url,
        status,
        backoff_ms: duration_ms(backoff),
        elapsed_ms: duration_ms(elapsed),
    });
    tracing::warn!(
        target: "chat_http",
        event = "chat_http_retry_scheduled",
        request_id,
        attempt,
        max_attempts,
        phase,
        url,
        status,
        backoff_ms = backoff.as_millis(),
        elapsed_ms = elapsed.as_millis()
    );
}

fn trace_chat_http_completed(
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    url: &str,
    status: u16,
    elapsed: Duration,
) {
    emit_chat_http_stderr_line(serde_json::json!({
        "event": "chat_http_request_completed",
        "request_id": request_id,
        "attempt": attempt,
        "max_attempts": max_attempts,
        "url": url,
        "status": status,
        "elapsed_ms": elapsed.as_millis()
    }));
    tracing::info!(
        target: "chat_http",
        event = "chat_http_request_completed",
        request_id,
        attempt,
        max_attempts,
        url,
        status,
        elapsed_ms = elapsed.as_millis()
    );
}

struct ChatHttpErrorTrace<'a> {
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    phase: &'a str,
    url: &'a str,
    status: Option<u16>,
    failure: &'a str,
    receive_phase: Option<&'a str>,
    body_failure: Option<&'a str>,
    elapsed: Duration,
    is_timeout: bool,
    raw_error: &'a str,
}

fn trace_chat_http_error(event: ChatHttpErrorTrace<'_>) {
    emit_chat_http_stderr_line(ChatHttpRequestErrorObservation {
        event: "chat_http_request_error",
        request_id: event.request_id,
        attempt: event.attempt,
        max_attempts: event.max_attempts,
        phase: event.phase,
        url: event.url,
        status: event.status,
        failure: event.failure,
        receive_phase: event.receive_phase,
        body_failure: event.body_failure,
        elapsed_ms: duration_ms(event.elapsed),
        is_timeout: event.is_timeout,
        raw_error: event.raw_error,
    });
    tracing::warn!(
        target: "chat_http",
        event = "chat_http_request_error",
        request_id = event.request_id,
        attempt = event.attempt,
        max_attempts = event.max_attempts,
        phase = event.phase,
        url = event.url,
        status = event.status,
        failure = event.failure,
        receive_phase = event.receive_phase,
        body_failure = event.body_failure,
        elapsed_ms = event.elapsed.as_millis(),
        is_timeout = event.is_timeout,
        raw_error = event.raw_error
    );
}

fn trace_chat_http_retry_suppressed(
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    url: &str,
    phase: &str,
    status: Option<u16>,
    body_failure: Option<&str>,
    elapsed: Duration,
) {
    emit_chat_http_stderr_line(ChatHttpRetrySuppressedObservation {
        event: "chat_http_retry_suppressed",
        request_id,
        attempt,
        max_attempts,
        phase,
        url,
        status,
        body_failure,
        reason: "classified_non_retryable",
        elapsed_ms: duration_ms(elapsed),
    });
    tracing::warn!(
        target: "chat_http",
        event = "chat_http_retry_suppressed",
        request_id,
        attempt,
        max_attempts,
        phase,
        url,
        status,
        body_failure,
        reason = "classified_non_retryable",
        elapsed_ms = elapsed.as_millis()
    );
}

fn should_retry_send_error(error: &reqwest::Error, tuning: &RetryTuning) -> bool {
    if error.is_timeout() {
        return tuning.retry_send_timeout;
    }
    (error.is_connect() || error.is_request() || error.is_body()) && tuning.retry_send_failure
}

fn should_retry_body_failure(
    status: u16,
    failure: &HttpBodyFailure,
    attempt: u32,
    tuning: &RetryTuning,
) -> bool {
    let _ = status;
    match failure {
        HttpBodyFailure::Timeout => {
            tuning.retry_body_timeout
                && tuning
                    .body_timeout_retry_limit
                    .is_none_or(|limit| attempt <= limit)
        }
        HttpBodyFailure::ReadFailed => tuning.retry_body_read_failed,
        HttpBodyFailure::DecodeFailed => false,
    }
}

fn classify_body_failure(error: &reqwest::Error) -> HttpBodyFailure {
    if error.is_timeout() {
        HttpBodyFailure::Timeout
    } else if error.is_decode() {
        HttpBodyFailure::DecodeFailed
    } else {
        HttpBodyFailure::ReadFailed
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis() as u64
}

impl HttpSendFailure {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Failed => "failed",
        }
    }
}

impl HttpBodyFailure {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::ReadFailed => "read_failed",
            Self::DecodeFailed => "decode_failed",
        }
    }
}

fn should_retry_status(status: u16, tuning: &RetryTuning) -> bool {
    tuning.retry_statuses.contains(&status)
}

/// Deterministic upper bound of the exponential backoff for `attempt`:
/// `min(initial_backoff * 2^(attempt-1), max_backoff)`.
///
/// This is the *ceiling*; the actual sleep is a full-jitter sample within
/// `[0, ceiling]` (see [`full_jitter`]). Kept separate so the exponential
/// schedule can be asserted deterministically in tests.
fn backoff_ceiling(cfg: &ChatHttpConfig, attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1);
    let multiplier = 1u32.checked_shl(exponent.min(16)).unwrap_or(u32::MAX);
    let backoff = cfg.initial_backoff.saturating_mul(multiplier);
    backoff.min(cfg.max_backoff)
}

/// Full-jitter sample in `[0, ceiling]`.
///
/// Full jitter (`random_between(0, ceiling)`) decorrelates concurrent retries.
/// This matters for the direct-Google path because Prototype 1 fires several
/// parallel child attempts that can all hit Vertex DSQ 429s at the same instant;
/// a deterministic backoff would resynchronize them into a fresh burst, whereas
/// jitter spreads the retries out.
fn full_jitter(ceiling: Duration) -> Duration {
    let ceiling_nanos = ceiling.as_nanos();
    if ceiling_nanos == 0 {
        return Duration::ZERO;
    }
    let sampled = (ceiling_nanos as f64 * next_jitter_unit()) as u128;
    let nanos = sampled.min(ceiling_nanos).min(u128::from(u64::MAX)) as u64;
    Duration::from_nanos(nanos)
}

/// Cheap, dependency-free, non-cryptographic PRNG returning a value in `[0, 1)`.
///
/// Used only to jitter retry backoff. A process-wide atomic counter (advanced by
/// an odd constant) mixed with the wall clock gives well-spread values across
/// concurrent calls without pulling in a `rand` dependency.
fn next_jitter_unit() -> f64 {
    static STATE: AtomicU64 = AtomicU64::new(0);
    let counter = STATE.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed);
    let clock = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64);
    // xorshift64* on the mixed seed.
    let mut x = counter ^ clock ^ 0x2545_F491_4F6C_DD1D;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    let x = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
    // Take the top 53 bits for an evenly distributed f64 in [0, 1).
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}

/// Effective per-attempt HTTP timeout.
///
/// The first attempt always receives the full configured `attempt_timeout`, so a
/// normal single attempt is never truncated even when the total budget is
/// smaller than one attempt timeout (as on the direct-Google path, where the
/// budget is ~60s but `attempt_timeout` defaults to `LLM_TIMEOUT_SECS`).
///
/// Retries (`attempt >= 2`) are additionally bounded to the remaining total
/// budget (`max_total_elapsed - elapsed`), so an in-flight retry cannot push the
/// chat step past `max_total_elapsed`. When no budget is configured (the legacy
/// OpenRouter path) retries keep the full `attempt_timeout`.
fn effective_attempt_timeout(cfg: &ChatHttpConfig, attempt: u32, elapsed: Duration) -> Duration {
    let base = cfg.attempt_timeout.for_attempt(attempt);
    if attempt <= 1 {
        return base;
    }
    match cfg.max_total_elapsed {
        Some(budget) => base.min(budget.saturating_sub(elapsed)),
        None => base,
    }
}

/// Decide the sleep before the next retry, or `None` to stop retrying.
///
/// This governs only whether (and for how long) to wait before *scheduling* the
/// next attempt; it never bounds an attempt already in flight (those are bounded
/// by [`effective_attempt_timeout`]). See [`ChatHttpConfig::max_total_elapsed`]
/// for the resulting worst-case overshoot.
///
/// - A server-provided `Retry-After` / RetryInfo delay is honored as-is, but
///   never beyond the remaining total budget (when one is configured); if it
///   cannot be honored within budget we stop retrying rather than waking early.
/// - Otherwise we use a full-jitter exponential backoff, capped to the remaining
///   budget.
/// - When `cfg.max_total_elapsed` is `None` the budget is unbounded, preserving
///   legacy behavior for routers that have not opted in.
fn schedule_retry_backoff(
    cfg: &ChatHttpConfig,
    attempt: u32,
    retry_after: Option<Duration>,
    elapsed: Duration,
) -> Option<Duration> {
    let remaining = match cfg.max_total_elapsed {
        Some(budget) => budget.checked_sub(elapsed).filter(|left| !left.is_zero())?,
        None => Duration::MAX,
    };

    let backoff = match retry_after {
        Some(delay) => match cfg.max_total_elapsed {
            // Budgeted (direct-Google): honor the server delay, but only if we
            // can wait it out within the remaining budget.
            Some(_) => {
                if delay > remaining {
                    return None;
                }
                delay
            }
            // Unbudgeted (legacy): preserve the historical max_backoff cap.
            None => delay.min(cfg.max_backoff),
        },
        None => full_jitter(backoff_ceiling(cfg, attempt)).min(remaining),
    };
    Some(backoff)
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    let header = headers.get(reqwest::header::RETRY_AFTER)?;
    let raw = header.to_str().ok()?.trim();
    if raw.is_empty() {
        return None;
    }

    if let Ok(seconds) = raw.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    let retry_at = DateTime::parse_from_rfc2822(raw).ok()?.with_timezone(&Utc);
    let now = Utc::now();
    let delta = retry_at.signed_duration_since(now);
    delta.to_std().ok()
}

#[derive(Debug)]
pub struct ChatStepData {
    pub outcome: ChatStepOutcome,
    pub full_response: OpenAiResponse,
    pub provider_attempts: Vec<ProviderAttempt>,
}

#[derive(Debug)]
pub struct ChatStepDataBuilder {
    pub outcome: Option<ChatStepOutcome>,
    pub full_response: Option<OpenAiResponse>,
    pub provider_attempts: Vec<ProviderAttempt>,
}

impl ChatStepDataBuilder {
    pub fn new() -> Self {
        Self {
            outcome: None,
            full_response: None,
            provider_attempts: Vec::new(),
        }
    }

    pub fn outcome(mut self, outcome: ChatStepOutcome) -> Self {
        self.outcome = Some(outcome);
        self
    }

    pub fn full_response(mut self, response: OpenAiResponse) -> Self {
        self.full_response = Some(response);
        self
    }

    pub fn provider_attempts(mut self, attempts: Vec<ProviderAttempt>) -> Self {
        self.provider_attempts = attempts;
        self
    }

    pub fn build(self) -> Result<ChatStepData, LlmError> {
        let outcome = self
            .outcome
            .ok_or(LlmError::ChatStep("Outcome is required".to_string()))?;
        let full_response = self
            .full_response
            .ok_or(LlmError::ChatStep("Full response is required".to_string()))?;

        Ok(ChatStepData {
            outcome,
            full_response,
            provider_attempts: self.provider_attempts,
        })
    }
}

impl ChatStepData {
    pub fn new(outcome: ChatStepOutcome, full_response: OpenAiResponse) -> Self {
        Self {
            outcome,
            full_response,
            provider_attempts: Vec::new(),
        }
    }

    pub fn with_provider_attempts(mut self, attempts: Vec<ProviderAttempt>) -> Self {
        self.provider_attempts = attempts;
        self
    }
}

#[derive(Debug)]
pub struct ChatStepError {
    pub source: LlmError,
    pub provider_attempts: Vec<ProviderAttempt>,
}

impl ChatStepError {
    pub fn new(source: LlmError) -> Self {
        Self {
            source,
            provider_attempts: Vec::new(),
        }
    }

    pub fn with_provider_attempts(
        source: LlmError,
        provider_attempts: Vec<ProviderAttempt>,
    ) -> Self {
        Self {
            source,
            provider_attempts,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResponseIndex(pub usize);

impl ResponseIndex {
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    pub fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for ResponseIndex {
    fn from(index: usize) -> Self {
        Self::new(index)
    }
}

impl fmt::Display for ResponseIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedResponse {
    pub response_index: ResponseIndex,
    pub response: OpenAiResponse,
}

impl RecordedResponse {
    pub fn new(response_index: usize, response: OpenAiResponse) -> Self {
        Self {
            response_index: ResponseIndex::new(response_index),
            response,
        }
    }

    pub fn index(&self) -> usize {
        self.response_index.get()
    }
}

/// Provider-response replay tape.
///
/// This replays provider envelopes through the same parser as live responses.
/// It deliberately does not replay tool results or session events.
#[derive(Debug, Clone)]
pub struct RecordedResponseTape {
    responses: Vec<RecordedResponse>,
    cursor: usize,
}

impl RecordedResponseTape {
    pub fn new(mut responses: Vec<RecordedResponse>) -> Self {
        responses.sort_by_key(|response| response.response_index);
        Self {
            responses,
            cursor: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.responses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.responses.is_empty()
    }

    pub fn remaining(&self) -> usize {
        self.responses.len().saturating_sub(self.cursor)
    }

    pub fn next_chat_step(&mut self) -> Result<ChatStepData, ChatStepError> {
        let Some(record) = self.responses.get(self.cursor) else {
            return Err(ChatStepError::new(LlmError::ReplayExhausted(
                "recorded provider response tape exhausted".to_string(),
            )));
        };
        self.cursor += 1;
        let body = serde_json::to_string(&record.response).map_err(|source| {
            ChatStepError::new(LlmError::Serialization(format!(
                "failed to serialize recorded provider response {}: {source}",
                record.response_index
            )))
        })?;
        parse_chat_outcome(&body).map_err(ChatStepError::new)
    }
}

/// Parse a (non-streaming) OpenAI/OpenRouter-style response body into a normalized outcome.
///
/// This function is used by the *driver* (session/tool loop) to decide what to do next:
/// - If the model produced tool calls, we return `ParseOutcome::ToolCalls` so the caller can
///   execute them and then continue the conversation.
/// - Otherwise we return `ParseOutcome::Content` containing the assistant text.
/// - Streaming deltas are not supported here; if you enable streaming, route those responses to a
///   different parser.
///
/// ## Finish reason normalization
/// Some providers:
/// - omit `finish_reason`, or
/// - incorrectly set it to `"stop"` even when `tool_calls` are present.
///
/// If `tool_calls` are present, we **force** `finish_reason = FinishReason::ToolCalls` because
/// that is the only safe interpretation for a tool-driving session loop.
///
/// ## Provider-embedded errors
/// Some providers return `{ "error": ... }` in a 200 OK body. We detect that early and surface it
/// as `LlmError::Api`.
pub fn parse_chat_outcome(body_text: &str) -> Result<ChatStepData, LlmError> {
    let mut builder = ChatStepDataBuilder::new();
    let mut raw_provider_slug: Option<ArcStr> = None;
    let mut first_choice_error: Option<LlmError> = None;

    // Parse a small typed projection first so embedded provider errors do not
    // require anonymous JSON field walking.
    // If this fails, we still attempt typed parsing below to produce a more specific error.
    if let Some(observation) = ProviderResponseObservation::from_body(body_text) {
        raw_provider_slug = observation.provider_slug();

        if let Some(err) = observation.error.as_ref() {
            return Err(api_error_from_embedded_error(
                err,
                observation.provider.as_ref(),
                raw_provider_slug.clone(),
                body_text,
                ApiErrorSource::TopLevelError,
            ));
        }
    }

    let parsed: OpenAiResponse = serde_json::from_str(body_text).map_err(|e| {
        // Avoid dumping arbitrarily large bodies into errors/logs.
        let excerpt = truncate_for_error(body_text, 2_000);
        LlmError::Deserialization {
            message: format!("{e} — body excerpt: {excerpt}"),
            body_snippet: Some(excerpt),
        }
    })?;

    // We prefer the first choice that yields a usable outcome.
    for choice in parsed.choices.iter() {
        if let Some(model_behavior_reason) = choice.finish_reason.as_ref().filter(|reason| {
            matches!(
                reason,
                FinishReason::MalformedFunctionCall | FinishReason::UnexpectedToolCall
            )
        }) {
            let default_msg = match model_behavior_reason {
                FinishReason::UnexpectedToolCall => "Provider returned unexpected tool call",
                _ => "Provider returned malformed function call",
            };
            let msg = choice
                .message
                .as_ref()
                .and_then(|message| message.refusal.clone())
                .unwrap_or_else(|| default_msg.to_string());
            let finish_reason = model_behavior_reason.clone();
            return Err(LlmError::FinishError {
                msg,
                full_response: parsed,
                finish_reason,
            });
        }

        if let Some(err) = &choice.error {
            if first_choice_error.is_none() {
                first_choice_error = Some(api_error_from_choice_error(
                    err,
                    parsed.provider.as_ref(),
                    raw_provider_slug.clone(),
                    body_text,
                ));
            }
            continue;
        }

        // Case 1: Chat-style `message`
        if let Some(msg) = &choice.message {
            let calls_opt = &msg.tool_calls;
            let content_opt = &msg.content;
            let reasoning_opt = &msg.reasoning;

            // Normalize: tool calls always win.
            if let Some(calls) = calls_opt {
                // If you care about empty tool_calls arrays, you can treat empty as an error.
                // Here, empty still counts as "tool calls" because the session loop expects it.
                // - however, still warn for the logs
                if choice.finish_reason != Some(FinishReason::ToolCalls) {
                    warn!(target: "chat-loop", "FinishReason is not ToolCalls when calling tools, found finish reason: {:?}", choice.finish_reason);
                }
                info!(target: "chat-loop", "native_finish_reason, type string, is not well-understood yet. Logging to learn more:{:?}", choice.native_finish_reason);
                let finish_reason = FinishReason::ToolCalls;
                let outcome = ChatStepOutcome::ToolCalls {
                    // TODO: Find a way to get rid of this clone
                    calls: calls.clone(),
                    content: content_opt.as_deref().map(ArcStr::from),
                    finish_reason,
                    reasoning: reasoning_opt.as_deref().map(ArcStr::from),
                };
                builder = builder.outcome(outcome).full_response(parsed);

                return builder.build();
            }

            // No tool calls → return content if present.
            if let Some(text) = content_opt {
                let outcome = ChatStepOutcome::Content {
                    reasoning: reasoning_opt.as_deref().map(ArcStr::from),
                    content: Some(ArcStr::from(text.as_str())),
                };
                return builder.outcome(outcome).full_response(parsed).build();
            }

            // Coalesce reasoning to content when content is missing but reasoning is present.
            // This handles models like qwen/qwen3.6-plus that return reasoning without content.
            #[cfg(feature = "qwen_reasoning_fix")]
            if let Some(reasoning_text) = reasoning_opt {
                tracing::warn!(
                    target: "chat-loop",
                    "Model returned reasoning without content; coalescing reasoning to content"
                );
                let outcome = ChatStepOutcome::Content {
                    reasoning: None, // Already coalesced to content
                    content: Some(ArcStr::from(reasoning_text.as_str())),
                };
                return builder.outcome(outcome).full_response(parsed).build();
            }

            // If message exists but is empty, fall through to try other forms / choices.
            continue;
        }

        // Case 2: Legacy completions-style `text`
        if let Some(text) = &choice.text {
            let outcome = ChatStepOutcome::Content {
                reasoning: choice
                    .message
                    .as_ref()
                    .and_then(|m| m.reasoning.as_ref().map(|s| ArcStr::from(s.as_str()))),
                content: Some(ArcStr::from(text.as_str())),
            };
            return builder.outcome(outcome).full_response(parsed).build();
        }

        // Case 3: Streaming deltas (unsupported in this parser)
        if choice.delta.is_some() {
            return Err(LlmError::Deserialization {
                message: "Unexpected streaming delta in non-streaming parser".into(),
                body_snippet: Some(truncate_for_error(body_text, 512)),
            });
        }
    }

    if let Some(err) = first_choice_error {
        return Err(err);
    }

    Err(LlmError::Deserialization {
        message: "No usable choice in LLM response (no message/text/tool_calls)".into(),
        body_snippet: Some(truncate_for_error(body_text, 512)),
    })
}

/// Truncate large response bodies so error strings remain bounded.
fn truncate_for_error(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // Preserve a little tail too (often contains the interesting part).
        let head = &s[..max.saturating_sub(200)];
        let tail = &s[s.len().saturating_sub(200)..];
        format!("{head}…<snip>…{tail}")
    }
}

fn extract_provider_name_from_body(body_text: &str) -> Option<ProviderName> {
    ProviderResponseObservation::from_body(body_text).and_then(|observation| observation.provider)
}

fn extract_provider_slug_from_body(body_text: &str) -> Option<ArcStr> {
    ProviderResponseObservation::from_body(body_text)
        .and_then(|observation| observation.provider_slug())
}

fn extract_api_code_from_body(body_text: &str) -> Option<ArcStr> {
    ProviderResponseObservation::from_body(body_text)
        .and_then(|observation| observation.error)
        .and_then(|error| error.api_code())
}

fn api_error_from_embedded_error(
    err: &ProviderEmbeddedError,
    provider_name: Option<&ProviderName>,
    provider_slug: Option<ArcStr>,
    body_text: &str,
    error_source: ApiErrorSource,
) -> LlmError {
    let msg = err.message();
    let api_code = err.api_code();
    // Provider "code" is often not an HTTP status; it may be a string like "invalid_api_key".
    // Prefer an explicit `status` field if present, otherwise mark as 200 (embedded error).
    let status = err.status();

    let full_msg = if let Some(code) = api_code.as_ref() {
        format!("{msg} (code: {code})")
    } else {
        msg.to_string()
    };

    LlmError::Api {
        status,
        message: full_msg,
        url: None,
        body_snippet: Some(truncate_for_error(body_text, 4_096)),
        api_code,
        provider_name: provider_name.map(|name| ArcStr::from(name.as_str())),
        provider_slug,
        error_source,
    }
}

fn api_error_from_choice_error(
    err: &crate::response::ErrorResponse,
    provider_name: Option<&ProviderName>,
    raw_provider_slug: Option<ArcStr>,
    body_text: &str,
) -> LlmError {
    let api_code = ArcStr::from(err.code.to_string());
    LlmError::Api {
        status: 200,
        message: format!("{} (code: {})", err.message, err.code),
        url: None,
        body_snippet: Some(truncate_for_error(body_text, 4_096)),
        api_code: Some(api_code),
        provider_name: provider_name.map(|name| ArcStr::from(name.as_str())),
        provider_slug: provider_name
            .and_then(ProviderName::to_slug)
            .map(|slug| ArcStr::from(slug.as_str()))
            .or(raw_provider_slug),
        error_source: ApiErrorSource::ChoiceError,
    }
}

fn check_provider_error(body_text: &str) -> Result<(), LlmError> {
    // Providers sometimes put errors inside a 200 body
    match serde_json::from_str::<ProviderResponseObservation>(body_text) {
        Ok(observation) => match observation.error.as_ref() {
            Some(err) => Err(api_error_from_embedded_error(
                err,
                observation.provider.as_ref(),
                observation.provider_slug(),
                body_text,
                ApiErrorSource::TopLevelError,
            )),
            None => Err(LlmError::Deserialization {
                message: "No choices".into(),
                body_snippet: Some(truncate_for_error(body_text, 512)),
            }),
        },
        Err(e) => {
            let err_msg = format!("Failed to Deserialize to json: {e}");
            Err(LlmError::Deserialization {
                message: err_msg,
                body_snippet: Some(truncate_for_error(body_text, 512)),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_outcome_content_message() {
        let body = r#"{
            "choices": [
                { "message": {"role": "assistant", "content": "Hello world"} }
            ]
        }"#;
        let r = parse_chat_outcome(body).unwrap();
        match r.outcome {
            ChatStepOutcome::Content {
                content: Some(c), ..
            } => assert_eq!(c.as_ref(), "Hello world"),
            _ => panic!("expected content"),
        }
    }

    #[test]
    fn recorded_response_tape_replays_provider_envelopes_through_parser() {
        let response: OpenAiResponse = serde_json::from_str(
            r#"{
                "id": "recorded-1",
                "choices": [
                    {
                        "index": 0,
                        "finish_reason": "stop",
                        "message": {"role": "assistant", "content": "from tape"}
                    }
                ],
                "created": 0,
                "model": "test/model",
                "object": "chat.completion"
            }"#,
        )
        .expect("response envelope");
        let mut tape = RecordedResponseTape::new(vec![RecordedResponse::new(0, response)]);

        let step = tape.next_chat_step().expect("recorded chat step");

        match step.outcome {
            ChatStepOutcome::Content {
                content: Some(content),
                ..
            } => assert_eq!(content.as_ref(), "from tape"),
            other => panic!("expected content response, got {other:?}"),
        }
        assert_eq!(step.full_response.id, "recorded-1");
        assert_eq!(tape.remaining(), 0);
        assert!(tape.next_chat_step().is_err());
    }

    #[test]
    fn parse_outcome_text_field() {
        let body = r#"{
            "choices": [
                { "text": "Hello text" }
            ]
        }"#;
        let r = parse_chat_outcome(body).unwrap();
        match r.outcome {
            ChatStepOutcome::Content {
                content: Some(c), ..
            } => assert_eq!(c.as_ref(), "Hello text"),
            _ => panic!("expected content"),
        }
    }

    #[test]
    fn parse_outcome_preserves_provider_on_success() {
        let body = r#"{
            "provider": "OpenAI",
            "choices": [
                { "message": {"role": "assistant", "content": "Hello world"} }
            ]
        }"#;

        let r = parse_chat_outcome(body).unwrap();
        assert_eq!(
            r.full_response.provider.as_ref().map(ProviderName::as_str),
            Some("OpenAI")
        );
    }

    #[test]
    fn parse_outcome_malformed_function_call_returns_finish_error() {
        let body = r#"{
            "id": "google-malformed",
            "choices": [{
                "index": 0,
                "finish_reason": "malformed_function_call",
                "message": {
                    "role": "assistant",
                    "refusal": "Malformed function call: print(default_api.apply_code_edit(edits=[...]))"
                }
            }],
            "created": 0,
            "model": "google/gemini-2.5-flash",
            "object": "chat.completion"
        }"#;

        let err = parse_chat_outcome(body).expect_err("malformed function call should fail");
        match err {
            LlmError::FinishError {
                finish_reason, msg, ..
            } => {
                assert_eq!(finish_reason, FinishReason::MalformedFunctionCall);
                assert!(msg.contains("default_api.apply_code_edit"));
            }
            other => panic!("expected finish error, got {other:?}"),
        }
    }

    #[test]
    fn parse_outcome_unexpected_tool_call_returns_finish_error() {
        let body = r#"{
            "id": "google-unexpected-tool-call",
            "choices": [{
                "index": 0,
                "finish_reason": "unexpected_tool_call",
                "message": {
                    "role": "assistant",
                    "refusal": "Model tried to call an undeclared function: NonSemanticPatchPatches"
                }
            }],
            "created": 0,
            "model": "google/gemini-2.5-flash",
            "object": "chat.completion"
        }"#;

        let err = parse_chat_outcome(body).expect_err("unexpected tool call should fail");
        match err {
            LlmError::FinishError {
                finish_reason, msg, ..
            } => {
                assert_eq!(finish_reason, FinishReason::UnexpectedToolCall);
                assert!(msg.contains("undeclared function"));
            }
            other => panic!("expected finish error, got {other:?}"),
        }
    }

    /// Real refusal text captured from the state6 `gemini-2.5-pro` direct-Google
    /// incident (`MALFORMED_FUNCTION_CALL` finish reason). The model emitted a
    /// Python `print(default_api.non_semantic_patch(...))` call whose `diff`
    /// argument is a multi-line unified diff, which Vertex rejected as a
    /// malformed function call.
    ///
    /// Provenance: `~/.ploke-eval/instances/prototype1/`
    /// `p1-g25p-direct-protocol-2target-g0g2-1x3-state6-20260610-014509/`
    /// `BurntSushi__ripgrep-2209/runs/`
    /// `run-1781081390723-structured-current-policy-9b2c8ea1/agent-turn-summary.json`
    const MALFORMED_NON_SEMANTIC_PATCH_REFUSAL: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/google/malformed_non_semantic_patch_refusal.txt"
    ));

    #[test]
    fn parse_outcome_malformed_non_semantic_patch_multiline_diff_replays_to_finish_error() {
        // Replays the captured eval-shape Google response (finish_reason
        // `malformed_function_call` + a multi-line `non_semantic_patch` diff
        // refusal) through the production parse path. This locks in the
        // classification fix (commit 3f4d69ea) against the exact state6 payload:
        // it must surface as a MalformedFunctionCall FinishError, never as an
        // unknown tool name or repair loop, and the multi-line diff body must
        // not break parsing.
        let value = serde_json::json!({
            "id": "ZSUpar6LGMiFodAP1JvsmQQ",
            "object": "chat.completion",
            "created": 1781081445,
            "model": "google/gemini-2.5-pro",
            "system_fingerprint": "",
            "choices": [
                {
                    "index": 0,
                    "finish_reason": "malformed_function_call",
                    "message": {
                        "role": "assistant",
                        "refusal": MALFORMED_NON_SEMANTIC_PATCH_REFUSAL
                    }
                }
            ],
            "usage": {
                "prompt_tokens": 8665,
                "completion_tokens": 113,
                "total_tokens": 18503
            }
        });

        // The finish reason must deserialize to the dedicated variant.
        let response: OpenAiResponse = serde_json::from_value(value.clone())
            .expect("captured malformed response deserializes");
        assert_eq!(
            response.choices[0].finish_reason,
            Some(FinishReason::MalformedFunctionCall)
        );

        // The driver parse path must surface it as a MalformedFunctionCall finish
        // error, preserving the multi-line diff content in the message.
        let body = serde_json::to_string(&value).expect("serialize captured response body");
        let err =
            parse_chat_outcome(&body).expect_err("malformed multi-line patch call should fail");
        match err {
            LlmError::FinishError {
                finish_reason, msg, ..
            } => {
                assert_eq!(finish_reason, FinishReason::MalformedFunctionCall);
                assert!(
                    msg.contains("default_api.non_semantic_patch"),
                    "expected captured python call in refusal msg"
                );
                assert!(
                    msg.contains("--- a/crates/printer/src/util.rs")
                        && msg.contains("replace_with_captures_at_kludge"),
                    "expected the multi-line unified diff body to survive parsing"
                );
            }
            other => panic!("expected malformed-function-call finish error, got {other:?}"),
        }
    }

    #[test]
    fn parse_outcome_choice_error_returns_api_error_with_provider_metadata() {
        let body = r#"{
            "provider": "Groq",
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": "Let me create the fix:"},
                    "error": {
                        "code": 502,
                        "message": "Upstream error from Groq: tool call validation failed: parameters for tool cargo did not match schema: errors: [`/command`: value must be one of \"test\", \"check\"]"
                    }
                }
            ]
        }"#;

        let err = parse_chat_outcome(body).expect_err("choice error should abort parsing");
        match err {
            LlmError::Api {
                status,
                api_code,
                provider_name,
                provider_slug,
                error_source,
                ..
            } => {
                assert_eq!(status, 200);
                assert_eq!(api_code.as_deref(), Some("502"));
                assert_eq!(provider_name.as_deref(), Some("Groq"));
                assert_eq!(provider_slug.as_deref(), Some("groq"));
                assert_eq!(error_source, ApiErrorSource::ChoiceError);
            }
            other => panic!("expected api error, got {other:?}"),
        }
    }

    #[test]
    fn parse_outcome_skips_errored_choice_when_later_choice_is_valid() {
        let body = r#"{
            "provider": "Groq",
            "choices": [
                {
                    "index": 0,
                    "error": {
                        "code": 502,
                        "message": "bad first choice"
                    }
                },
                {
                    "index": 1,
                    "message": {
                        "role": "assistant",
                        "content": "usable second choice"
                    }
                }
            ]
        }"#;

        let parsed = parse_chat_outcome(body).expect("later valid choice should still be accepted");
        match parsed.outcome {
            ChatStepOutcome::Content {
                content: Some(content),
                ..
            } => assert_eq!(content.as_ref(), "usable second choice"),
            other => panic!("expected content outcome, got {other:?}"),
        }
    }

    #[test]
    fn parse_outcome_top_level_error_preserves_provider_metadata() {
        let body = r#"{
            "provider": "Groq",
            "error": {
                "message": "No successful provider responses.",
                "code": 404
            }
        }"#;

        let err = parse_chat_outcome(body).expect_err("top-level error should abort parsing");
        match err {
            LlmError::Api {
                status,
                api_code,
                provider_name,
                provider_slug,
                error_source,
                ..
            } => {
                assert_eq!(status, 200);
                assert_eq!(api_code.as_deref(), Some("404"));
                assert_eq!(provider_name.as_deref(), Some("Groq"));
                assert_eq!(provider_slug.as_deref(), Some("groq"));
                assert_eq!(error_source, ApiErrorSource::TopLevelError);
            }
            other => panic!("expected api error, got {other:?}"),
        }
    }

    #[test]
    fn parse_outcome_top_level_rate_limit_preserves_status_and_api_code_separately() {
        let body = r#"{
            "provider": "Io Net",
            "error": {
                "message": "Provider returned error",
                "code": 429
            }
        }"#;

        let err = parse_chat_outcome(body).expect_err("top-level error should abort parsing");
        match err {
            LlmError::Api {
                status,
                api_code,
                provider_name,
                error_source,
                ..
            } => {
                assert_eq!(status, 200);
                assert_eq!(api_code.as_deref(), Some("429"));
                assert_eq!(provider_name.as_deref(), Some("Io Net"));
                assert_eq!(error_source, ApiErrorSource::TopLevelError);
            }
            other => panic!("expected api error, got {other:?}"),
        }
    }

    #[test]
    fn parse_outcome_top_level_error_accepts_non_object_error_shape() {
        let body = r#"{
            "provider": "Groq",
            "error": "upstream failed"
        }"#;

        let err = parse_chat_outcome(body).expect_err("top-level error should abort parsing");
        match err {
            LlmError::Api {
                status,
                message,
                provider_name,
                error_source,
                ..
            } => {
                assert_eq!(status, 200);
                assert_eq!(message, "Unknown provider error");
                assert_eq!(provider_name.as_deref(), Some("Groq"));
                assert_eq!(error_source, ApiErrorSource::TopLevelError);
            }
            other => panic!("expected api error, got {other:?}"),
        }
    }

    #[test]
    fn parse_retry_after_supports_delta_seconds() {
        let mut headers = HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "2".parse().unwrap());

        assert_eq!(parse_retry_after(&headers), Some(Duration::from_secs(2)));
    }

    /// Config matching the direct-Google retry budget so the schedule tests
    /// exercise the same parameters production uses for Vertex DSQ 429s.
    fn google_like_cfg() -> ChatHttpConfig {
        ChatHttpConfig {
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(8),
            max_total_elapsed: Some(Duration::from_secs(60)),
            ..ChatHttpConfig::default()
        }
    }

    #[test]
    fn backoff_ceiling_is_monotonic_exponential_and_capped() {
        let cfg = google_like_cfg();
        // 500ms * 2^(attempt-1), capped at max_backoff (8s).
        assert_eq!(backoff_ceiling(&cfg, 1), Duration::from_millis(500));
        assert_eq!(backoff_ceiling(&cfg, 2), Duration::from_secs(1));
        assert_eq!(backoff_ceiling(&cfg, 3), Duration::from_secs(2));
        assert_eq!(backoff_ceiling(&cfg, 4), Duration::from_secs(4));
        assert_eq!(backoff_ceiling(&cfg, 5), Duration::from_secs(8));
        // Cap holds and the schedule never decreases.
        assert_eq!(backoff_ceiling(&cfg, 6), Duration::from_secs(8));
        let mut previous = Duration::ZERO;
        for attempt in 1..=8 {
            let ceiling = backoff_ceiling(&cfg, attempt);
            assert!(
                ceiling >= previous,
                "ceiling must be monotonic non-decreasing"
            );
            assert!(
                ceiling <= cfg.max_backoff,
                "ceiling must respect max_backoff"
            );
            previous = ceiling;
        }
    }

    #[test]
    fn backoff_ceiling_does_not_overflow_for_large_attempts() {
        let cfg = google_like_cfg();
        // Saturating shift/mul must not panic and must stay capped.
        assert_eq!(backoff_ceiling(&cfg, u32::MAX), cfg.max_backoff);
    }

    #[test]
    fn full_jitter_stays_within_zero_and_ceiling() {
        let ceiling = Duration::from_secs(8);
        for _ in 0..10_000 {
            let sampled = full_jitter(ceiling);
            assert!(sampled <= ceiling, "jitter must not exceed the ceiling");
        }
        // Zero ceiling yields zero (no panic, no negative).
        assert_eq!(full_jitter(Duration::ZERO), Duration::ZERO);
    }

    #[test]
    fn schedule_jittered_backoff_respects_ceiling_and_remaining_budget() {
        let cfg = google_like_cfg();
        // Early in the request: jittered exponential bounded by the attempt ceiling.
        for _ in 0..1_000 {
            let scheduled = schedule_retry_backoff(&cfg, 4, None, Duration::from_secs(1))
                .expect("budget remains");
            assert!(scheduled <= backoff_ceiling(&cfg, 4));
        }
        // Near the end of the budget the sleep is clamped to what is left.
        let remaining = Duration::from_millis(120);
        let elapsed = cfg.max_total_elapsed.unwrap() - remaining;
        for _ in 0..1_000 {
            let scheduled =
                schedule_retry_backoff(&cfg, 6, None, elapsed).expect("tiny budget remains");
            assert!(
                scheduled <= remaining,
                "must not sleep past the remaining budget"
            );
        }
    }

    #[test]
    fn schedule_stops_retrying_once_budget_is_exhausted() {
        let cfg = google_like_cfg();
        let budget = cfg.max_total_elapsed.unwrap();
        assert!(schedule_retry_backoff(&cfg, 3, None, budget).is_none());
        assert!(schedule_retry_backoff(&cfg, 3, None, budget + Duration::from_secs(5)).is_none());
    }

    #[test]
    fn schedule_honors_retry_after_within_budget() {
        let cfg = google_like_cfg();
        // A server Retry-After larger than max_backoff is honored in full when it
        // still fits in the remaining budget (it is NOT clamped to max_backoff).
        assert_eq!(
            schedule_retry_backoff(
                &cfg,
                1,
                Some(Duration::from_secs(20)),
                Duration::from_secs(1)
            ),
            Some(Duration::from_secs(20))
        );
    }

    #[test]
    fn schedule_drops_retry_after_that_exceeds_remaining_budget() {
        let cfg = google_like_cfg();
        // Only 5s of budget remains but the server asks for 20s: we cannot honor
        // it within budget, so we stop retrying instead of waking too early.
        let elapsed = cfg.max_total_elapsed.unwrap() - Duration::from_secs(5);
        assert!(schedule_retry_backoff(&cfg, 1, Some(Duration::from_secs(20)), elapsed).is_none());
    }

    #[test]
    fn schedule_unbudgeted_caps_retry_after_to_max_backoff() {
        // Legacy (OpenRouter) path: no total budget, Retry-After clamped to
        // max_backoff and exponential backoff never blocked by a budget.
        let cfg = ChatHttpConfig {
            max_backoff: Duration::from_secs(2),
            max_total_elapsed: None,
            ..ChatHttpConfig::default()
        };
        assert_eq!(
            schedule_retry_backoff(
                &cfg,
                2,
                Some(Duration::from_secs(9)),
                Duration::from_secs(120)
            ),
            Some(Duration::from_secs(2))
        );
        // Without a budget, a retry is always scheduled regardless of elapsed.
        let scheduled = schedule_retry_backoff(&cfg, 3, None, Duration::from_secs(600))
            .expect("unbudgeted path always schedules");
        assert!(scheduled <= cfg.max_backoff);
    }

    #[test]
    fn retryable_statuses_are_retried_and_fatal_4xx_fail_fast() {
        let tuning = RetryTuning::default();
        // Retryable: 429 RESOURCE_EXHAUSTED, 503 UNAVAILABLE, transient 5xx/timeout.
        for status in [408, 409, 425, 429, 500, 502, 503, 504] {
            assert!(
                should_retry_status(status, &tuning),
                "status {status} should be retryable"
            );
        }
        // Fatal 4xx must fail fast (never retried).
        for status in [400, 401, 403, 404, 422] {
            assert!(
                !should_retry_status(status, &tuning),
                "status {status} must not be retried"
            );
        }
    }

    #[test]
    fn body_timeout_after_success_status_is_retried() {
        let tuning = RetryTuning::default();
        assert!(should_retry_body_failure(
            200,
            &HttpBodyFailure::Timeout,
            1,
            &tuning
        ));
        assert!(should_retry_body_failure(
            204,
            &HttpBodyFailure::Timeout,
            1,
            &tuning
        ));
        assert!(should_retry_body_failure(
            503,
            &HttpBodyFailure::Timeout,
            1,
            &tuning
        ));
    }

    #[test]
    fn body_timeout_retry_limit_caps_retries() {
        let tuning = RetryTuning {
            body_timeout_retry_limit: Some(1),
            ..RetryTuning::default()
        };

        assert!(should_retry_body_failure(
            200,
            &HttpBodyFailure::Timeout,
            1,
            &tuning
        ));
        assert!(!should_retry_body_failure(
            200,
            &HttpBodyFailure::Timeout,
            2,
            &tuning
        ));
    }

    #[test]
    fn body_decode_failure_is_not_retried() {
        let tuning = RetryTuning::default();
        assert!(!should_retry_body_failure(
            200,
            &HttpBodyFailure::DecodeFailed,
            1,
            &tuning
        ));
        assert!(!should_retry_body_failure(
            503,
            &HttpBodyFailure::DecodeFailed,
            1,
            &tuning
        ));
    }

    #[test]
    fn effective_attempt_timeout_clamps_retries_but_not_first_attempt() {
        let cfg = ChatHttpConfig {
            attempt_timeout: AttemptTimeout::fixed(Duration::from_secs(300)),
            max_total_elapsed: Some(Duration::from_secs(60)),
            ..ChatHttpConfig::default()
        };
        // First attempt always keeps the full attempt_timeout, even when the
        // elapsed time already exceeds the budget (budget < attempt_timeout here).
        // A normal single attempt must never be truncated by the budget.
        assert_eq!(
            effective_attempt_timeout(&cfg, 1, Duration::from_secs(120)),
            Duration::from_secs(300)
        );
        // A retry is clamped to the remaining budget.
        assert_eq!(
            effective_attempt_timeout(&cfg, 2, Duration::from_secs(50)),
            Duration::from_secs(10)
        );
        // With most of the budget left, a retry is still capped at the budget
        // (it can never exceed max_total_elapsed), not the full attempt_timeout.
        assert_eq!(
            effective_attempt_timeout(&cfg, 2, Duration::ZERO),
            Duration::from_secs(60)
        );
        // A retry past the budget saturates to zero rather than underflowing.
        assert_eq!(
            effective_attempt_timeout(&cfg, 3, Duration::from_secs(75)),
            Duration::ZERO
        );
    }

    #[test]
    fn effective_attempt_timeout_never_clamps_unbudgeted_routers() {
        // Legacy/OpenRouter path (no total budget): retries keep the full timeout.
        let cfg = ChatHttpConfig {
            attempt_timeout: AttemptTimeout::fixed(Duration::from_secs(300)),
            max_total_elapsed: None,
            ..ChatHttpConfig::default()
        };
        assert_eq!(
            effective_attempt_timeout(&cfg, 5, Duration::from_secs(10_000)),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn chat_http_default_timeout_uses_llm_timeout_constant() {
        assert_eq!(
            ChatHttpConfig::default().attempt_timeout.for_attempt(1),
            Duration::from_secs(crate::LLM_TIMEOUT_SECS)
        );
        assert_eq!(
            ChatHttpConfig::default().attempt_timeout.for_attempt(2),
            Duration::from_secs(crate::LLM_TIMEOUT_SECS)
        );
        assert_eq!(ChatHttpConfig::default().max_attempts, 1);
    }
}

/// End-to-end coverage for the [`chat_step_with_attempts`] retry loop driven to
/// exhaustion against local mock transports.
///
/// These tests exercise the `schedule_retry_backoff -> None ->
/// ProviderRetryDecision::Exhausted` fall-through across all three structurally
/// distinct failure branches (send / body / status), plus pin the retry-loop
/// count for the budgeted (direct-Google) path versus the legacy one-retry
/// floor.
#[cfg(test)]
mod chat_step_retry_exhaustion_tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use once_cell::sync::Lazy;
    use reqwest::Client;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::{ChatHttpConfig, ChatStepError, chat_step_with_attempts};
    use crate::LlmError;
    use crate::manager::builders::attempt::{ProviderFailurePhase, ProviderRetryDecision};
    use crate::registry::calibration::{
        AttemptTimeout, ProviderTiming, RetryTuning, RouterCalibration as _,
    };
    use crate::router_only::openrouter::{ChatCompFields, OpenRouter, OpenRouterModelId};
    use crate::router_only::{ChatCompRequest, Router};

    // The process-global mock completion URL is shared via `MockRouter`, so chat
    // step tests must run one at a time.
    static CHAT_TEST_LOCK: Lazy<tokio::sync::Mutex<()>> =
        Lazy::new(|| tokio::sync::Mutex::new(()));

    // `Router::completion_url` returns `&'static str`, but a mock transport binds
    // an ephemeral port. We stash a per-test URL here and hand out a leaked
    // `&'static str` (called once per chat step, while holding `CHAT_TEST_LOCK`).
    static MOCK_COMPLETION_URL: Mutex<Option<&'static str>> = Mutex::new(None);

    fn set_mock_completion_url(url: &str) {
        let leaked: &'static str = Box::leak(url.to_string().into_boxed_str());
        *MOCK_COMPLETION_URL.lock().expect("mock url lock") = Some(leaked);
    }

    fn current_mock_completion_url() -> &'static str {
        MOCK_COMPLETION_URL
            .lock()
            .expect("mock url lock")
            .expect("mock completion url must be set before chat_step")
    }

    /// Test-only router that reuses OpenRouter's request types but resolves its
    /// completion URL to the per-test mock transport.
    #[derive(
        Copy,
        Clone,
        Debug,
        Default,
        PartialEq,
        Eq,
        PartialOrd,
        serde::Serialize,
        serde::Deserialize,
    )]
    struct MockRouter;

    impl Router for MockRouter {
        type CompletionFields = ChatCompFields;
        type RouterModelId = OpenRouterModelId;
        const BASE_URL: &str = "http://127.0.0.1";
        const COMPLETION_URL: &str = "http://127.0.0.1/v1/chat/completions";
        const MODELS_URL: &str = "http://127.0.0.1/v1/models";
        const ENDPOINTS_TAIL: &str = "endpoints";
        const API_KEY_NAME: &str = "PLOKE_MOCK_API_KEY";
        const PROVIDERS_URL: &str = "http://127.0.0.1/v1/providers";

        fn resolve_api_key() -> Result<String, LlmError> {
            Ok("test-key".to_string())
        }

        fn completion_url() -> Result<&'static str, LlmError> {
            Ok(current_mock_completion_url())
        }
    }

    fn mock_request() -> ChatCompRequest<MockRouter> {
        MockRouter::default_chat_completion()
            .with_model_str("test/model")
            .expect("valid model id")
    }

    fn cfg_from(timing: &ProviderTiming) -> ChatHttpConfig {
        // Shrink the backoff schedule so loop tests stay fast while preserving
        // the calibrated attempt/budget shape under test.
        let mut cfg = ChatHttpConfig::from(timing);
        cfg.initial_backoff = Duration::from_millis(1);
        cfg.max_backoff = Duration::from_millis(2);
        cfg
    }

    /// Budget of zero deterministically forces `schedule_retry_backoff` to return
    /// `None` on the first retry decision (remaining budget is zero), so the
    /// first failed attempt falls through to `Exhausted` without truncating the
    /// in-flight attempt itself.
    fn budget_zero_cfg(max_attempts: u32, attempt_timeout: Duration) -> ChatHttpConfig {
        ChatHttpConfig {
            attempt_timeout: AttemptTimeout::fixed(attempt_timeout),
            max_attempts,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(2),
            max_total_elapsed: Some(Duration::ZERO),
            retry: RetryTuning::default(),
            ..ChatHttpConfig::default()
        }
    }

    /// Bind an ephemeral port, then drop the listener so connecting to it yields
    /// a fast connection-refused (a retryable send failure).
    fn refused_completion_url() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        format!("http://{addr}/v1/chat/completions")
    }

    /// Spawn a raw TCP server that returns response headers (promising a body via
    /// `Content-Length`) and then stalls without ever writing the body. The
    /// client receives headers (so `send()` resolves) but the per-attempt
    /// `.timeout()` fires while reading the body, producing a retryable
    /// `HttpBodyFailure::Timeout` in the body branch.
    async fn spawn_stalling_body_server() -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind body server");
        let addr = listener.local_addr().expect("body server addr");
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    // Best-effort drain of the request so the client finishes
                    // sending before we reply with headers.
                    let _ = socket.read(&mut buf).await;
                    let _ = socket
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                              Content-Length: 1000\r\n\r\n",
                        )
                        .await;
                    let _ = socket.flush().await;
                    // Never deliver the promised body; hold the connection open so
                    // the client's body read times out instead of seeing EOF.
                    tokio::time::sleep(Duration::from_secs(30)).await;
                });
            }
        });
        (format!("http://{addr}/v1/chat/completions"), handle)
    }

    /// Spawn a raw TCP server whose first connection returns a fast, retryable
    /// 503 (so the loop advances to a retry) and whose subsequent connections
    /// send headers and then stall (so a retry's body read times out). The 503
    /// uses `Connection: close` so the client dials a fresh connection for the
    /// retry, keeping the per-connection branching reliable.
    async fn spawn_fast_then_stalling_server() -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fast-then-stalling server");
        let addr = listener.local_addr().expect("server addr");
        let handle = tokio::spawn(async move {
            let mut connection_count: u32 = 0;
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let is_first = connection_count == 0;
                connection_count += 1;
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    let _ = socket.read(&mut buf).await;
                    if is_first {
                        let body = b"upstream unavailable";
                        let header = format!(
                            "HTTP/1.1 503 Service Unavailable\r\nContent-Type: text/plain\r\n\
                             Connection: close\r\nContent-Length: {}\r\n\r\n",
                            body.len()
                        );
                        let _ = socket.write_all(header.as_bytes()).await;
                        let _ = socket.write_all(body).await;
                        let _ = socket.flush().await;
                    } else {
                        let _ = socket
                            .write_all(
                                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                                  Content-Length: 1000\r\n\r\n",
                            )
                            .await;
                        let _ = socket.flush().await;
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                });
            }
        });
        (format!("http://{addr}/v1/chat/completions"), handle)
    }

    #[tokio::test]
    async fn retry_attempt_timeout_is_clamped_to_remaining_budget() {
        let _guard = CHAT_TEST_LOCK.lock().await;
        let (url, server) = spawn_fast_then_stalling_server().await;
        set_mock_completion_url(&url);

        // The full per-attempt timeout (30s) dwarfs the total budget (400ms). The
        // first attempt fails fast with a retryable 503, then the retry stalls on
        // the body. If the retry were NOT clamped it would block for the full 30s;
        // the clamp bounds it to the remaining budget instead. `max_attempts == 2`
        // stops the loop deterministically after exactly one retry.
        let cfg = ChatHttpConfig {
            attempt_timeout: AttemptTimeout::fixed(Duration::from_secs(30)),
            max_attempts: 2,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(2),
            max_total_elapsed: Some(Duration::from_millis(400)),
            retry: RetryTuning::default(),
            ..ChatHttpConfig::default()
        };
        let req = mock_request();

        let started = std::time::Instant::now();
        let err: ChatStepError = chat_step_with_attempts(&Client::new(), &req, &cfg)
            .await
            .expect_err("stalled retry must surface as an error");
        let total = started.elapsed();

        assert_eq!(err.provider_attempts.len(), 2);
        let first = &err.provider_attempts[0];
        let retry = &err.provider_attempts[1];

        // First attempt: fast retryable 503 that scheduled the retry.
        assert_eq!(first.failure_phase, Some(ProviderFailurePhase::Status));
        assert_eq!(first.status, Some(503));
        assert_eq!(first.retry_decision, ProviderRetryDecision::Scheduled);

        // Retry attempt: stalled body read that timed out and exhausted the loop.
        assert_eq!(retry.failure_phase, Some(ProviderFailurePhase::Body));
        assert_eq!(
            retry.body_failure,
            Some(crate::error::HttpBodyFailure::Timeout)
        );
        assert_eq!(retry.retry_decision, ProviderRetryDecision::Exhausted);

        // The retry timed out at its clamped budget (~400ms), far short of the
        // full 30s attempt_timeout it would have used if it were not clamped.
        let retry_duration = retry.failed.expect("retry recorded a failure time");
        assert!(
            retry_duration < Duration::from_secs(5),
            "retry should be clamped to the remaining budget, took {retry_duration:?}"
        );
        assert!(
            total < Duration::from_secs(5),
            "the whole chat step should finish well under the full attempt_timeout, took {total:?}"
        );

        server.abort();
    }

    #[tokio::test]
    async fn send_failure_exhausts_via_budget_none_fall_through() {
        let _guard = CHAT_TEST_LOCK.lock().await;
        set_mock_completion_url(&refused_completion_url());

        // max_attempts is generous; the zero budget (not the attempt limit) is
        // what stops the loop, exercising the `schedule_retry_backoff -> None`
        // fall-through in the send branch (~session.rs:389).
        let cfg = budget_zero_cfg(5, Duration::from_secs(5));
        let req = mock_request();

        let err: ChatStepError = chat_step_with_attempts(&Client::new(), &req, &cfg)
            .await
            .expect_err("connection refused must surface as an error");

        assert!(
            matches!(err.source, LlmError::Http(_)),
            "send failure should surface as LlmError::Http, got {:?}",
            err.source
        );
        let last = err
            .provider_attempts
            .last()
            .expect("at least one provider attempt recorded");
        assert_eq!(last.failure_phase, Some(ProviderFailurePhase::Send));
        assert_eq!(last.retry_decision, ProviderRetryDecision::Exhausted);
        // Zero budget means we never schedule a retry, so exhaustion is reached
        // before the attempt limit is consumed.
        assert!(
            err.provider_attempts.len() < cfg.max_attempts as usize,
            "budget (not attempt limit) should stop the loop; attempts: {}",
            err.provider_attempts.len()
        );
        assert!(
            err.provider_attempts
                .iter()
                .all(|attempt| attempt.retry_decision != ProviderRetryDecision::Scheduled),
            "no retry should be scheduled under a zero budget"
        );
    }

    #[tokio::test]
    async fn body_failure_exhausts_via_budget_none_fall_through() {
        let _guard = CHAT_TEST_LOCK.lock().await;
        let (url, server) = spawn_stalling_body_server().await;
        set_mock_completion_url(&url);

        // Short per-attempt timeout so the stalled body read times out quickly.
        // Zero budget drives the body branch's nested `if let ... else`
        // fall-through (~session.rs:488-547) to `Exhausted`.
        let cfg = budget_zero_cfg(5, Duration::from_millis(150));
        let req = mock_request();

        let err: ChatStepError = chat_step_with_attempts(&Client::new(), &req, &cfg)
            .await
            .expect_err("stalled body read must surface as an error");

        assert!(
            matches!(err.source, LlmError::Http(_)),
            "body failure should surface as LlmError::Http, got {:?}",
            err.source
        );
        let last = err
            .provider_attempts
            .last()
            .expect("at least one provider attempt recorded");
        assert_eq!(last.failure_phase, Some(ProviderFailurePhase::Body));
        assert_eq!(last.retry_decision, ProviderRetryDecision::Exhausted);
        assert!(
            err.provider_attempts.len() < cfg.max_attempts as usize,
            "budget (not attempt limit) should stop the loop; attempts: {}",
            err.provider_attempts.len()
        );

        server.abort();
    }

    #[tokio::test]
    async fn status_failure_exhausts_when_retry_after_exceeds_budget() {
        use httpmock::prelude::*;

        let _guard = CHAT_TEST_LOCK.lock().await;
        let server = MockServer::start();
        // 503 UNAVAILABLE with a Retry-After far larger than the remaining budget:
        // `schedule_retry_backoff` cannot honor it within budget, so it returns
        // `None` and the status branch (~session.rs:582) falls through to
        // `Exhausted` on the first failure.
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/chat/completions");
            then.status(503)
                .header("Retry-After", "3600")
                .body("upstream unavailable");
        });
        set_mock_completion_url(&server.url("/v1/chat/completions"));

        let cfg = ChatHttpConfig {
            attempt_timeout: AttemptTimeout::fixed(Duration::from_secs(5)),
            max_attempts: 6,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(2),
            max_total_elapsed: Some(Duration::from_secs(2)),
            retry: RetryTuning::default(),
            ..ChatHttpConfig::default()
        };
        let req = mock_request();

        let err: ChatStepError = chat_step_with_attempts(&Client::new(), &req, &cfg)
            .await
            .expect_err("503 with unaffordable Retry-After must surface an error");

        match &err.source {
            LlmError::Api { status, .. } => assert_eq!(*status, 503),
            other => panic!("status failure should surface as LlmError::Api, got {other:?}"),
        }
        let last = err
            .provider_attempts
            .last()
            .expect("at least one provider attempt recorded");
        assert_eq!(last.failure_phase, Some(ProviderFailurePhase::Status));
        assert_eq!(last.retry_decision, ProviderRetryDecision::Exhausted);
        // The unaffordable Retry-After stops scheduling on the very first
        // failure, so exactly one attempt is made.
        assert_eq!(err.provider_attempts.len(), 1);
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn rate_limit_loops_up_to_google_max_attempts_then_exhausts() {
        use httpmock::prelude::*;

        let _guard = CHAT_TEST_LOCK.lock().await;
        let server = MockServer::start();
        // 429 with an immediately-affordable Retry-After: the loop should retry
        // up to the calibrated Google max-attempts and then stop.
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/chat/completions");
            then.status(429).header("Retry-After", "0").body("rate limited");
        });
        set_mock_completion_url(&server.url("/v1/chat/completions"));

        let google_timing = crate::router_only::google::Google::default_provider_timing();
        let google_max = google_timing.max_attempts;
        assert!(google_max >= 2, "google calibration should allow retries");
        let cfg = cfg_from(&google_timing);
        let req = mock_request();

        let err: ChatStepError = chat_step_with_attempts(&Client::new(), &req, &cfg)
            .await
            .expect_err("repeated 429 must eventually exhaust");

        match &err.source {
            LlmError::Api { status, .. } => assert_eq!(*status, 429),
            other => panic!("rate limit should surface as LlmError::Api, got {other:?}"),
        }
        assert_eq!(
            err.provider_attempts.len(),
            google_max as usize,
            "loop must run exactly the calibrated Google max-attempts"
        );
        mock.assert_hits(google_max as usize);
        let (last, earlier) = err
            .provider_attempts
            .split_last()
            .expect("at least one attempt");
        assert_eq!(last.retry_decision, ProviderRetryDecision::Exhausted);
        assert_eq!(last.failure_phase, Some(ProviderFailurePhase::Status));
        assert!(
            earlier
                .iter()
                .all(|attempt| attempt.retry_decision == ProviderRetryDecision::Scheduled),
            "every attempt before the last should have scheduled a retry"
        );
    }

    #[tokio::test]
    async fn legacy_path_honors_single_retry_floor() {
        use httpmock::prelude::*;

        let _guard = CHAT_TEST_LOCK.lock().await;
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/chat/completions");
            then.status(429).header("Retry-After", "0").body("rate limited");
        });
        set_mock_completion_url(&server.url("/v1/chat/completions"));

        // OpenRouter calibrates a single attempt; ploke-tui's session layer
        // floors it to one retry via `MIN_CHAT_HTTP_ATTEMPTS` (= 2) before
        // building the ChatHttpConfig. Mirror that floor here so the legacy
        // unbudgeted path performs exactly one retry (two attempts) and stops.
        const MIN_CHAT_HTTP_ATTEMPTS: u32 = 2;
        let mut timing = OpenRouter::default_provider_timing();
        assert_eq!(timing.max_attempts, 1, "OpenRouter calibrates a single attempt");
        assert_eq!(
            timing.max_total_elapsed, None,
            "legacy OpenRouter path stays unbudgeted"
        );
        timing.max_attempts = timing.max_attempts.max(MIN_CHAT_HTTP_ATTEMPTS);
        let cfg = cfg_from(&timing);
        let req = mock_request();

        let err: ChatStepError = chat_step_with_attempts(&Client::new(), &req, &cfg)
            .await
            .expect_err("repeated 429 must exhaust the one-retry floor");

        match &err.source {
            LlmError::Api { status, .. } => assert_eq!(*status, 429),
            other => panic!("rate limit should surface as LlmError::Api, got {other:?}"),
        }
        assert_eq!(
            err.provider_attempts.len(),
            MIN_CHAT_HTTP_ATTEMPTS as usize,
            "legacy floor should perform exactly one retry"
        );
        mock.assert_hits(MIN_CHAT_HTTP_ATTEMPTS as usize);
        let last = err.provider_attempts.last().expect("at least one attempt");
        assert_eq!(last.retry_decision, ProviderRetryDecision::Exhausted);
    }
}
