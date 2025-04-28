# Known Limitation: P3-00 - ModuleTree Construction Fails on Duplicate Paths from `cfg`

**Date:** 2025-04-23
**Status:** Active
**Component:** `syn_parser::parser::module_tree` (`ModuleTree::add_module`, `CodeGraph::build_module_tree`)
**Related Tests:** `crates/ingest/syn_parser/tests/uuid_phase3_resolution/edge_cases.rs` (specifically tests involving `cfg_mod`, e.g., `test_spp_cfg_exclusive_a`, `test_spp_cfg_nested_exclusive_ab`, etc., which were initially failing due to this issue).

## Description

The current implementation of `CodeGraph::build_module_tree` fails when processing a `CodeGraph` derived from source code containing multiple module definitions that resolve to the same logical path, even if those definitions are gated by mutually exclusive `#[cfg]` attributes.

## Root Cause

1.  **Syntactic Parsing (Phase 2):** The initial parsing phase (`analyze_files_parallel`) operates syntactically. It does not evaluate `#[cfg]` attributes. Consequently, if a file contains multiple `mod foo { ... }` blocks with the same name but different `#[cfg]` attributes (e.g., `#[cfg(feature = "a")] mod foo {}` and `#[cfg(not(feature = "a"))] mod foo {}`), Phase 2 generates distinct `ModuleNode` instances for each block in the `CodeGraph`.
2.  **ModuleTree Indexing:** During `build_module_tree`, the `ModuleTree::add_module` function is called for each `ModuleNode` from the `CodeGraph`. This function uses the module's definition path (`module.defn_path()`) as a key to populate internal indices (`path_index` or `decl_index`).
3.  **Path Collision:** When `add_module` encounters the *second* `ModuleNode` corresponding to the same logical path (e.g., `["crate", "foo"]`), it detects a collision in the index because that path key is already associated with the `NodeId` of the first `ModuleNode`.
4.  **Error:** This collision results in a `ModuleTreeError::DuplicatePath` error (wrapped as `SynParserError::ModuleTreeDuplicateDefnPath`), causing `build_module_tree` to fail prematurely.

## Impact

*   The `ModuleTree` cannot be successfully constructed for any crate or fixture containing `#[cfg]`-gated modules that share the same logical path.
*   This prevents testing of *any* Phase 3 resolution logic (like `shortest_public_path`) using fixtures that exhibit this pattern (e.g., `fixture_spp_edge_cases`).
*   Tests targeting such fixtures will panic during the `build_tree_for_edge_cases()` setup phase.

## Workaround

*   For testing purposes unrelated to `cfg` handling, use modified fixtures where the conflicting `#[cfg]`-gated module definitions are commented out or removed (e.g., `fixture_spp_edge_cases_no_cfg`).
*   Tests specifically designed to verify `cfg` handling in Phase 3 must be marked with `#[ignore]` until this limitation is addressed.

## Resolution Plan

*   Enhance `ModuleTree` construction logic (likely within `add_module` and potentially `link_mods_syntactic` or a new dedicated `cfg`-processing step) to correctly handle multiple `ModuleNode`s sharing a path due to `cfg` attributes.
*   This might involve:
    *   Storing `cfg` information alongside path index entries.
    *   Allowing multiple `NodeId`s per path in the index, differentiated by their `cfg` constraints.
    *   Potentially evaluating `cfg`s based on a target configuration during tree building or resolution (though this adds complexity).
*   See related development branch: `feature/mod_tree_cfg`.
