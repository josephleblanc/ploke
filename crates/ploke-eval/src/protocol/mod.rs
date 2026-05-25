//! Compatibility bridge to the dedicated `ploke-protocol` crate.
//!
//! Protocol structure now lives in `crates/ploke-protocol`. Keep this module as
//! a thin re-export so `ploke-eval` can depend on the shared protocol types
//! without treating them as local scratch definitions.

#[path = "../protocol_aggregate.rs"]
pub mod protocol_aggregate;

pub use ploke_protocol::*;
pub use protocol_aggregate::*;
