//! Tests focusing on edge cases for `shortest_public_path` (SPP) logic,
//! using the `fixture_spp_edge_cases` crate.
//!
//! # Test Strategy Connection:
//! These tests primarily fall under **Tier 3 (Canary Tests)** of the Phase 3
//! testing strategy. They use a complex fixture to perform detailed checks on
//! specific items in challenging scenarios. Failures here indicate potential
//! regressions or unhandled edge cases in the SPP implementation or the
//! underlying `ModuleTree` structure. Some tests also touch upon **Tier 4
//! (Error Handling)** by verifying expected `Err` results for non-public items
//! or specific error conditions like external re-exports.
//!
//! # Test Scenarios (Based on `fixture_spp_edge_cases/src/lib.rs`):
//!
//! 1.  **Multi-Step Re-export Chain (3-step):** Test SPP for `item_c`.
//! 2.  **Multi-Step Re-export Chain (4-step):** Test SPP for `item_alt_d`, ensure shortest path selected.
//! 3.  **Inline Module `#[path]` Shadowing:** Test SPP for `inline_path_mod::shadow_me`.
//! 4.  **Inline Module `#[path]` Item Access:** Test SPP for `inline_path_mod::item_only_in_inline_target`.
//! 5.  **One File → Multiple Logical Modules (Public):** Test SPP for `logical_mod_1::item_in_shared_target`.
//! 6.  **One File → Multiple Logical Modules (Crate):** Test SPP for `logical_mod_1::crate_item_in_shared_target`.
//! 7.  **Glob Re-export (Public Item):** Test SPP for `glob_public_item` (accessed at root).
//! 8.  **Glob Re-export (Item in `#[path]` Submodule):** Test SPP for `glob_sub_path::item_in_glob_sub_path`.
//! 9.  **Glob Re-export (Item in Public Submodule):** Test SPP for `pub_sub_with_restricted::public_item_here`.
//! 10. **Glob Re-export (Restricted Item):** Test SPP for `pub_sub_with_restricted::super_visible_item`.
//! 11. **Restricted Visibility Item (`pub(crate)`):** Test SPP for `restricted_vis::crate_func`.
//! 12. **Restricted Visibility Item (`pub(super)`):** Test SPP for `restricted_vis::super_func`.
//! 13. **Restricted Visibility Item (`pub(in path)`):** Test SPP for `restricted_vis::inner::in_path_func`.
//! 14. **Shadowing (Local Definition):** Test SPP for `shadowing::shadowed_item`.
//! 15. **Relative Re-export (`super`):** Test SPP for `relative::inner::reexport_super`.
//! 16. **Relative Re-export (`self`):** Test SPP for `relative::reexport_self`.
//! 17. **Deep Re-export Chain:** Test SPP for `final_deep_item`.
//! 18. **Branching/Converging Re-export:** Test SPP for `item_via_a` or `item_via_b`.
//! 19. **Multiple Renames in Chain:** Test SPP for `final_renamed_item`.
//! 20. **Nested `#[path]` (Level 1 Item):** Test SPP for `nested_path_1::item_in_nested_target_1`.
//! 21. **Nested `#[path]` (Level 2 Item):** Test SPP for `nested_path_1::nested_target_2::item_in_nested_target_2`.
//! 22. **Mutually Exclusive `cfg` (Branch A):** Test SPP for `cfg_mod::item_in_cfg_a`.
//! 23. **Mutually Exclusive `cfg` (Branch Not A):** Test SPP for `cfg_mod::item_in_cfg_not_a`.
//! 24. **Nested Mutually Exclusive `cfg` (Branch AB):** Test SPP for `cfg_mod::nested_cfg::item_in_cfg_ab`.
//! 25. **Nested Mutually Exclusive `cfg` (Branch NotA C):** Test SPP for `cfg_mod::nested_cfg::item_in_cfg_nac`.
//! 26. **Conflicting Parent/Child `cfg`:** Test SPP for `conflict_parent::conflict_child::impossible_item`.
//!
//! ---
//!
//! ## Testing Philosophy & Handling Failures (Restated from Strategy)
//!
//! *   **No Tests Confirming Undesired States:** Tests must assert the *desired* behavior. Tests for unimplemented features *must fail* initially.
//! *   **Handling Known Limitations:** If a failing test represents a feature deliberately deferred or a known limitation deemed acceptable *and* non-corrupting:
//!     1.  Document the limitation thoroughly in `docs/design/known_limitations/`, linking to the specific test(s).
//!     2.  Mark the test(s) with `#[ignore = "Reason for ignoring (e.g., Known Limitation: XYZ - See docs/...)"]`.
//!     3.  Add a comment within the test explaining the situation and linking to the documentation.
//!     4.  Consider creating an ADR (`docs/design/decision_tracking/`) proposing a future solution or formally accepting the limitation.
//! *   **Prioritize Graph Integrity:** Error handling tests must rigorously verify that scenarios potentially leading to inconsistent or invalid graph states result in errors that clearly signal "do not update database".
//!
//! ---

use ploke_core::NodeId;
use syn_parser::error::SynParserError;
use syn_parser::parser::module_tree::{ModuleTree, ModuleTreeError};
use syn_parser::CodeGraph;

use crate::common::resolution::{
    find_item_id_in_module_by_name, find_reexport_import_node_by_name,
};
use crate::common::uuid_ids_utils::run_phases_and_collect;

// Helper to build the tree for edge case tests
fn build_tree_for_edge_cases() -> (CodeGraph, ModuleTree) {
    let fixture_name = "fixture_spp_edge_cases"; // Use the dedicated fixture
    let results = run_phases_and_collect(fixture_name);
    let mut graphs: Vec<CodeGraph> = Vec::new();
    for parsed_graph in results {
        graphs.push(parsed_graph.graph);
    }
    let merged_graph = CodeGraph::merge_new(graphs).expect("Failed to merge graphs");
    let tree = merged_graph
        .build_module_tree()
        .expect("Failed to build module tree for edge cases fixture");
    (merged_graph, tree)
}

// --- Test Placeholders ---

// 1. Multi-Step Re-export Chain (3-step)
//    Target: `item_c` (re-export of `chain_a::item_a`)
//    Expected: Ok(["crate"])
//    Anticipated Status: FAIL (SPP doesn't handle re-exports yet)
//    Note: It's desirable for this test to fail until SPP is implemented correctly.

// 2. Multi-Step Re-export Chain (4-step) & Shortest Path
//    Target: `item_alt_d` (re-export of `chain_a::item_a`)
//    Expected: Ok(["crate"]) (SPP should find the shorter path via `item_c`)
//    Anticipated Status: FAIL (SPP doesn't handle re-exports or shortest path selection yet)
//    Note: It's desirable for this test to fail until SPP is implemented correctly.

// 3. Inline Module `#[path]` Shadowing
//    Target: `shadow_me` inside `inline_path_mod`
//    Expected: Ok(["crate", "inline_path_mod"])
//    Anticipated Status: PASS (SPP should find items in their direct containing module)

// 4. Inline Module `#[path]` Item Access
//    Target: `item_only_in_inline_target` (defined in target file)
//    Expected: Ok(["crate", "inline_path_mod"])
//    Anticipated Status: PASS (SPP should find items in their direct containing module, which is the inline mod here)

// 5. One File → Multiple Logical Modules (Public)
//    Target: `item_in_shared_target` (defined in shared_target.rs)
//    Expected: Ok(["crate", "logical_mod_1"])
//    Anticipated Status: PASS (SPP should find the item via the public logical module path)

// 6. One File → Multiple Logical Modules (Crate)
//    Target: `crate_item_in_shared_target` (defined in shared_target.rs)
//    Expected: Err(ItemNotPubliclyAccessible)
//    Anticipated Status: PASS (Item is pub(crate))

// 7. Glob Re-export (Public Item)
//    Target: `glob_public_item` (defined in glob_target, re-exported at root)
//    Expected: Ok(["crate"])
//    Anticipated Status: FAIL (SPP doesn't handle glob re-exports yet)
//    Note: It's desirable for this test to fail until SPP is implemented correctly.

// 8. Glob Re-export (Item in `#[path]` Submodule)
//    Target: `item_in_glob_sub_path` (defined in glob_target/sub_path.rs)
//    Expected: Ok(["crate", "glob_sub_path"])
//    Anticipated Status: FAIL (SPP doesn't handle glob re-exports yet)
//    Note: It's desirable for this test to fail until SPP is implemented correctly.

// 9. Glob Re-export (Item in Public Submodule)
//    Target: `public_item_here` (defined in glob_target::pub_sub_with_restricted)
//    Expected: Ok(["crate", "pub_sub_with_restricted"])
//    Anticipated Status: FAIL (SPP doesn't handle glob re-exports yet)
//    Note: It's desirable for this test to fail until SPP is implemented correctly.

// 10. Glob Re-export (Restricted Item)
//     Target: `super_visible_item` (defined as pub(super) in glob_target::pub_sub_with_restricted)
//     Expected: Err(ItemNotPubliclyAccessible)
//     Anticipated Status: PASS (Item has restricted visibility, glob doesn't elevate it)

// 11. Restricted Visibility Item (`pub(crate)`)
//     Target: `crate_func` (defined in restricted_vis)
//     Expected: Err(ItemNotPubliclyAccessible)
//     Anticipated Status: PASS (Item is pub(crate))

// 12. Restricted Visibility Item (`pub(super)`)
//     Target: `super_func` (defined in restricted_vis)
//     Expected: Err(ItemNotPubliclyAccessible)
//     Anticipated Status: PASS (Item is pub(super) relative to crate)

// 13. Restricted Visibility Item (`pub(in path)`)
//     Target: `in_path_func` (defined in restricted_vis::inner)
//     Expected: Err(ItemNotPubliclyAccessible)
//     Anticipated Status: PASS (Item is pub(in restricted_vis))

// 14. Shadowing (Local Definition)
//     Target: `shadowed_item` (defined locally in shadowing module)
//     Expected: Ok(["crate", "shadowing"])
//     Anticipated Status: PASS (SPP finds the local definition)

// 15. Relative Re-export (`super`)
//     Target: `reexport_super` (re-export of `item_in_relative` inside `relative::inner`)
//     Expected: Ok(["crate", "relative", "inner"])
//     Anticipated Status: FAIL (SPP doesn't handle re-exports yet)
//     Note: It's desirable for this test to fail until SPP is implemented correctly.

// 16. Relative Re-export (`self`)
//     Target: `reexport_self` (re-export of `item_in_inner` inside `relative`)
//     Expected: Ok(["crate", "relative"])
//     Anticipated Status: FAIL (SPP doesn't handle re-exports yet)
//     Note: It's desirable for this test to fail until SPP is implemented correctly.

// 17. Deep Re-export Chain
//     Target: `final_deep_item` (11-step re-export)
//     Expected: Ok(["crate"])
//     Anticipated Status: FAIL (SPP doesn't handle re-exports yet)
//     Note: It's desirable for this test to fail until SPP is implemented correctly.

// 18. Branching/Converging Re-export
//     Target: `item_via_a` or `item_via_b` (re-exports of `branch_item`)
//     Expected: Ok(["crate"])
//     Anticipated Status: FAIL (SPP doesn't handle re-exports or shortest path selection yet)
//     Note: It's desirable for this test to fail until SPP is implemented correctly.

// 19. Multiple Renames in Chain
//     Target: `final_renamed_item` (re-export of `multi_rename_item`)
//     Expected: Ok(["crate"])
//     Anticipated Status: FAIL (SPP doesn't handle re-exports or renaming yet)
//     Note: It's desirable for this test to fail until SPP is implemented correctly.

// 20. Nested `#[path]` (Level 1 Item)
//     Target: `item_in_nested_target_1` (defined in nested_path_target_1.rs)
//     Expected: Ok(["crate", "nested_path_1"])
//     Anticipated Status: PASS (SPP finds item in its direct module)

// 21. Nested `#[path]` (Level 2 Item)
//     Target: `item_in_nested_target_2` (defined in nested_path_target_2.rs)
//     Expected: Ok(["crate", "nested_path_1", "nested_target_2"])
//     Anticipated Status: PASS (SPP finds item in its direct module)

// 22. Mutually Exclusive `cfg` (Branch A)
//     Target: `item_in_cfg_a` (defined in `#[cfg(feature = "cfg_a")] cfg_mod`)
//     Expected: Ok(["crate", "cfg_mod"])
//     Anticipated Status: PASS (SPP finds syntactic path, ignoring cfg evaluation)

// 23. Mutually Exclusive `cfg` (Branch Not A)
//     Target: `item_in_cfg_not_a` (defined in `#[cfg(not(feature = "cfg_a"))] cfg_mod`)
//     Expected: Ok(["crate", "cfg_mod"])
//     Anticipated Status: PASS (SPP finds syntactic path, ignoring cfg evaluation)

// 24. Nested Mutually Exclusive `cfg` (Branch AB)
//     Target: `item_in_cfg_ab` (defined in `#[cfg(a)] cfg_mod { #[cfg(b)] nested_cfg }`)
//     Expected: Ok(["crate", "cfg_mod", "nested_cfg"])
//     Anticipated Status: PASS (SPP finds syntactic path, ignoring cfg evaluation)

// 25. Nested Mutually Exclusive `cfg` (Branch NotA C)
//     Target: `item_in_cfg_nac` (defined in `#[cfg(not a)] cfg_mod { #[cfg(c)] nested_cfg }`)
//     Expected: Ok(["crate", "cfg_mod", "nested_cfg"])
//     Anticipated Status: PASS (SPP finds syntactic path, ignoring cfg evaluation)

// 26. Conflicting Parent/Child `cfg`
//     Target: `impossible_item` (defined in `#[cfg(conflict)] parent { #[cfg(not conflict)] child }`)
//     Expected: Ok(["crate", "conflict_parent", "conflict_child"])
//     Anticipated Status: PASS (SPP finds syntactic path, ignoring cfg impossibility)
