#![allow(dead_code)] // REMOVE BY 2026-04-27: workspace action sketch is not wired into the live controller yet

//! Workspace lifecycle sketch for the active-checkout / child-worktree model.
//!
//! This module does not implement the full loop yet. It exists to pin down the
//! concrete objects we want the typed loop to operate over now that git
//! worktrees are the first realization backend.
//!
//! The intended shape is:
//! - one active checkout for the currently authoritative parent runtime
//! - many ephemeral child worktrees for parallel child realization/build/eval
//! - one shared external record area that is not git-tracked
//!
//! The important rule is that logical nodes in the tree are durable, while
//! realized workspaces are cache-like and may be created and pruned as policy
//! allows.

use std::path::PathBuf;

use super::backend::{Workspace, WorkspaceBackend};
use super::event::RuntimeId;

/// Shared non-git-tracked paths visible to both parent and child runtimes.
///
/// These paths must stay stable across worktree creation, cleanup, and active
/// checkout updates. Handshakes, transition journals, and runner results live
/// here rather than inside any git-tracked workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedPaths {
    pub journal: PathBuf,
    pub scheduler_root: PathBuf,
    pub runner_root: PathBuf,
}

/// Canonical checkout for the currently authoritative parent runtime.
///
/// This is the workspace the parent uses while generating and selecting child
/// branches. It stays clean while candidate children are realized in parallel
/// worktrees. Once a successor is selected, this checkout may be updated to
/// that selected branch/state before the next parent runtime is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveCheckout<B: WorkspaceBackend> {
    pub branch: B::Branch,
    pub head: B::Head,
    pub root: B::Root,
}

/// One ephemeral child realization used for parallel child evaluation.
///
/// A child worktree is a realized descendant artifact world. It can be built
/// and executed independently of the active checkout. Keeping a child worktree
/// on disk is a cache decision, not the durable lineage model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildWorktree<B: WorkspaceBackend> {
    pub workspace: Workspace<B::Branch, B::Head, B::Root>,
    pub binary_path: PathBuf,
    pub committed: bool,
    pub runtime: Option<RuntimeId>,
}

/// Workspace-facing view of the current operating world.
///
/// This is narrower than the full prototype configuration. It only names the
/// parts needed to reason about active checkout updates, child worktree
/// creation, and shared external records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct World<B: WorkspaceBackend> {
    pub active: ActiveCheckout<B>,
    pub children: Vec<ChildWorktree<B>>,
    pub shared: SharedPaths,
}

/// High-level actions the active runtime performs over the workspace model.
///
/// These are not all implemented yet. The point of naming them here is to make
/// the intended intervention surface explicit before we thread them through the
/// rest of the typed loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action<B: WorkspaceBackend> {
    Create(Create<B>),
    Select(Select<B>),
    Update(Update<B>),
    Build(Build<B>),
    Cleanup(Cleanup<B>),
    Exit(Exit<B>),
}

/// Create one candidate child branch/worktree from the current active world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Create<B: WorkspaceBackend> {
    pub source_branch: B::Branch,
    pub child_branch: B::Branch,
    pub keep_workspace: bool,
}

/// Choose which branch should continue as the next active lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Select<B: WorkspaceBackend> {
    pub successor: B::Branch,
}

/// Update the active checkout to the selected successor branch/state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Update<B: WorkspaceBackend> {
    pub successor: B::Branch,
}

/// Build the runtime that will execute from the selected successor state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Build<B: WorkspaceBackend> {
    pub successor: B::Branch,
}

/// Prune one child worktree once its result has been recorded and policy
/// allows cleanup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cleanup<B: WorkspaceBackend> {
    pub branch: B::Branch,
}

/// Terminate the current authoritative parent runtime, optionally after a
/// successor has acknowledged handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Exit<B: WorkspaceBackend> {
    pub successor: Option<B::Branch>,
}
