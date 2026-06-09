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
    use tempfile::tempdir;

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
        let journal_path = tmp.path().join("transition-journal.jsonl");
        let result_path = tmp.path().join("result.json");
        let child = Child::<Starting>::new(journal_path.clone(), runtime_id, 1, refs, paths, 4242);

        let result = child
            .ready()
            .expect("ready")
            .evaluating()
            .expect("evaluating")
            .result_written(result_path.clone());
        result.expect("result written");

        let entries = PrototypeJournal::new(journal_path)
            .load_entries()
            .expect("load child transition journal");
        let records = entries
            .iter()
            .map(|entry| match entry {
                JournalEntry::Child(record) => record,
                other => panic!("unexpected journal entry: {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 3);

        assert_eq!(records[0].runtime_id, runtime_id);
        assert_eq!(records[0].generation, 1);
        assert_eq!(records[0].refs.campaign_id, "campaign-a");
        assert_eq!(records[0].refs.node_id, "node-child");
        assert_eq!(records[0].refs.branch_id, "branch-child");
        assert_eq!(records[0].state, State::Ready);

        assert_eq!(records[1].runtime_id, runtime_id);
        assert_eq!(records[1].refs.candidate_id, "candidate-child");
        assert_eq!(records[1].state, State::Evaluating);

        assert_eq!(records[2].runtime_id, runtime_id);
        assert_eq!(
            records[2].state,
            State::ResultWritten {
                runner_result_path: result_path,
            }
        );
    }
}
