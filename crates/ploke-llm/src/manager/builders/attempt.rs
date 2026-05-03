use std::{marker::PhantomData, time::Duration};

use crate::Router;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttemptBuilder<R: StreamingMarker> {
    started: Duration,
    request_sent: Option<Duration>,
    headers_received: Option<Duration>,
    output_started: Option<Duration>,
    output_progress: Option<Duration>,
    output_completed: Option<Duration>,
    failed: Option<Duration>,
    _router: PhantomData<R>,
}

impl<R: StreamingMarker> AttemptBuilder<R> {
    #[must_use]
    pub fn new(started: Duration) -> Self {
        Self {
            started,
            request_sent: None,
            headers_received: None,
            output_started: None,
            output_progress: None,
            output_completed: None,
            failed: None,
            _router: PhantomData,
        }
    }

    #[must_use]
    pub fn started(mut self, started: Duration) -> Self {
        self.started = started;
        self
    }

    #[must_use]
    pub fn request_sent(mut self, elapsed: Duration) -> Self {
        self.request_sent = Some(elapsed);
        self
    }

    #[must_use]
    pub fn headers_received(mut self, elapsed: Duration) -> Self {
        self.headers_received = Some(elapsed);
        self
    }

    #[must_use]
    pub fn failed(mut self, elapsed: Duration) -> Self {
        self.failed = Some(elapsed);
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

    pub fn is_terminal(&self) -> bool {
        self.output_completed.is_some() || self.failed.is_some()
    }

    pub fn completed(&self) -> bool {
        self.output_completed.is_some()
    }

    pub fn failed_at(&self) -> Option<Duration> {
        self.failed
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
    pub fn output_started(mut self, elapsed: Duration) -> Self {
        self.output_started = Some(elapsed);
        self
    }

    #[allow(
        dead_code,
        reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
    )]
    #[must_use]
    pub fn output_progress(mut self, elapsed: Duration) -> Self {
        self.output_progress = Some(elapsed);
        self
    }

    #[allow(
        dead_code,
        reason = "streaming calibration scaffold; integrate when downstream ploke-tui has streaming semantics"
    )]
    #[must_use]
    pub fn output_completed(mut self, elapsed: Duration) -> Self {
        self.output_completed = Some(elapsed);
        self
    }
}

impl<R: Router> AttemptBuilder<NonStreaming<R>> {
    #[must_use]
    pub fn non_streaming(started: Duration) -> Self {
        Self::new(started)
    }

    #[must_use]
    pub fn body_received(mut self, elapsed: Duration) -> Self {
        self.output_started = Some(elapsed);
        self.output_completed = Some(elapsed);
        self
    }
}
