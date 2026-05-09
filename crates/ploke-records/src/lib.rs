//! Passive record schemas shared across Ploke tools.
//!
//! This crate contains only data shapes for persisted files and machine-readable
//! command output. Deserializing a type from this crate means the bytes parsed
//! into a known record shape; it does not validate authority, advance runtime
//! state, admit History entries, or prove a Crown/Block transition.
//!
//! Functional typestate, validation, sealing, admission, scheduling, filesystem
//! mutation, and runtime advancement remain in `ploke-eval`. UI and projection
//! crates may depend on these records to read evidence surfaces without gaining
//! authority constructors.

pub mod branch;
pub mod channel;
pub mod history;
pub mod identity;
pub mod ids;
pub mod invocation;
pub mod journal;
pub mod scheduler;
pub mod value;
