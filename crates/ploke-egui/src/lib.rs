//! Operator UI for inspecting Ploke execution as a graph.
//!
//! This crate centers on renderable projections of `ploke_tree::Graph`. The
//! graph connects artifacts, hydrated runtimes, transitions, and the evidence
//! attached to them. `ploke-tree` owns that semantic object; this crate borrows
//! from it and derives presentation data for egui.
//!
//! Design invariant: `ploke_tree::Graph` is the semantic data object. Views
//! borrow from it or derive temporary data from it; they do not clone semantic
//! facts into parallel authorities. The borrow checker is part of this design
//! discipline: when a view cache cannot hold references into the graph, that
//! cache should own only view state and rejoin with `&ploke_tree::Graph` when
//! it needs semantics. Cloning is reserved for actual view artifacts such as
//! labels, UI keys, or independent interaction state, not for bypassing
//! semantic ownership.
//!
//! History, protocol records, tool calls, provider attempts, retries, timeouts,
//! patch evidence, and diagnostics can all contribute facts to the graph. They
//! do not become the graph by themselves. Playback order, phases, tracks, and
//! detail panels are views over the graph for human inspection.
//!
//! `ploke-egui` owns the egui-facing operator surface and the projection shapes
//! needed by that surface. Runtime authority and active loop behavior stay out
//! of this crate.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod allocation;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-benchmark"))]
#[global_allocator]
static GLOBAL_ALLOCATOR: tracking_allocator::Allocator<std::alloc::System> =
    tracking_allocator::Allocator::system();

#[cfg(not(target_arch = "wasm32"))]
pub mod benchmark;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
pub mod demo;
#[cfg(not(target_arch = "wasm32"))]
pub mod diagnostics;
pub mod import;
#[cfg(not(target_arch = "wasm32"))]
pub mod perf;
#[cfg(not(target_arch = "wasm32"))]
pub mod run_picker;
pub mod ui;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod web;
