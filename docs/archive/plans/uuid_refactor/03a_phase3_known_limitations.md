# Phase 3 (`ModuleTree` & Resolution) Known Limitations

This document tracks known limitations, missing features, or areas where the Phase 3 logic (`ModuleTree` construction, path resolution, visibility checks) deviates from complete Rust semantics or desired behavior. These limitations were primarily discovered during testing and are documented here to inform future development and prevent regressions.

---

## 1. (SOLVED) `ModuleTree` Construction Fails on Duplicate Paths from `cfg`

Solved Jul 26, 2025

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

## 2. Limited granularity of pruned `ParsedCodeGraph` validation

*   **Tracking** 
    *   Documented: commit 68487bbdc09ac165a4224735f60f9f36c71dc496 (HEAD -> exp/debug-parse-self)

*   **Limitation:** The secondary nodes `Variant`, `Field`, `Param`, `GenericParam` are not explicitly verified for removal in the pruning step of the module tree, though they are implicitly removed since these items are sub-fields of primary nodes, e.g. `StructNode`.

*   **Patch Solution**: Filter out those secondary node ids from the pruned item list before comparing the counts of items to be pruned and pruned items. This allows the validation step to check for the explicitly removed items, but does not explicitly verify that those items have been removed (since they are fields within the removed primary nodes).

*   **Impact:**  
    • Add a small possibility of error, insofar as the count of items in the items to prune in the `PruningResult` field `pruned_item_ids` may not correctly match the actual items to be pruned. 
    • For example, supposing we missed adding a pruned `Variant` secondary node to the items to be pruned, this item's parent would still be pruned, but the mismatch would not be noticed.
    • If we added an item to be pruned from the list of `Variant`, `Field`, etc. but did not add the primary node parent, then the `Variant` would not be pruned and would slip in to the code graph erroneously.
    • NOTE: The lack of collision now means that the parser will not panic

*   **Future Work:**  

    • TODO: Add tests for the case of a field of a struct being discluded from the cfg items while the struct itself is included. I'm not clear on whether we are intending to track this right now or not.
    • When we add greater granularity to type resolution, we will need to be more aware of the counts of the included and pruned secondary items, and this limitation should be addressed at that time.

---

*(Add subsequent limitations below this line)*
