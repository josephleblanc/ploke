# Phase 3 (`ModuleTree` & Resolution) Known Limitations

This document tracks known limitations, missing features, or areas where the Phase 3 logic (`ModuleTree` construction, path resolution, visibility checks) deviates from complete Rust semantics or desired behavior. These limitations were primarily discovered during testing and are documented here to inform future development and prevent regressions.

---

## 1. `ModuleTree` Construction Fails on Duplicate Paths from `cfg`

*   **Limitation:** The `CodeGraph::build_module_tree` function fails with a `ModuleTreeError::DuplicatePath` (wrapped as `SynParserError::ModuleTreeDuplicateDefnPath`) when processing a `CodeGraph` containing multiple `ModuleNode`s that resolve to the same logical path due to mutually exclusive `#[cfg]` attributes.
*   **Discovery:** This was identified when all tests in [`crates/ingest/syn_parser/tests/uuid_phase3_resolution/edge_cases.rs`](../../../../crates/ingest/syn_parser/tests/uuid_phase3_resolution/edge_cases.rs) using the `fixture_spp_edge_cases` fixture panicked during the `build_tree_for_edge_cases()` setup phase.
*   **Root Cause:** The `ModuleTree::add_module` function uses the module's definition path as a key for indexing. Since Phase 2 parsing doesn't evaluate `cfg`s, it creates distinct `ModuleNode`s for each `cfg` branch of a module, leading to path collisions during `ModuleTree` construction. See detailed explanation: [`docs/design/known_limitations/P3-00-cfg-duplication.md`](../../../design/known_limitations/P3-00-cfg-duplication.md).
*   **Affected Code:**
    *   `crates/ingest/syn_parser/src/parser/module_tree.rs` (`ModuleTree::add_module`)
    *   `crates/ingest/syn_parser/src/parser/graph.rs` (`CodeGraph::build_module_tree`)
*   **Ignored Tests (Due to this limitation preventing `ModuleTree` build):**
    *   `test_spp_cfg_exclusive_a`
    *   `test_spp_cfg_exclusive_not_a`
    *   `test_spp_cfg_nested_exclusive_ab`
    *   `test_spp_cfg_nested_exclusive_nac`
    *   `test_spp_cfg_conflicting`
    *   *(Note: Other tests in `edge_cases.rs` might be ignored for different reasons related to SPP implementation status, but these specific tests are blocked by the tree build failure when using the original `fixture_spp_edge_cases`)*

---

*(Add subsequent limitations below this line)*
