//! Thin tracing helpers for Prototype 1 runtime observation.
//!
//! This module is telemetry-only. History and journal records remain the
//! authoritative projections of allowed protocol transitions.

use std::fmt;
use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::process::Output;
use std::time::{Duration, Instant};

use tracing::{Instrument, Level, Span, event};

use super::{
    identity::ParentIdentity,
    inner::{At, Transition},
    parent::ChildPlanFile,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Parent,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Self::Parent => "parent",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pipeline {
    ChildPlanAuthority,
}

impl Pipeline {
    fn as_str(self) -> &'static str {
        match self {
            Self::ChildPlanAuthority => "prototype1.child_plan_authority",
        }
    }
}

impl fmt::Display for Pipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    TypestateTransition,
    RequestPublication,
    BatchAdmission,
    FailedBatchPersistence,
    RetryReplay,
    MessageLock,
    MessageReceive,
}

impl Stage {
    fn as_str(self) -> &'static str {
        match self {
            Self::TypestateTransition => "typestate_transition",
            Self::RequestPublication => "request_publication",
            Self::BatchAdmission => "batch_admission",
            Self::FailedBatchPersistence => "failed_batch_persistence",
            Self::RetryReplay => "retry_replay",
            Self::MessageLock => "message_lock",
            Self::MessageReceive => "message_receive",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Authority {
    ParentBroadcastChannel,
}

impl Authority {
    fn as_str(self) -> &'static str {
        match self {
            Self::ParentBroadcastChannel => "parent_broadcast_channel",
        }
    }
}

impl fmt::Display for Authority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordKind {
    ChildPlanFile,
}

impl RecordKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ChildPlanFile => "child_plan_file",
        }
    }
}

impl fmt::Display for RecordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordAccess {
    Read,
    Write,
}

impl RecordAccess {
    fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

impl fmt::Display for RecordAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RecordRef<'a> {
    ChildPlanFile(&'a At<ChildPlanFile>),
}

impl RecordRef<'_> {
    fn kind(&self) -> RecordKind {
        match self {
            Self::ChildPlanFile(_) => RecordKind::ChildPlanFile,
        }
    }

    fn path(&self) -> &std::path::Path {
        match self {
            Self::ChildPlanFile(at) => at.path(),
        }
    }
}

pub(crate) trait ObservedTransition: Transition {
    const ROLE: Role;
    const PIPELINE: Pipeline;
    const STAGE: Stage;
    const AUTHORITY: Authority;
    const LABEL: &'static str;
}

#[derive(Debug, Clone, Copy)]
struct RecordSlot<'a> {
    access: RecordAccess,
    record: RecordRef<'a>,
}

const MAX_TRANSITION_RECORDS: usize = 6;

#[derive(Debug, Clone, Copy)]
struct RecordSlots<'a> {
    records: [Option<RecordSlot<'a>>; MAX_TRANSITION_RECORDS],
    len: usize,
}

impl<'a> RecordSlots<'a> {
    fn empty() -> Self {
        Self {
            records: [None; MAX_TRANSITION_RECORDS],
            len: 0,
        }
    }

    fn push(&mut self, access: RecordAccess, record: RecordRef<'a>) {
        if self.len < MAX_TRANSITION_RECORDS {
            self.records[self.len] = Some(RecordSlot { access, record });
            self.len += 1;
        } else {
            debug_assert!(
                false,
                "prototype1 typestate transition recorded too many persistence refs"
            );
        }
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn len(&self) -> usize {
        self.len
    }

    fn iter(&self) -> impl Iterator<Item = RecordSlot<'a>> + '_ {
        self.records[..self.len].iter().filter_map(|record| *record)
    }
}

#[derive(Debug)]
pub(crate) struct TransitionBuilder<'a, T: ObservedTransition> {
    parent: &'a ParentIdentity,
    stage: Stage,
    records: RecordSlots<'a>,
    _transition: PhantomData<T>,
}

pub(crate) fn transition<'a, T: ObservedTransition>(
    parent: &'a ParentIdentity,
) -> TransitionBuilder<'a, T> {
    TransitionBuilder {
        parent,
        stage: T::STAGE,
        records: RecordSlots::empty(),
        _transition: PhantomData,
    }
}

impl<'a, T: ObservedTransition> TransitionBuilder<'a, T> {
    pub(crate) fn stage(mut self, stage: Stage) -> Self {
        self.stage = stage;
        self
    }

    pub(crate) fn reads(mut self, record: RecordRef<'a>) -> Self {
        self.records.push(RecordAccess::Read, record);
        self
    }

    pub(crate) fn writes(mut self, record: RecordRef<'a>) -> Self {
        self.records.push(RecordAccess::Write, record);
        self
    }

    pub(crate) fn commit<R>(self, run: impl FnOnce() -> R) -> R {
        let started = Instant::now();
        let value = run();
        self.emit("committed", started.elapsed(), None);
        value
    }

    pub(crate) fn try_commit<R, E>(self, run: impl FnOnce() -> Result<R, E>) -> Result<R, E>
    where
        E: fmt::Display,
    {
        let started = Instant::now();
        let result = run();
        match &result {
            Ok(_) => self.emit("committed", started.elapsed(), None),
            Err(error) => self.emit("failed", started.elapsed(), Some(error)),
        }
        result
    }

    fn emit(&self, outcome: &'static str, duration: Duration, error: Option<&dyn fmt::Display>) {
        if self.records.is_empty() {
            self.emit_one(outcome, duration, error, None, 0);
        } else {
            for (index, record) in self.records.iter().enumerate() {
                self.emit_one(outcome, duration, error, Some(record), index);
            }
        }
    }

    fn emit_one(
        &self,
        outcome: &'static str,
        duration: Duration,
        error: Option<&dyn fmt::Display>,
        record: Option<RecordSlot<'_>>,
        record_index: usize,
    ) {
        let duration_ms = duration_ms(duration);
        let record_count = self.records.len();
        let parent = self.parent;
        match (record, error) {
            (Some(record), Some(error)) => event!(
                target: TARGET,
                Level::WARN,
                event = "typestate_transition",
                role = %T::ROLE,
                pipeline = %T::PIPELINE,
                phase = %self.stage,
                authority = %T::AUTHORITY,
                transition = T::LABEL,
                outcome,
                duration_ms,
                campaign_id = %parent.campaign_id(),
                parent_id = %parent.parent_id(),
                node_id = %parent.node_id(),
                generation = parent.generation(),
                branch_id = %parent.branch_id(),
                record_access = %record.access,
                record_kind = %record.record.kind(),
                record_path = %record.record.path().display(),
                record_index,
                record_count,
                error = %error,
                "prototype1 typestate transition failed"
            ),
            (Some(record), None) => event!(
                target: TARGET,
                Level::INFO,
                event = "typestate_transition",
                role = %T::ROLE,
                pipeline = %T::PIPELINE,
                phase = %self.stage,
                authority = %T::AUTHORITY,
                transition = T::LABEL,
                outcome,
                duration_ms,
                campaign_id = %parent.campaign_id(),
                parent_id = %parent.parent_id(),
                node_id = %parent.node_id(),
                generation = parent.generation(),
                branch_id = %parent.branch_id(),
                record_access = %record.access,
                record_kind = %record.record.kind(),
                record_path = %record.record.path().display(),
                record_index,
                record_count,
                "prototype1 typestate transition committed"
            ),
            (None, Some(error)) => event!(
                target: TARGET,
                Level::WARN,
                event = "typestate_transition",
                role = %T::ROLE,
                pipeline = %T::PIPELINE,
                phase = %self.stage,
                authority = %T::AUTHORITY,
                transition = T::LABEL,
                outcome,
                duration_ms,
                campaign_id = %parent.campaign_id(),
                parent_id = %parent.parent_id(),
                node_id = %parent.node_id(),
                generation = parent.generation(),
                branch_id = %parent.branch_id(),
                record_count,
                error = %error,
                "prototype1 typestate transition failed"
            ),
            (None, None) => event!(
                target: TARGET,
                Level::INFO,
                event = "typestate_transition",
                role = %T::ROLE,
                pipeline = %T::PIPELINE,
                phase = %self.stage,
                authority = %T::AUTHORITY,
                transition = T::LABEL,
                outcome,
                duration_ms,
                campaign_id = %parent.campaign_id(),
                parent_id = %parent.parent_id(),
                node_id = %parent.node_id(),
                generation = parent.generation(),
                branch_id = %parent.branch_id(),
                record_count,
                "prototype1 typestate transition committed"
            ),
        }
    }
}

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
