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
    owner_eval_db_file_for_record_path, prototype1_eval_store_db_path,
    write_channel_message_to_owner_db, write_invocation_to_owner_db, write_record_ref_to_owner_db,
    write_trace_event_to_owner_db,
};
pub(crate) use error::EvalStoreError;
pub(crate) use evidence::{
    ChannelMessageEvidence, InvocationEvidence, ParentStartedEvidence, RecordRefEvidence,
    TraceEventEvidence,
};
#[cfg(test)]
pub(crate) use evidence::{LogRefEvidence, ObservationJsonlImport, ParentStartedReceipt};
