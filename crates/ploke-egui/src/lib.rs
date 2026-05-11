//! Operator UI for inspecting Ploke execution as a graph.
//!
//! This crate centers on a single object: `Graph`. The graph is the
//! operator-facing structure that connects artifacts, hydrated runtimes,
//! transitions, and the evidence attached to them. It is not the source of
//! truth for the loop; it is a read-only projection assembled from typed
//! persisted records.
//!
//! History, protocol records, tool calls, provider attempts, retries, timeouts,
//! patch evidence, and diagnostics can all contribute facts to the graph. They
//! do not become the graph by themselves. Playback order, phases, tracks, and
//! detail panels are views over the graph for human inspection.
//!
//! `ploke-egui` owns the egui-facing operator surface and the projection shapes
//! needed by that surface. Runtime authority and active loop behavior stay out
//! of this crate.

pub mod graph;
