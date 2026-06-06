//! Typed child runtime role-state transitions.
//!
//! This module keeps the communication shape structural: a child runtime is a
//! `Child<State>`, and journal records are durable projections of allowed
//! state transitions.
//!
//! A child is not a thread and not a recursive parent. It is a separately
//! spawned OS process running a candidate runtime for one node-owned worktree.
//! Its authority is intentionally narrower than parent authority: it may
//! acknowledge startup, run its bounded evaluation, write its own attempt
//! result, and send child-shaped protocol messages. It must not stage further
//! children, select successors, mutate parent identity, or make continuation
//! decisions.
//!
//! The durable records emitted here are therefore not generic progress events.
//! They are projections of the small set of state transitions a child runtime is
//! allowed to cross after it has been admitted as a child.

use std::marker::PhantomData;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, instrument};

use crate::intervention::RecordStore;

use super::event::{Paths, RecordedAt, Refs, RuntimeId};
use super::journal::{JournalEntry, PrototypeJournal, PrototypeJournalError, ReadyEntry};

/// State parameter projected by a recorded `Child<State>` transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum State {
    /// Child runtime has started and can be observed by the parent.
    Ready,
    /// Child runtime has entered its bounded evaluation procedure.
    Evaluating,
    /// Child runtime has persisted its attempt-scoped runner result.
    ResultWritten { runner_result_path: PathBuf },
}

/// Durable record written by a typed `Child<State>` transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Record {
    runtime_id: RuntimeId,
    recorded_at: RecordedAt,
    generation: u32,
    refs: Refs,
    paths: Paths,
    pid: u32,
    state: State,
}

impl Record {
    pub(crate) fn runtime_id(&self) -> RuntimeId {
        self.runtime_id
    }

    /// Stable display label for monitor summaries.
    pub(crate) fn entry_kind(&self) -> &'static str {
        match self.state {
            State::Ready => "child:ready",
            State::Evaluating => "child:evaluating",
            State::ResultWritten { .. } => "child:result_written",
        }
    }

    /// Project `Child<Ready>` into the legacy ready witness shape.
    pub(crate) fn ready_entry(&self) -> Option<ReadyEntry> {
        if self.state != State::Ready {
            return None;
        }
        Some(ReadyEntry {
            runtime_id: self.runtime_id,
            recorded_at: self.recorded_at,
            generation: self.generation,
            refs: self.refs.clone(),
            paths: self.paths.clone(),
            pid: self.pid,
        })
    }

    /// Attempt result path when this record is `Child<ResultWritten>`.
    pub(crate) fn result_path(&self, runtime_id: RuntimeId) -> Option<PathBuf> {
        if self.runtime_id != runtime_id {
            return None;
        }
        match &self.state {
            State::ResultWritten { runner_result_path } => Some(runner_result_path.clone()),
            State::Ready | State::Evaluating => None,
        }
    }
}

/// Initial child runtime state before it has acknowledged the parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Starting;

/// Child runtime state after it has acknowledged the parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ready;

/// Child runtime state while it is executing the bounded evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Evaluating;

/// Child runtime state after it has persisted its attempt result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResultWritten;

/// Error produced while recording a child state transition.
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// The transition journal could not be updated.
    #[error("failed to record child state")]
    Record {
        /// Underlying journal failure.
        #[source]
        source: PrototypeJournalError,
    },
}

/// Runtime role carrier for a child in state `S`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Child<S> {
    journal_path: PathBuf,
    runtime_id: RuntimeId,
    generation: u32,
    refs: Refs,
    paths: Paths,
    pid: u32,
    _state: PhantomData<S>,
}

impl Child<Starting> {
    /// Construct a child role before it has acknowledged the parent.
    pub(crate) fn new(
        journal_path: PathBuf,
        runtime_id: RuntimeId,
        generation: u32,
        refs: Refs,
        paths: Paths,
        pid: u32,
    ) -> Self {
        Self {
            journal_path,
            runtime_id,
            generation,
            refs,
            paths,
            pid,
            _state: PhantomData,
        }
    }

    #[instrument(
        target = "ploke_exec",
        level = "info",
        skip(self),
        fields(
            role = "child",
            authority = "child_runtime_channel",
            transition = "Child<Starting>->Child<Ready>",
            campaign_id = %self.refs.campaign_id,
            node_id = %self.refs.node_id,
            generation = self.generation,
            branch_id = %self.refs.branch_id,
            candidate_id = %self.refs.candidate_id,
            runtime_id = %self.runtime_id,
            pid = self.pid,
        )
    )]
    /// Record `Child<Ready>` and return the typed ready state.
    pub(crate) fn ready(self) -> Result<Child<Ready>, Error> {
        info!(
            target: "ploke_exec",
            role = "child",
            authority = "child_runtime_channel",
            transition = "Child<Starting>->Child<Ready>",
            campaign_id = %self.refs.campaign_id,
            node_id = %self.refs.node_id,
            generation = self.generation,
            branch_id = %self.refs.branch_id,
            candidate_id = %self.refs.candidate_id,
            runtime_id = %self.runtime_id,
            pid = self.pid,
            "child runtime acknowledged parent bootstrap"
        );
        self.record(State::Ready)?;
        Ok(self.cast())
    }
}

impl Child<Ready> {
    #[instrument(
        target = "ploke_exec",
        level = "info",
        skip(self),
        fields(
            role = "child",
            authority = "child_runtime_channel",
            transition = "Child<Ready>->Child<Evaluating>",
            campaign_id = %self.refs.campaign_id,
            node_id = %self.refs.node_id,
            generation = self.generation,
            branch_id = %self.refs.branch_id,
            candidate_id = %self.refs.candidate_id,
            runtime_id = %self.runtime_id,
            pid = self.pid,
        )
    )]
    /// Record `Child<Evaluating>` and return the typed evaluating state.
    pub(crate) fn evaluating(self) -> Result<Child<Evaluating>, Error> {
        info!(
            target: "ploke_exec",
            role = "child",
            authority = "child_runtime_channel",
            transition = "Child<Ready>->Child<Evaluating>",
            campaign_id = %self.refs.campaign_id,
            node_id = %self.refs.node_id,
            generation = self.generation,
            branch_id = %self.refs.branch_id,
            candidate_id = %self.refs.candidate_id,
            runtime_id = %self.runtime_id,
            pid = self.pid,
            "child runtime entered bounded evaluation"
        );
        self.record(State::Evaluating)?;
        Ok(self.cast())
    }
}

impl Child<Evaluating> {
    #[instrument(
        target = "ploke_exec",
        level = "info",
        skip(self, runner_result_path),
        fields(
            role = "child",
            authority = "child_runtime_channel",
            transition = "Child<Evaluating>->Child<ResultWritten>",
            campaign_id = %self.refs.campaign_id,
            node_id = %self.refs.node_id,
            generation = self.generation,
            branch_id = %self.refs.branch_id,
            candidate_id = %self.refs.candidate_id,
            runtime_id = %self.runtime_id,
            pid = self.pid,
            runner_result_path = %runner_result_path.display(),
        )
    )]
    /// Record `Child<ResultWritten>` and return the typed result-written state.
    pub(crate) fn result_written(
        self,
        runner_result_path: PathBuf,
    ) -> Result<Child<ResultWritten>, Error> {
        info!(
            target: "ploke_exec",
            role = "child",
            authority = "child_runtime_channel",
            transition = "Child<Evaluating>->Child<ResultWritten>",
            campaign_id = %self.refs.campaign_id,
            node_id = %self.refs.node_id,
            generation = self.generation,
            branch_id = %self.refs.branch_id,
            candidate_id = %self.refs.candidate_id,
            runtime_id = %self.runtime_id,
            pid = self.pid,
            runner_result_path = %runner_result_path.display(),
            "child runtime persisted attempt result"
        );
        self.record(State::ResultWritten { runner_result_path })?;
        Ok(self.cast())
    }
}

impl<S> Child<S> {
    fn cast<T>(self) -> Child<T> {
        Child {
            journal_path: self.journal_path,
            runtime_id: self.runtime_id,
            generation: self.generation,
            refs: self.refs,
            paths: self.paths,
            pid: self.pid,
            _state: PhantomData,
        }
    }

    fn record(&self, state: State) -> Result<(), Error> {
        let mut journal = PrototypeJournal::new(self.journal_path.clone());
        journal
            .append(JournalEntry::Child(Record {
                runtime_id: self.runtime_id,
                recorded_at: RecordedAt::now(),
                generation: self.generation,
                refs: self.refs.clone(),
                paths: self.paths.clone(),
                pid: self.pid,
                state,
            }))
            .map_err(|source| Error::Record { source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;
    use tracing::field::{Field, Visit};
    use tracing::{Event, Id, Subscriber};
    use tracing_subscriber::layer::{Context, SubscriberExt};
    use tracing_subscriber::registry::LookupSpan;
    use tracing_subscriber::{Layer, Registry};

    #[derive(Clone, Default)]
    struct TraceLines(Arc<Mutex<Vec<String>>>);

    impl TraceLines {
        fn push(&self, line: String) {
            self.0.lock().expect("trace lock").push(line);
        }

        fn snapshot(&self) -> Vec<String> {
            self.0.lock().expect("trace lock").clone()
        }
    }

    #[derive(Default)]
    struct TraceFields {
        values: Vec<String>,
    }

    impl TraceFields {
        fn push(&mut self, field: &Field, value: impl Into<String>) {
            self.values
                .push(format!("{}={}", field.name(), value.into()));
        }

        fn finish(self) -> String {
            self.values.join(" ")
        }
    }

    impl Visit for TraceFields {
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.push(field, value.to_string());
        }

        fn record_i64(&mut self, field: &Field, value: i64) {
            self.push(field, value.to_string());
        }

        fn record_u64(&mut self, field: &Field, value: u64) {
            self.push(field, value.to_string());
        }

        fn record_str(&mut self, field: &Field, value: &str) {
            self.push(field, value.to_string());
        }

        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.push(field, format!("{value:?}"));
        }
    }

    struct TraceLayer {
        lines: TraceLines,
    }

    impl<S> Layer<S> for TraceLayer
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            _id: &Id,
            _ctx: Context<'_, S>,
        ) {
            let mut fields = TraceFields::default();
            attrs.record(&mut fields);
            self.lines.push(format!(
                "span:{} {}",
                attrs.metadata().name(),
                fields.finish()
            ));
        }

        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            let mut fields = TraceFields::default();
            event.record(&mut fields);
            self.lines.push(format!(
                "event:{} {}",
                event.metadata().target(),
                fields.finish()
            ));
        }
    }

    fn collect_traces<T>(f: impl FnOnce() -> T) -> (T, Vec<String>) {
        let _trace_capture_guard = crate::test_support::trace_capture_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let lines = TraceLines::default();
        let subscriber = Registry::default().with(TraceLayer {
            lines: lines.clone(),
        });
        let guard = tracing::subscriber::set_default(subscriber);
        crate::test_support::rebuild_trace_interest_cache();
        let result = f();
        drop(guard);
        crate::test_support::rebuild_trace_interest_cache();
        (result, lines.snapshot())
    }

    fn trace_contains(lines: &[String], needles: &[&str]) -> bool {
        lines
            .iter()
            .any(|line| needles.iter().all(|needle| line.contains(needle)))
    }

    #[test]
    fn child_runtime_transitions_emit_authority_trace() {
        let tmp = tempdir().expect("tmp");
        let runtime_id = RuntimeId::new();
        let refs = Refs {
            campaign_id: "campaign-a".to_string(),
            node_id: "node-child".to_string(),
            instance_id: "instance-a".to_string(),
            source_state_id: "source-a".to_string(),
            branch_id: "branch-child".to_string(),
            candidate_id: "candidate-child".to_string(),
            branch_label: "candidate label".to_string(),
            spec_id: "spec-child".to_string(),
        };
        let paths = Paths {
            repo_root: tmp.path().join("repo"),
            workspace_root: tmp.path().join("workspace"),
            binary_path: tmp.path().join("bin/ploke-eval"),
            target_relpath: PathBuf::from("crates/example/src/lib.rs"),
            absolute_path: tmp.path().join("workspace/crates/example/src/lib.rs"),
        };
        let child = Child::<Starting>::new(
            tmp.path().join("transition-journal.jsonl"),
            runtime_id,
            1,
            refs,
            paths,
            4242,
        );

        let (result, trace) = collect_traces(|| {
            child
                .ready()
                .expect("ready")
                .evaluating()
                .expect("evaluating")
                .result_written(tmp.path().join("result.json"))
        });
        result.expect("result written");

        assert!(trace_contains(
            &trace,
            &[
                "transition=Child<Starting>->Child<Ready>",
                "authority=child_runtime_channel",
                "runtime_id=",
                "node_id=node-child",
            ],
        ));
        assert!(trace_contains(
            &trace,
            &[
                "transition=Child<Ready>->Child<Evaluating>",
                "campaign_id=campaign-a",
                "branch_id=branch-child",
            ],
        ));
        assert!(trace_contains(
            &trace,
            &[
                "transition=Child<Evaluating>->Child<ResultWritten>",
                "runner_result_path=",
                "candidate_id=candidate-child",
            ],
        ));
    }
}
