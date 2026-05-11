//! History-spined read-side graph over loaded Prototype 1 records.
//!
//! This module is the semantic read boundary for graph consumers. It builds
//! from [`crate::RunRecordSet`] and keeps sealed History as the primary lineage
//! authority while treating scheduler, branch, turn, tool, provider, protocol,
//! metric, and log records as evidence attached to graph objects.
//!
//! The graph is intentionally not a visual DTO. Browser, egui, CLI-free
//! investigation, and later analysis projections should borrow or derive from
//! this object instead of reconstructing loop semantics independently.

mod build;
mod types;

pub use types::*;
