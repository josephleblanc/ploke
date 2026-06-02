//! Thin tracing helpers for Prototype 1 runtime observation.
//!
//! This module is telemetry-only. History and journal records remain the
//! authoritative projections of allowed protocol transitions.

use std::fmt;
use std::future::Future;
use std::io;
use std::process::Output;
use std::time::{Duration, Instant};

use tracing::{Instrument, Level, Span, event};

pub(crate) const TARGET: &str = ploke_core::EXECUTION_DEBUG_TARGET;

macro_rules! span {
    ($name:literal) => {
        tracing::info_span!(target: $crate::cli::prototype1_state::observe::TARGET, $name)
    };
    ($name:literal, $($fields:tt)+) => {
        tracing::info_span!(target: $crate::cli::prototype1_state::observe::TARGET, $name, $($fields)+)
    };
}

pub(crate) use span;

#[derive(Debug)]
pub(crate) struct Step {
    span: Span,
    started: Instant,
}

impl Step {
    pub(crate) fn start(span: Span) -> Self {
        Self {
            span,
            started: Instant::now(),
        }
    }

    pub(crate) fn success(self) {
        self.finish("succeeded");
    }

    pub(crate) fn rejected(self) {
        self.finish("rejected");
    }

    pub(crate) fn removed(self) {
        self.finish("removed");
    }

    pub(crate) fn missing(self) {
        self.finish("missing");
    }

    pub(crate) fn timed_out(self) {
        self.finish("timed_out");
    }

    pub(crate) fn exited_before_ready(self) {
        self.finish("exited_before_ready");
    }

    pub(crate) fn fail(self, phase: &'static str, error: impl fmt::Display) {
        let duration_ms = duration_ms(self.started.elapsed());
        event!(
            target: TARGET,
            parent: &self.span,
            Level::WARN,
            outcome = "failed",
            phase,
            duration_ms,
            error = %error,
            "prototype1 step failed"
        );
    }

    fn finish(self, outcome: &'static str) {
        let duration_ms = duration_ms(self.started.elapsed());
        event!(
            target: TARGET,
            parent: &self.span,
            Level::INFO,
            outcome,
            duration_ms,
            "prototype1 step finished"
        );
    }
}

pub(crate) fn command_output<F>(span: Span, program: &'static str, run: F) -> io::Result<Output>
where
    F: FnOnce() -> io::Result<Output>,
{
    let started = Instant::now();
    let result = {
        let _entered = span.enter();
        run()
    };

    match result {
        Ok(output) => {
            let outcome = if output.status.success() {
                "succeeded"
            } else {
                "rejected"
            };
            event!(
                target: TARGET,
                parent: &span,
                Level::INFO,
                outcome,
                duration_ms = duration_ms(started.elapsed()),
                program,
                exit_code = ?output.status.code(),
                stdout_excerpt = ?excerpt(&output.stdout),
                stderr_excerpt = ?excerpt(&output.stderr),
                "prototype1 command finished"
            );
            Ok(output)
        }
        Err(error) => {
            event!(
                target: TARGET,
                parent: &span,
                Level::WARN,
                outcome = "failed",
                duration_ms = duration_ms(started.elapsed()),
                program,
                error = %error,
                "prototype1 command failed"
            );
            Err(error)
        }
    }
}

pub(crate) fn io_result<T, F>(span: Span, phase: &'static str, run: F) -> io::Result<T>
where
    F: FnOnce() -> io::Result<T>,
{
    let started = Instant::now();
    let result = {
        let _entered = span.enter();
        run()
    };

    match result {
        Ok(value) => {
            event!(
                target: TARGET,
                parent: &span,
                Level::INFO,
                outcome = "succeeded",
                duration_ms = duration_ms(started.elapsed()),
                phase,
                "prototype1 io step finished"
            );
            Ok(value)
        }
        Err(error) => {
            event!(
                target: TARGET,
                parent: &span,
                Level::WARN,
                outcome = "failed",
                duration_ms = duration_ms(started.elapsed()),
                phase,
                error = %error,
                "prototype1 io step failed"
            );
            Err(error)
        }
    }
}

pub(crate) fn result<T, E, F>(span: Span, run: F) -> Result<T, E>
where
    E: fmt::Display,
    F: FnOnce() -> Result<T, E>,
{
    let started = Instant::now();
    let result = {
        let _entered = span.enter();
        run()
    };
    finish_result(span, started, result)
}

pub(crate) async fn future<T, E, Fut>(span: Span, future: Fut) -> Result<T, E>
where
    E: fmt::Display,
    Fut: Future<Output = Result<T, E>>,
{
    let started = Instant::now();
    let result = future.instrument(span.clone()).await;
    finish_result(span, started, result)
}

fn finish_result<T, E>(span: Span, started: Instant, result: Result<T, E>) -> Result<T, E>
where
    E: fmt::Display,
{
    match result {
        Ok(value) => {
            event!(
                target: TARGET,
                parent: &span,
                Level::INFO,
                outcome = "succeeded",
                duration_ms = duration_ms(started.elapsed()),
                "prototype1 result step finished"
            );
            Ok(value)
        }
        Err(error) => {
            event!(
                target: TARGET,
                parent: &span,
                Level::WARN,
                outcome = "failed",
                duration_ms = duration_ms(started.elapsed()),
                error = %error,
                "prototype1 result step failed"
            );
            Err(error)
        }
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn excerpt(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut excerpt = trimmed.chars().take(4000).collect::<String>();
    if trimmed.chars().count() > 4000 {
        excerpt.push_str("...");
    }
    Some(excerpt)
}
