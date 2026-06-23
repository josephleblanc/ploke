//! Prototype 1 eval evidence storage port.
#![cfg_attr(not(test), allow(dead_code))]
//!
//! This module is Domain-C only: it records ordinary eval evidence/projections
//! and must not replace History, channel transport, MessageBox authority,
//! invocation/bootstrap, or artifact/worktree mutation.

mod api;
mod cozo_params;
mod cozo_store;
mod error;
mod evidence;

#[cfg(test)]
mod tests;

pub(crate) use api::{ConfiguredEvalStore, EvalStore};
#[cfg(test)]
pub(crate) use api::{FileDbEvalStore, FsEvalStore};
#[cfg(test)]
pub(crate) use cozo_store::{DbEvalStore, load_owner_eval_database};
pub(crate) use cozo_store::{
    prototype1_eval_store_db_path, write_record_ref_to_owner_db, write_trace_event_to_owner_db,
};
#[cfg(test)]
pub(crate) use evidence::{LogRefEvidence, ObservationJsonlImport, ParentStartedReceipt};
pub(crate) use evidence::{ParentStartedEvidence, RecordRefEvidence, TraceEventEvidence};
