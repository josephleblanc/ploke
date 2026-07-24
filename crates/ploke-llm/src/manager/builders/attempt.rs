use std::{
    env,
    sync::OnceLock,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::error::{HttpBodyFailure, HttpSendFailure};

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

#[derive(Debug)]
pub(crate) struct AttemptBuilder {
    request_id: u64,
    attempt: u32,
    max_attempts: u32,
    started: Duration,
    started_instant: Instant,
    request_sent: Option<Duration>,
    headers_received: Option<Duration>,
    output_started: Option<Duration>,
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
}

impl AttemptBuilder {
    pub(crate) fn new(started: Duration, request_id: u64, attempt: u32, max_attempts: u32) -> Self {
        Self {
            request_id,
            attempt,
            max_attempts,
            started,
            started_instant: Instant::now(),
            request_sent: None,
            headers_received: None,
            output_started: None,
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
        }
    }

    pub(crate) fn request_sent(&mut self) {
        self.request_sent = Some(self.offset());
    }

    pub(crate) fn headers_received(&mut self, status: u16, retry_after: Option<Duration>) {
        self.headers_received = Some(self.offset());
        self.status = Some(status);
        self.retry_after = retry_after;
    }

    pub(crate) fn body_received(&mut self, response_bytes: usize) {
        let elapsed = self.offset();
        self.output_started = Some(elapsed);
        self.output_completed = Some(elapsed);
        self.response_bytes = Some(response_bytes);
    }

    pub(crate) fn send_failed(
        &mut self,
        failure: HttpSendFailure,
        error: impl Into<String>,
    ) -> Duration {
        self.send_failure = Some(failure);
        self.fail(ProviderFailurePhase::Send, error)
    }

    pub(crate) fn body_failed(
        &mut self,
        failure: HttpBodyFailure,
        error: impl Into<String>,
    ) -> Duration {
        self.body_failure = Some(failure);
        self.fail(ProviderFailurePhase::Body, error)
    }

    pub(crate) fn status_failed(&mut self, error: impl Into<String>) {
        self.failure_phase = Some(ProviderFailurePhase::Status);
        self.error = Some(error.into());
    }

    pub(crate) fn response_parsed(&mut self) {
        self.response_outcome = ProviderResponseOutcome::Parsed;
    }

    pub(crate) fn response_rejected(&mut self, outcome: ProviderResponseOutcome) {
        debug_assert!(matches!(
            outcome,
            ProviderResponseOutcome::ProviderError | ProviderResponseOutcome::InvalidResponse
        ));
        self.response_outcome = outcome;
    }

    pub(crate) fn retry_scheduled(&mut self, backoff: Duration) {
        self.retry_decision = ProviderRetryDecision::Scheduled;
        self.backoff = Some(backoff);
    }

    pub(crate) fn retry_stopped(&mut self, decision: ProviderRetryDecision) {
        debug_assert!(matches!(
            decision,
            ProviderRetryDecision::Suppressed
                | ProviderRetryDecision::Exhausted
                | ProviderRetryDecision::NotRetryable
        ));
        self.retry_decision = decision;
    }

    fn fail(&mut self, phase: ProviderFailurePhase, error: impl Into<String>) -> Duration {
        let elapsed = self.offset();
        self.failed = Some(elapsed);
        self.failure_phase = Some(phase);
        self.error = Some(error.into());
        elapsed.saturating_sub(self.started)
    }

    fn offset(&self) -> Duration {
        self.started.saturating_add(self.started_instant.elapsed())
    }

    #[must_use]
    pub(crate) fn finish(self) -> ProviderAttempt {
        let started = self.started;
        let relative =
            |elapsed: Option<Duration>| elapsed.map(|elapsed| elapsed.saturating_sub(started));
        ProviderAttempt {
            request_id: self.request_id,
            attempt: self.attempt,
            max_attempts: self.max_attempts,
            started_at: started,
            request_sent: relative(self.request_sent),
            headers_received: relative(self.headers_received),
            output_started: relative(self.output_started),
            output_progress: None,
            output_completed: relative(self.output_completed),
            failed: relative(self.failed),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_streaming_body_received_collapses_output_start_and_completion() {
        let mut builder = AttemptBuilder::new(Duration::from_secs(10), 42, 2, 3);
        builder.request_sent();
        builder.headers_received(200, None);
        builder.body_received(123);
        builder.response_parsed();
        let attempt = builder.finish();

        assert_eq!(attempt.request_id, 42);
        assert_eq!(attempt.attempt, 2);
        assert_eq!(attempt.max_attempts, 3);
        assert_eq!(attempt.started_at, Duration::from_secs(10));
        assert!(attempt.request_sent.is_some());
        assert!(attempt.headers_received >= attempt.request_sent);
        assert_eq!(attempt.output_started, attempt.output_completed);
        assert!(attempt.output_completed >= attempt.headers_received);
        assert_eq!(attempt.status, Some(200));
        assert_eq!(attempt.response_bytes, Some(123));
        assert_eq!(attempt.outcome, ProviderAttemptOutcome::Completed);
        assert_eq!(attempt.response_outcome, ProviderResponseOutcome::Parsed);
        assert_eq!(attempt.retry_decision, ProviderRetryDecision::None);
    }

    #[test]
    fn provider_attempt_timeline_roundtrips_attempt_facts() {
        let mut builder = AttemptBuilder::new(Duration::from_secs(10), 42, 2, 3);
        builder.request_sent();
        builder.headers_received(200, Some(Duration::from_millis(125)));
        builder.body_failed(HttpBodyFailure::Timeout, "response body timed out");
        builder.retry_scheduled(Duration::from_millis(250));
        let attempt = builder.finish();

        let timeline = ProviderAttemptTimeline::from_attempt(&attempt);
        assert_eq!(timeline.started_at_ms, 10_000);
        assert!(timeline.request_sent_ms.is_some());
        assert!(timeline.headers_received_ms.is_some());
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
        let restored = decoded.into_attempt();
        assert_eq!(ProviderAttemptTimeline::from_attempt(&restored), timeline);
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
