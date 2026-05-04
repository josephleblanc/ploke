use std::{
    marker::PhantomData,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::Router;
use crate::error::HttpBodyFailure;

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
    pub outcome: ProviderAttemptOutcome,
    pub failure_phase: Option<ProviderFailurePhase>,
    pub body_failure: Option<HttpBodyFailure>,
    pub retry_decision: ProviderRetryDecision,
    pub backoff: Option<Duration>,
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
    body_failure: Option<HttpBodyFailure>,
    retry_decision: ProviderRetryDecision,
    backoff: Option<Duration>,
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
            body_failure: None,
            retry_decision: ProviderRetryDecision::None,
            backoff: None,
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
    pub fn body_failure(mut self, failure: HttpBodyFailure) -> Self {
        self.body_failure = Some(failure);
        self
    }

    #[must_use]
    pub fn retry_decision(mut self, decision: ProviderRetryDecision) -> Self {
        self.retry_decision = decision;
        self
    }

    #[must_use]
    pub fn backoff(mut self, backoff: Duration) -> Self {
        self.backoff = Some(backoff);
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
            body_failure: self.body_failure,
            retry_decision: self.retry_decision,
            backoff: self.backoff,
        }
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
        assert_eq!(attempt.retry_decision, ProviderRetryDecision::None);
    }
}
