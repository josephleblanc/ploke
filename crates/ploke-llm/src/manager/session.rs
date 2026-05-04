#![allow(
    dead_code,
    unused_variables,
    reason = "evolving api surface, may be useful, written 2025-12-15"
)]

use std::env;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use ploke_core::ArcStr;
use reqwest::header::HeaderMap;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatHttpConfig {
    referer: &'static str,
    title: &'static str,
    pub attempt_timeout: AttemptTimeout,
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
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

fn emit_chat_http_stderr_line(payload: serde_json::Value) {
    if chat_http_stderr_enabled()
        && let Ok(line) = serde_json::to_string(&payload)
    {
        eprintln!("{line}");
    }
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
    let url = R::COMPLETION_URL;
    let request_id = NEXT_CHAT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let mut provider_attempts = Vec::new();
    let api_key = R::resolve_api_key().map_err(|e| {
        ChatStepError::new(LlmError::Http(HttpFailure::send(
            None,
            None,
            format!("missing api key: {e}"),
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
        let attempt_timeout = cfg.attempt_timeout.for_attempt(attempt);
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
                if should_retry && attempt < max_attempts {
                    let backoff = compute_retry_backoff(cfg, attempt, None);
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
                    let backoff = compute_retry_backoff(cfg, attempt, retry_after);
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
            if should_retry && attempt < max_attempts {
                let backoff = compute_retry_backoff(cfg, attempt, retry_after);
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
    emit_chat_http_stderr_line(serde_json::json!({
        "event": "chat_http_response_error_status",
        "request_id": request_id,
        "attempt": attempt,
        "max_attempts": max_attempts,
        "url": url,
        "status": status,
        "retry_after_ms": retry_after.map(|value| value.as_millis() as u64),
        "elapsed_ms": elapsed.as_millis()
    }));
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
    emit_chat_http_stderr_line(serde_json::json!({
        "event": "chat_http_retry_scheduled",
        "request_id": request_id,
        "attempt": attempt,
        "max_attempts": max_attempts,
        "phase": phase,
        "url": url,
        "status": status,
        "backoff_ms": backoff.as_millis(),
        "elapsed_ms": elapsed.as_millis()
    }));
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
    emit_chat_http_stderr_line(serde_json::json!({
        "event": "chat_http_request_error",
        "request_id": event.request_id,
        "attempt": event.attempt,
        "max_attempts": event.max_attempts,
        "phase": event.phase,
        "url": event.url,
        "status": event.status,
        "failure": event.failure,
        "receive_phase": event.receive_phase,
        "body_failure": event.body_failure,
        "elapsed_ms": event.elapsed.as_millis(),
        "is_timeout": event.is_timeout,
        "raw_error": event.raw_error
    }));
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
    emit_chat_http_stderr_line(serde_json::json!({
        "event": "chat_http_retry_suppressed",
        "request_id": request_id,
        "attempt": attempt,
        "max_attempts": max_attempts,
        "phase": phase,
        "url": url,
        "status": status,
        "body_failure": body_failure,
        "reason": "classified_non_retryable",
        "elapsed_ms": elapsed.as_millis()
    }));
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

fn compute_retry_backoff(
    cfg: &ChatHttpConfig,
    attempt: u32,
    retry_after: Option<Duration>,
) -> Duration {
    if let Some(retry_after) = retry_after {
        return retry_after.min(cfg.max_backoff);
    }

    let exponent = attempt.saturating_sub(1);
    let multiplier = 1u32.checked_shl(exponent.min(16)).unwrap_or(u32::MAX);
    let backoff = cfg.initial_backoff.saturating_mul(multiplier);
    backoff.min(cfg.max_backoff)
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
    use serde_json::Value;
    let mut builder = ChatStepDataBuilder::new();
    let mut raw_provider_slug: Option<ArcStr> = None;
    let mut first_choice_error: Option<LlmError> = None;

    // Parse once as JSON so we can cheaply detect embedded errors without double-deserializing.
    // If this fails, we still attempt typed parsing below to produce a more specific error.
    if let Ok(v) = serde_json::from_str::<Value>(body_text) {
        let raw_provider_name = extract_provider_name(&v);
        raw_provider_slug = raw_provider_name
            .as_ref()
            .and_then(ProviderName::to_slug)
            .map(|slug| ArcStr::from(slug.as_str()));

        if let Some(err) = v.get("error") {
            return Err(api_error_from_embedded_error(
                err,
                raw_provider_name.as_ref(),
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

fn extract_provider_name(value: &serde_json::Value) -> Option<ProviderName> {
    value
        .get("provider")
        .and_then(serde_json::Value::as_str)
        .map(ProviderName::new)
}

fn extract_provider_name_from_body(body_text: &str) -> Option<ProviderName> {
    serde_json::from_str::<serde_json::Value>(body_text)
        .ok()
        .and_then(|value| extract_provider_name(&value))
}

fn extract_provider_slug_from_body(body_text: &str) -> Option<ArcStr> {
    extract_provider_name_from_body(body_text)
        .and_then(|name| name.to_slug())
        .map(|slug| ArcStr::from(slug.as_str()))
}

fn extract_api_code(value: &serde_json::Value) -> Option<ArcStr> {
    match value.get("code") {
        Some(serde_json::Value::String(s)) => Some(ArcStr::from(s.as_str())),
        Some(serde_json::Value::Number(n)) => Some(ArcStr::from(n.to_string())),
        _ => None,
    }
}

fn extract_api_code_from_body(body_text: &str) -> Option<ArcStr> {
    serde_json::from_str::<serde_json::Value>(body_text)
        .ok()
        .and_then(|value| value.get("error").and_then(extract_api_code))
}

fn api_error_from_embedded_error(
    err: &serde_json::Value,
    provider_name: Option<&ProviderName>,
    provider_slug: Option<ArcStr>,
    body_text: &str,
    error_source: ApiErrorSource,
) -> LlmError {
    let msg = err
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("Unknown provider error");

    let api_code = extract_api_code(err);
    // Provider "code" is often not an HTTP status; it may be a string like "invalid_api_key".
    // Prefer an explicit `status` field if present, otherwise mark as 200 (embedded error).
    let status = err.get("status").and_then(|s| s.as_u64()).unwrap_or(200) as u16;

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
    match serde_json::from_str::<serde_json::Value>(body_text) {
        Ok(v) => {
            if let Some(err) = v.get("error") {
                let provider_name = extract_provider_name(&v);
                let provider_slug = provider_name
                    .as_ref()
                    .and_then(ProviderName::to_slug)
                    .map(|slug| ArcStr::from(slug.as_str()));
                Err(api_error_from_embedded_error(
                    err,
                    provider_name.as_ref(),
                    provider_slug,
                    body_text,
                    ApiErrorSource::TopLevelError,
                ))
            } else {
                Err(LlmError::Deserialization {
                    message: "No choices".into(),
                    body_snippet: Some(truncate_for_error(body_text, 512)),
                })
            }
        }
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
    fn parse_retry_after_supports_delta_seconds() {
        let mut headers = HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "2".parse().unwrap());

        assert_eq!(parse_retry_after(&headers), Some(Duration::from_secs(2)));
    }

    #[test]
    fn compute_retry_backoff_caps_retry_after() {
        let cfg = ChatHttpConfig {
            max_backoff: Duration::from_secs(2),
            ..ChatHttpConfig::default()
        };

        assert_eq!(
            compute_retry_backoff(&cfg, 2, Some(Duration::from_secs(9))),
            Duration::from_secs(2)
        );
        assert_eq!(compute_retry_backoff(&cfg, 3, None), Duration::from_secs(1));
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
