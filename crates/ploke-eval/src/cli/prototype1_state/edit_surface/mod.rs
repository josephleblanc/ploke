#![allow(dead_code)] // Phase 1 boundary; live adapters are wired in later phases.

//! Authority-side edit-surface boundary for Prototype 1 candidate creation.
//!
//! This module intentionally stops before any live `ploke-tui` or `ploke-db`
//! wiring. It defines the ploke-eval-owned carriers that bind a graph
//! projection to one Artifact, grant a writable surface, publish a non-authority
//! broad-harness request/result contract, check a proposal, and project an
//! admitted patch-shaped result.

pub(crate) mod diagnosis;
pub(crate) mod graph;
pub(crate) mod harness;
pub(crate) mod harness_request;
pub(crate) mod harness_result;
pub(crate) mod request_policy;
pub(crate) mod route;
pub(crate) mod surface;
pub(crate) mod tui;

pub(crate) use diagnosis::{Diagnosis, classify};
pub(crate) use harness::ArtifactDelta;
pub(crate) use route::semantic_resolution;

#[cfg(test)]
mod tests;
