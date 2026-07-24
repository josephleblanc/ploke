use std::{
    env,
    marker::PhantomData,
    sync::OnceLock,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::Router;
use crate::error::{HttpBodyFailure, HttpSendFailure};

mod private {
    pub trait Sealed {}
}

pub trait StreamingMarker: private::Sealed {}

#[allow(
    dead_code,
    reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
)]
pub struct Streaming<R: Router>(PhantomData<R>);
impl<R: Router> private::Sealed for Streaming<R> {}
impl<R: Router> StreamingMarker for Streaming<R> {}
pub struct NonStreaming<R: Router>(PhantomData<R>);
impl<R: Router> private::Sealed for NonStreaming<R> {}
impl<R: Router> StreamingMarker for NonStreaming<R> {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailurePhase {
    Send,
    Body,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAttemptOutcome {
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderResponseOutcome {
    #[default]
    NotParsed,
    Parsed,
    ProviderError,
    InvalidResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRetryDecision {
    None,
    Scheduled,
    Suppressed,
    Exhausted,
    NotRetryable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAttempt {
    pub request_id: u64,
    pub attempt: u32,
    pub max_attempts: u32,
    pub started_at: Duration,
    pub request_sent: Option<Duration>,
    pub headers_received: Option<Duration>,
    pub output_started: Option<Duration>,
    pub output_progress: Option<Duration>,
    pub output_completed: Option<Duration>,
    pub failed: Option<Duration>,
    pub status: Option<u16>,
    pub response_bytes: Option<usize>,
    #[serde(alias = "transport_outcome")]
    pub outcome: ProviderAttemptOutcome,
    pub failure_phase: Option<ProviderFailurePhase>,
    pub send_failure: Option<HttpSendFailure>,
    pub body_failure: Option<HttpBodyFailure>,
    #[serde(default)]
    pub response_outcome: ProviderResponseOutcome,
    pub retry_decision: ProviderRetryDecision,
    pub retry_after: Option<Duration>,
    pub backoff: Option<Duration>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAttemptTimeline {
    pub request_id: u64,
    pub attempt: u32,
    pub max_attempts: u32,
    pub started_at_ms: u64,
    pub request_sent_ms: Option<u64>,
    pub headers_received_ms: Option<u64>,
    pub output_started_ms: Option<u64>,
    pub output_progress_ms: Option<u64>,
    pub output_completed_ms: Option<u64>,
    pub failed_ms: Option<u64>,
    pub status: Option<u16>,
    pub response_bytes: Option<usize>,
    #[serde(rename = "transport_outcome", alias = "outcome")]
    pub outcome: ProviderAttemptOutcome,
    pub failure_phase: Option<ProviderFailurePhase>,
    #[serde(default, with = "send_failure_serde")]
    pub send_failure: Option<HttpSendFailure>,
    #[serde(default, with = "body_failure_serde")]
    pub body_failure: Option<HttpBodyFailure>,
    #[serde(default)]
    pub response_outcome: ProviderResponseOutcome,
    pub retry_decision: ProviderRetryDecision,
    pub retry_after_ms: Option<u64>,
    pub backoff_ms: Option<u64>,
    pub error: Option<String>,
}

impl ProviderAttemptTimeline {
    #[must_use]
    pub fn from_attempt(attempt: &ProviderAttempt) -> Self {
        Self {
            request_id: attempt.request_id,
            attempt: attempt.attempt,
            max_attempts: attempt.max_attempts,
            started_at_ms: duration_ms(attempt.started_at),
            request_sent_ms: attempt.request_sent.map(duration_ms),
            headers_received_ms: attempt.headers_received.map(duration_ms),
            output_started_ms: attempt.output_started.map(duration_ms),
            output_progress_ms: attempt.output_progress.map(duration_ms),
            output_completed_ms: attempt.output_completed.map(duration_ms),
            failed_ms: attempt.failed.map(duration_ms),
            status: attempt.status,
            response_bytes: attempt.response_bytes,
            outcome: attempt.outcome,
            failure_phase: attempt.failure_phase,
            send_failure: attempt.send_failure.clone(),
            body_failure: attempt.body_failure.clone(),
            response_outcome: attempt.response_outcome,
            retry_decision: attempt.retry_decision,
            retry_after_ms: attempt.retry_after.map(duration_ms),
            backoff_ms: attempt.backoff.map(duration_ms),
            error: attempt.error.clone(),
        }
    }

    #[must_use]
    pub fn into_attempt(self) -> ProviderAttempt {
        ProviderAttempt {
            request_id: self.request_id,
            attempt: self.attempt,
            max_attempts: self.max_attempts,
            started_at: Duration::from_millis(self.started_at_ms),
            request_sent: self.request_sent_ms.map(Duration::from_millis),
            headers_received: self.headers_received_ms.map(Duration::from_millis),
            output_started: self.output_started_ms.map(Duration::from_millis),
            output_progress: self.output_progress_ms.map(Duration::from_millis),
            output_completed: self.output_completed_ms.map(Duration::from_millis),
            failed: self.failed_ms.map(Duration::from_millis),
            status: self.status,
            response_bytes: self.response_bytes,
            outcome: self.outcome,
            failure_phase: self.failure_phase,
            send_failure: self.send_failure,
            body_failure: self.body_failure,
            response_outcome: self.response_outcome,
            retry_decision: self.retry_decision,
            retry_after: self.retry_after_ms.map(Duration::from_millis),
            backoff: self.backoff_ms.map(Duration::from_millis),
            error: self.error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptBuilder<R: StreamingMarker> {
    started: Duration,
    started_instant: Instant,
    request_sent: Option<Duration>,
    headers_received: Option<Duration>,
    output_started: Option<Duration>,
    output_progress: Option<Duration>,
    output_completed: Option<Duration>,
    failed: Option<Duration>,
    status: Option<u16>,
    response_bytes: Option<usize>,
    failure_phase: Option<ProviderFailurePhase>,
    send_failure: Option<HttpSendFailure>,
    body_failure: Option<HttpBodyFailure>,
    response_outcome: ProviderResponseOutcome,
    retry_decision: ProviderRetryDecision,
    retry_after: Option<Duration>,
    backoff: Option<Duration>,
    error: Option<String>,
    _router: PhantomData<R>,
}

impl<R: StreamingMarker> AttemptBuilder<R> {
    #[must_use]
    pub fn new(started: Duration) -> Self {
        Self {
            started,
            started_instant: Instant::now(),
            request_sent: None,
            headers_received: None,
            output_started: None,
            output_progress: None,
            output_completed: None,
            failed: None,
            status: None,
            response_bytes: None,
            failure_phase: None,
            send_failure: None,
            body_failure: None,
            response_outcome: ProviderResponseOutcome::NotParsed,
            retry_decision: ProviderRetryDecision::None,
            retry_after: None,
            backoff: None,
            error: None,
            _router: PhantomData,
        }
    }

    #[must_use]
    pub fn from_origin(origin: Instant) -> Self {
        Self::new(origin.elapsed())
    }

    #[must_use]
    pub fn started(mut self, started: Duration) -> Self {
        self.started = started;
        self
    }

    fn with_request_sent(mut self, elapsed: Duration) -> Self {
        self.request_sent = Some(elapsed);
        self
    }

    #[must_use]
    pub fn request_sent(self) -> Self {
        let elapsed = self.current_offset();
        self.with_request_sent(elapsed)
    }

    fn with_headers_received(mut self, elapsed: Duration) -> Self {
        self.headers_received = Some(elapsed);
        self
    }

    #[must_use]
    pub fn headers_received(self) -> Self {
        let elapsed = self.current_offset();
        self.with_headers_received(elapsed)
    }

    fn with_failed(mut self, elapsed: Duration) -> Self {
        self.failed = Some(elapsed);
        self
    }

    #[must_use]
    pub fn failed(self) -> Self {
        let elapsed = self.current_offset();
        self.with_failed(elapsed)
    }

    #[must_use]
    pub fn status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }

    #[must_use]
    pub fn response_bytes(mut self, response_bytes: usize) -> Self {
        self.response_bytes = Some(response_bytes);
        self
    }

    #[must_use]
    pub fn failure_phase(mut self, phase: ProviderFailurePhase) -> Self {
        self.failure_phase = Some(phase);
        self
    }

    #[must_use]
    pub fn send_failure(mut self, failure: HttpSendFailure) -> Self {
        self.send_failure = Some(failure);
        self
    }

    #[must_use]
    pub fn body_failure(mut self, failure: HttpBodyFailure) -> Self {
        self.body_failure = Some(failure);
        self
    }

    #[must_use]
    pub fn response_outcome(mut self, outcome: ProviderResponseOutcome) -> Self {
        self.response_outcome = outcome;
        self
    }

    #[must_use]
    pub fn retry_decision(mut self, decision: ProviderRetryDecision) -> Self {
        self.retry_decision = decision;
        self
    }

    #[must_use]
    pub fn retry_after(mut self, retry_after: Duration) -> Self {
        self.retry_after = Some(retry_after);
        self
    }

    #[must_use]
    pub fn backoff(mut self, backoff: Duration) -> Self {
        self.backoff = Some(backoff);
        self
    }

    #[must_use]
    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    pub fn started_at(&self) -> Duration {
        self.started
    }

    pub fn request_sent_at(&self) -> Option<Duration> {
        self.request_sent
    }

    pub fn headers_received_at(&self) -> Option<Duration> {
        self.headers_received
    }

    pub fn output_started_at(&self) -> Option<Duration> {
        self.output_started
    }

    pub fn output_progress_at(&self) -> Option<Duration> {
        self.output_progress
    }

    pub fn output_completed_at(&self) -> Option<Duration> {
        self.output_completed
    }

    pub fn current_offset(&self) -> Duration {
        self.started.saturating_add(self.started_instant.elapsed())
    }

    pub fn current_elapsed(&self) -> Duration {
        self.started_instant.elapsed()
    }

    pub fn elapsed_since_started(&self, elapsed: Duration) -> Duration {
        elapsed.saturating_sub(self.started)
    }

    pub fn is_terminal(&self) -> bool {
        self.output_completed.is_some() || self.failed.is_some()
    }

    pub fn completed(&self) -> bool {
        self.output_completed.is_some()
    }

    pub fn failed_at(&self) -> Option<Duration> {
        self.failed
    }

    pub fn failed_elapsed(&self) -> Option<Duration> {
        self.failed
            .map(|elapsed| self.elapsed_since_started(elapsed))
    }

    pub fn headers_received_elapsed(&self) -> Option<Duration> {
        self.headers_received
            .map(|elapsed| self.elapsed_since_started(elapsed))
    }

    pub fn output_completed_elapsed(&self) -> Option<Duration> {
        self.output_completed
            .map(|elapsed| self.elapsed_since_started(elapsed))
    }

    #[must_use]
    pub fn finish(self, request_id: u64, attempt: u32, max_attempts: u32) -> ProviderAttempt {
        ProviderAttempt {
            request_id,
            attempt,
            max_attempts,
            started_at: self.started,
            request_sent: self
                .request_sent
                .map(|elapsed| self.elapsed_since_started(elapsed)),
            headers_received: self
                .headers_received
                .map(|elapsed| self.elapsed_since_started(elapsed)),
            output_started: self
                .output_started
                .map(|elapsed| self.elapsed_since_started(elapsed)),
            output_progress: self
                .output_progress
                .map(|elapsed| self.elapsed_since_started(elapsed)),
            output_completed: self
                .output_completed
                .map(|elapsed| self.elapsed_since_started(elapsed)),
            failed: self
                .failed
                .map(|elapsed| self.elapsed_since_started(elapsed)),
            status: self.status,
            response_bytes: self.response_bytes,
            outcome: if self.failed.is_some() || self.failure_phase.is_some() {
                ProviderAttemptOutcome::Failed
            } else {
                ProviderAttemptOutcome::Completed
            },
            failure_phase: self.failure_phase,
            send_failure: self.send_failure,
            body_failure: self.body_failure,
            response_outcome: self.response_outcome,
            retry_decision: self.retry_decision,
            retry_after: self.retry_after,
            backoff: self.backoff,
            error: self.error,
        }
    }
}

pub(crate) fn trace_provider_attempt(attempt: &ProviderAttempt) {
    let timeline = ProviderAttemptTimeline::from_attempt(attempt);
    emit_attempt_stderr(&timeline);
    let elapsed_ms = attempt
        .failed
        .or(attempt.output_completed)
        .or(attempt.headers_received)
        .map(duration_ms);
    tracing::info!(
        target: "chat_http",
        event = "provider_attempt",
        request_id = timeline.request_id,
        attempt = timeline.attempt,
        max_attempts = timeline.max_attempts,
        started_at_ms = timeline.started_at_ms,
        request_sent_ms = timeline.request_sent_ms,
        headers_received_ms = timeline.headers_received_ms,
        output_started_ms = timeline.output_started_ms,
        output_progress_ms = timeline.output_progress_ms,
        output_completed_ms = timeline.output_completed_ms,
        failed_ms = timeline.failed_ms,
        status = timeline.status,
        response_bytes = timeline.response_bytes,
        transport_outcome = timeline.outcome.as_str(),
        failure_phase = timeline.failure_phase.map(ProviderFailurePhase::as_str),
        send_failure = timeline.send_failure.as_ref().map(HttpSendFailure::as_str),
        body_failure = timeline.body_failure.as_ref().map(HttpBodyFailure::as_str),
        response_outcome = timeline.response_outcome.as_str(),
        retry_decision = timeline.retry_decision.as_str(),
        retry_after_ms = timeline.retry_after_ms,
        backoff_ms = timeline.backoff_ms,
        error = timeline.error.as_deref(),
        elapsed_ms,
    );
}

static CHAT_HTTP_STDERR: OnceLock<bool> = OnceLock::new();

#[derive(Serialize)]
struct ProviderAttemptObservation<'a> {
    event: &'static str,
    #[serde(flatten)]
    attempt: &'a ProviderAttemptTimeline,
}

fn emit_attempt_stderr(attempt: &ProviderAttemptTimeline) {
    if chat_http_stderr_enabled()
        && let Ok(line) = serde_json::to_string(&ProviderAttemptObservation {
            event: "provider_attempt",
            attempt,
        })
    {
        eprintln!("{line}");
    }
}

fn chat_http_stderr_enabled() -> bool {
    *CHAT_HTTP_STDERR.get_or_init(|| {
        env::var("PLOKE_PROTOCOL_DEBUG").is_ok_and(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty() && normalized != "0" && normalized != "false"
        })
    })
}

impl ProviderFailurePhase {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Send => "send",
            Self::Body => "body",
            Self::Status => "status",
        }
    }
}

impl ProviderAttemptOutcome {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl ProviderResponseOutcome {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotParsed => "not_parsed",
            Self::Parsed => "parsed",
            Self::ProviderError => "provider_error",
            Self::InvalidResponse => "invalid_response",
        }
    }
}

impl ProviderRetryDecision {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Scheduled => "scheduled",
            Self::Suppressed => "suppressed",
            Self::Exhausted => "exhausted",
            Self::NotRetryable => "not_retryable",
        }
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis() as u64
}

mod send_failure_serde {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    use super::HttpSendFailure;

    pub(super) fn serialize<S>(
        failure: &Option<HttpSendFailure>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match failure {
            Some(failure) => serializer.serialize_some(failure.as_str()),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<HttpSendFailure>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|failure| match failure.as_str() {
                "timeout" | "Timeout" => Ok(HttpSendFailure::Timeout),
                "failed" | "Failed" => Ok(HttpSendFailure::Failed),
                other => Err(D::Error::custom(format!(
                    "unknown provider send failure '{other}'"
                ))),
            })
            .transpose()
    }
}

mod body_failure_serde {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    use super::HttpBodyFailure;

    pub(super) fn serialize<S>(
        failure: &Option<HttpBodyFailure>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match failure {
            Some(failure) => serializer.serialize_some(failure.as_str()),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<HttpBodyFailure>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|failure| match failure.as_str() {
                "timeout" | "Timeout" => Ok(HttpBodyFailure::Timeout),
                "read_failed" | "ReadFailed" => Ok(HttpBodyFailure::ReadFailed),
                "decode_failed" | "DecodeFailed" => Ok(HttpBodyFailure::DecodeFailed),
                other => Err(D::Error::custom(format!(
                    "unknown provider body failure '{other}'"
                ))),
            })
            .transpose()
    }
}

impl<R: Router> AttemptBuilder<Streaming<R>> {
    #[allow(
        dead_code,
        reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
    )]
    #[must_use]
    pub fn streaming(started: Duration) -> Self {
        Self::new(started)
    }

    #[allow(
        dead_code,
        reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
    )]
    #[must_use]
    pub fn streaming_from(origin: Instant) -> Self {
        Self::from_origin(origin)
    }

    #[allow(
        dead_code,
        reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
    )]
    #[must_use]
    fn with_output_started(mut self, elapsed: Duration) -> Self {
        self.output_started = Some(elapsed);
        self
    }

    #[allow(
        dead_code,
        reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
    )]
    #[must_use]
    pub fn output_started(self) -> Self {
        let elapsed = self.current_offset();
        self.with_output_started(elapsed)
    }

    #[allow(
        dead_code,
        reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
    )]
    #[must_use]
    fn with_output_progress(mut self, elapsed: Duration) -> Self {
        self.output_progress = Some(elapsed);
        self
    }

    #[allow(
        dead_code,
        reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
    )]
    #[must_use]
    pub fn output_progress(self) -> Self {
        let elapsed = self.current_offset();
        self.with_output_progress(elapsed)
    }

    #[allow(
        dead_code,
        reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
    )]
    #[must_use]
    fn with_output_completed(mut self, elapsed: Duration) -> Self {
        self.output_completed = Some(elapsed);
        self
    }

    #[allow(
        dead_code,
        reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
    )]
    #[must_use]
    pub fn output_completed(self) -> Self {
        let elapsed = self.current_offset();
        self.with_output_completed(elapsed)
    }
}

impl<R: Router> AttemptBuilder<NonStreaming<R>> {
    #[must_use]
    pub fn non_streaming(started: Duration) -> Self {
        Self::new(started)
    }

    #[must_use]
    pub fn non_streaming_from(origin: Instant) -> Self {
        Self::from_origin(origin)
    }

    #[must_use]
    fn with_body_received(mut self, elapsed: Duration) -> Self {
        self.output_started = Some(elapsed);
        self.output_completed = Some(elapsed);
        self
    }

    #[must_use]
    pub fn body_received(self) -> Self {
        let elapsed = self.current_offset();
        self.with_body_received(elapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenRouter;

    #[test]
    fn non_streaming_body_received_collapses_output_start_and_completion() {
        let attempt =
            AttemptBuilder::<NonStreaming<OpenRouter>>::non_streaming(Duration::from_secs(10))
                .with_request_sent(Duration::from_secs(11))
                .with_headers_received(Duration::from_secs(12))
                .with_body_received(Duration::from_secs(17));

        assert_eq!(attempt.started_at(), Duration::from_secs(10));
        assert_eq!(attempt.request_sent_at(), Some(Duration::from_secs(11)));
        assert_eq!(attempt.headers_received_at(), Some(Duration::from_secs(12)));
        assert_eq!(attempt.output_started_at(), Some(Duration::from_secs(17)));
        assert_eq!(attempt.output_completed_at(), Some(Duration::from_secs(17)));
        assert_eq!(
            attempt.elapsed_since_started(Duration::from_secs(17)),
            Duration::from_secs(7)
        );
        assert_eq!(
            attempt.headers_received_elapsed(),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            attempt.output_completed_elapsed(),
            Some(Duration::from_secs(7))
        );
        assert!(attempt.completed());
        assert!(attempt.is_terminal());
    }

    #[test]
    fn finish_preserves_per_attempt_elapsed_facts() {
        let attempt =
            AttemptBuilder::<NonStreaming<OpenRouter>>::non_streaming(Duration::from_secs(10))
                .with_request_sent(Duration::from_secs(11))
                .with_headers_received(Duration::from_secs(12))
                .with_body_received(Duration::from_secs(17))
                .status(200)
                .response_bytes(123)
                .response_outcome(ProviderResponseOutcome::Parsed)
                .finish(42, 2, 3);

        assert_eq!(attempt.request_id, 42);
        assert_eq!(attempt.attempt, 2);
        assert_eq!(attempt.max_attempts, 3);
        assert_eq!(attempt.started_at, Duration::from_secs(10));
        assert_eq!(attempt.request_sent, Some(Duration::from_secs(1)));
        assert_eq!(attempt.headers_received, Some(Duration::from_secs(2)));
        assert_eq!(attempt.output_started, Some(Duration::from_secs(7)));
        assert_eq!(attempt.output_completed, Some(Duration::from_secs(7)));
        assert_eq!(attempt.status, Some(200));
        assert_eq!(attempt.response_bytes, Some(123));
        assert_eq!(attempt.outcome, ProviderAttemptOutcome::Completed);
        assert_eq!(attempt.response_outcome, ProviderResponseOutcome::Parsed);
        assert_eq!(attempt.retry_decision, ProviderRetryDecision::None);
    }

    #[test]
    fn provider_attempt_timeline_roundtrips_attempt_facts() {
        let attempt =
            AttemptBuilder::<NonStreaming<OpenRouter>>::non_streaming(Duration::from_secs(10))
                .with_request_sent(Duration::from_secs(11))
                .with_headers_received(Duration::from_secs(12))
                .body_failure(HttpBodyFailure::Timeout)
                .failure_phase(ProviderFailurePhase::Body)
                .retry_decision(ProviderRetryDecision::Scheduled)
                .retry_after(Duration::from_millis(125))
                .backoff(Duration::from_millis(250))
                .error("response body timed out")
                .finish(42, 2, 3);

        let timeline = ProviderAttemptTimeline::from_attempt(&attempt);
        assert_eq!(timeline.started_at_ms, 10_000);
        assert_eq!(timeline.request_sent_ms, Some(1_000));
        assert_eq!(timeline.headers_received_ms, Some(2_000));
        assert_eq!(timeline.body_failure, Some(HttpBodyFailure::Timeout));
        assert_eq!(timeline.retry_after_ms, Some(125));
        assert_eq!(timeline.backoff_ms, Some(250));
        assert_eq!(timeline.error.as_deref(), Some("response body timed out"));

        let value = serde_json::to_value(&timeline).expect("serialize provider attempt timeline");
        assert_eq!(value["transport_outcome"], "failed");
        assert_eq!(value["body_failure"], "timeout");
        assert!(value.get("outcome").is_none());

        let decoded: ProviderAttemptTimeline =
            serde_json::from_value(value).expect("deserialize provider attempt timeline");
        assert_eq!(decoded.into_attempt(), attempt);
    }

    #[test]
    fn historical_timeline_defaults_missing_response_outcome() {
        let value = serde_json::json!({
            "request_id": 42,
            "attempt": 1,
            "max_attempts": 1,
            "started_at_ms": 0,
            "status": 200,
            "response_bytes": 123,
            "outcome": "completed",
            "body_failure": "ReadFailed",
            "retry_decision": "none"
        });

        let timeline: ProviderAttemptTimeline =
            serde_json::from_value(value).expect("historical provider attempt");

        assert_eq!(
            timeline.response_outcome,
            ProviderResponseOutcome::NotParsed
        );
        assert_eq!(timeline.body_failure, Some(HttpBodyFailure::ReadFailed),);
    }
}
