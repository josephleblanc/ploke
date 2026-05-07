#![allow(dead_code)] // Phase 1 boundary; live adapters are wired in later phases.

//! Authority-side edit-surface boundary for Prototype 1 candidate creation.
//!
//! This module intentionally stops before any live `ploke-tui` or `ploke-db`
//! wiring. It defines the ploke-eval-owned carriers that bind a graph
//! projection to one Artifact, grant a writable surface, check a proposal, and
//! project an admitted patch-shaped result.

pub(crate) mod graph;
pub(crate) mod harness;
pub(crate) mod surface;

pub(crate) use harness::ArtifactDelta;

#[cfg(test)]
mod tests;
