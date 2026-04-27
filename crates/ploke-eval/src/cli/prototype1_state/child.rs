//! Typed child runtime role-state transitions.
//!
//! This module keeps the communication shape structural: a child runtime is a
//! `Child<State>`, and journal records are durable projections of allowed
//! state transitions.

use std::marker::PhantomData;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

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

    /// Record `Child<Ready>` and return the typed ready state.
    pub(crate) fn ready(self) -> Result<Child<Ready>, Error> {
        self.record(State::Ready)?;
        Ok(self.cast())
    }
}

impl Child<Ready> {
    /// Record `Child<Evaluating>` and return the typed evaluating state.
    pub(crate) fn evaluating(self) -> Result<Child<Evaluating>, Error> {
        self.record(State::Evaluating)?;
        Ok(self.cast())
    }
}

impl Child<Evaluating> {
    /// Record `Child<ResultWritten>` and return the typed result-written state.
    pub(crate) fn result_written(
        self,
        runner_result_path: PathBuf,
    ) -> Result<Child<ResultWritten>, Error> {
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
