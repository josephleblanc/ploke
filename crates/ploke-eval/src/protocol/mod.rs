//! Compatibility bridge to the dedicated `ploke-protocol` crate.
//!
//! Protocol structure now lives in `crates/ploke-protocol`. Keep this module as
//! a thin re-export so `ploke-eval` can depend on the shared protocol types
//! without treating them as local scratch definitions.

pub use ploke_protocol::*;
