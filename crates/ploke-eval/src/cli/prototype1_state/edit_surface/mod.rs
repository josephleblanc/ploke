#![allow(dead_code)] // Phase 1 boundary; live adapters are wired in later phases.

//! Authority-side edit-surface boundary for Prototype 1 candidate creation.
//!
//! This module intentionally stops before any live `ploke-tui` or `ploke-db`
//! wiring. It defines the ploke-eval-owned carriers that bind a graph
//! projection to one Artifact, grant a writable surface, publish a non-authority
//! broad-harness request/result contract with mandatory authority binding,
//! check a proposal, mint an admitted edit transaction, and project child
//! evidence from that transaction.

pub(crate) mod diagnosis;
pub(crate) mod graph;
pub(crate) mod harness;
pub(crate) mod harness_request;
pub(crate) mod harness_result;
pub(crate) mod request_policy;
pub(crate) mod route;
pub(crate) mod surface;
pub(crate) mod tui;
pub(crate) mod tui_adapter;

pub(crate) use harness::ArtifactDelta;

#[cfg(test)]
mod tests;
