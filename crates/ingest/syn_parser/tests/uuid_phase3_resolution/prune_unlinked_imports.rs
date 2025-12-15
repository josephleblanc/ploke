#![cfg(test)]
//! Regression test ensuring ModuleTree pruning keeps pending import/export queues in sync.
//!
//! ## Context
//!
//! * Parsing some workspace crates (e.g. `ploke-core`) produces additional file-based modules
//!   that are never declared via `mod foo;`. During `build_tree_and_prune` those modules are
//!   correctly marked as “unlinked” and pruned from the tree.
//! * Before this regression existed, the pruning step removed the orphan `ModuleNode` (and its
//!   contained items) from the tree but **did not** scrub `pending_imports` / `pending_exports`.
//!   Later, when `link_definition_imports` iterated those pending entries, it tried to look up the
//!   already-pruned module ID and triggered `ModuleTreeError::ModuleNotFound` (e.g. in
//!   `full::parse_self::new_parse_core`).
//!
//! ## Intent
//!
//! Guard against regressions by building a real workspace crate and verifying pruning succeeds
//! without leaving stale pending imports/exports. If pending entries referencing pruned modules
//! reappear, this test will fail during `build_tree_and_prune`.

use ploke_common::workspace_root;
use syn_parser::{discovery::run_discovery_phase, parser::analyze_files_parallel, ParsedCodeGraph};

/// Builds the module tree for a workspace crate. Mirrors the logic used by the full
/// `parse_self` tests so any fixes applied there remain covered here.
fn build_tree_for_workspace_crate(crate_subpath: &str) -> Result<(), ploke_error::Error> {
    let root = workspace_root();
    let crate_path = root.join("crates").join(crate_subpath);

    let discovery = run_discovery_phase(&root, &[crate_path])?;
    let parsed_graphs = analyze_files_parallel(&discovery, 0);
    let mut successful_graphs = Vec::new();

    for result in parsed_graphs {
        successful_graphs.push(result?);
    }

    let mut merged = ParsedCodeGraph::merge_new(successful_graphs)?;
    merged.build_tree_and_prune().map(|_| ())
}

#[test]
fn prune_unlinked_modules_drop_stale_pending_imports() {
    // ploke-core contains `src/graph.rs`, which is never declared via `mod graph;`. The module
    // tree correctly prunes that file, historically left its ImportNodes in the pending list, which
    // surfaced as ModuleNotFound while linking definition imports. After filtering pending queues
    // against pruned modules, we expect ModuleTree construction to succeed.
    build_tree_for_workspace_crate("ploke-core")
        .expect("Pending imports belonging to pruned modules should be dropped during pruning");
}
