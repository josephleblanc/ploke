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
//! # Helper Functions Used:
//!
//! *   `build_tree_for_edge_cases()`: Local helper to parse the fixture and build the `ModuleTree`.
//! *   `find_item_id_by_path_name_kind_checked()`: Robust helper from `common::resolution` to find item `NodeId`s.
//! *   `find_reexport_import_node_by_name_checked()`: Robust helper from `common::resolution` to find re-export `ImportNode` `NodeId`s.
//! *   `ModuleTree::shortest_public_path()`: The function under test.
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

use ploke_core::ItemKind;
use syn_parser::discovery::CrateContext;
// Added ItemKind
use syn_parser::resolve::module_tree::{ModuleTree, ModuleTreeError};
use syn_parser::CodeGraph;

use crate::common::resolution::*;
use crate::common::uuid_ids_utils::run_phases_and_collect;

// Helper to build the tree for edge case tests
fn build_tree_for_edge_cases() -> (CodeGraph, ModuleTree) {
    // NOTE: cfg features being enabled currently remove all possibility of testing remaining items
    // due to presence of duplicates in test fixture `fixture_spp_edge_cases_no_cfg`. Development
    // on cfg-capable mod tree happening on git branch feature/mod_tree_cfg
    let fixture_name = "fixture_spp_edge_cases_no_cfg";
    let results = run_phases_and_collect(fixture_name);
    let mut contexts: Vec<CrateContext> = Vec::new();
    let mut graphs: Vec<CodeGraph> = Vec::new();
    for parsed_graph in results {
        graphs.push(parsed_graph.graph);
        if let Some(ctx) = parsed_graph.crate_context {
            // dirty, placeholder
            contexts.push(ctx);
        }
    }
    let merged_graph = CodeGraph::merge_new(graphs).expect("Failed to merge graphs");
    let tree = merged_graph
        .build_module_tree(contexts.first().unwrap().clone()) // dirty, placeholder
        .expect("Failed to build module tree for edge cases fixture");
    (merged_graph, tree)
}

// --- Tests ---

#[test]
fn test_spp_multi_step_3() {
    // 1. Multi-Step Re-export Chain (3-step)
    //    Target: `item_c` (re-export of `chain_a::item_a`)
    //    Expected: Ok(["crate"])
    //    Anticipated Status: FAIL (SPP doesn't handle re-exports yet)

    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .try_init();
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "chain_a"],
        "item_a",
        ItemKind::Function,
    )
    .expect("Failed to find original item_a");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for 3-step re-export 'item_c' failed"
    );
}

#[test]
fn test_spp_multi_step_4_shortest() {
    // 2. Multi-Step Re-export Chain (4-step) & Shortest Path
    //    Target: `item_alt_d` (re-export of `chain_a::item_a`)
    //    Expected: Ok(["crate"]) (SPP should find the shorter path via `item_c`)
    //    Anticipated Status: FAIL (SPP doesn't handle re-exports or shortest path selection yet)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "chain_a"],
        "item_a",
        ItemKind::Function,
    )
    .expect("Failed to find original item_a");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    // Expected is the path via item_c (length 1), not item_alt_d (length 1)
    let expected_result = Ok(vec!["crate".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item 'item_a' re-exported via 'item_alt_d' should select shorter path via 'item_c'"
    );
}

#[test]
fn test_spp_inline_path_shadowing() {
    // 3. Inline Module `#[path]` Shadowing
    //    Target: `shadow_me` inside `inline_path_mod`
    //    Expected: Ok(["crate", "inline_path_mod"])
    //    Anticipated Status: PASS (SPP should find items in their direct containing module)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "inline_path_mod"], // Definition path is the inline module
        "shadow_me",
        ItemKind::Function,
    )
    .expect("Failed to find inline shadow_me");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string(), "inline_path_mod".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for shadowed item in inline #[path] module failed"
    );
}

#[test]
fn test_spp_inline_path_item_access() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .try_init();
    // 4. Inline Module `#[path]` Item Access
    //    Target: `item_only_in_inline_target` (defined in target file)
    //    Expected: Ok(["crate", "inline_path_mod"])
    //    Anticipated Status: PASS (SPP should find items in their direct containing module, which
    //    is the inline mod here)
    let (graph, tree) = build_tree_for_edge_cases();
    // Find the item in the *file* module node where it's defined syntactically
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "inline_path_mod"], // The item is contained by the inline module node
        "item_only_in_inline_target",
        ItemKind::Function,
    )
    .expect("Failed to find item_only_in_inline_target");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string(), "inline_path_mod".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item defined in #[path] target file accessed via inline module failed"
    );
    Ok(())
}

#[test]
fn test_spp_one_file_multi_mod_public() {
    // 5. One File → Multiple Logical Modules (Public)
    //    Target: `item_in_shared_target` (defined in shared_target.rs)
    //    Expected: Ok(["crate", "logical_mod_1"])
    //    Anticipated Status: PASS (SPP should find the item via the public logical module path)
    let (graph, tree) = build_tree_for_edge_cases();
    // Find the item in the file-based module node
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "logical_mod_1"], // Item is contained by the file node linked to logical_mod_1
        "item_in_shared_target",
        ItemKind::Function,
    )
    .expect("Failed to find item_in_shared_target");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string(), "logical_mod_1".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item in file shared by multiple #[path] modules failed"
    );
}

#[test]
fn test_spp_one_file_multi_mod_crate() {
    // 6. One File → Multiple Logical Modules (Crate)
    //    Target: `crate_item_in_shared_target` (defined in shared_target.rs)
    //    Expected: Err(ItemNotPubliclyAccessible)
    //    Anticipated Status: PASS (Item is pub(crate))
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "logical_mod_1"], // Item is contained by the file node linked to logical_mod_1
        "crate_item_in_shared_target",
        ItemKind::Function,
    )
    .expect("Failed to find crate_item_in_shared_target");

    let spp_result = tree.shortest_public_path(item_id, &graph);

    assert!(
        matches!(spp_result, Err(ModuleTreeError::ItemNotPubliclyAccessible(id)) if id == item_id),
        "SPP for crate item in shared file should be Err, but was {:?}",
        spp_result
    );
}

#[test]
fn test_spp_glob_reexport_public() {
    // 7. Glob Re-export (Public Item)
    //    Target: `glob_public_item` (defined in glob_target, re-exported at root)
    //    Expected: Ok(["crate"])
    //    Anticipated Status: FAIL (SPP doesn't handle glob re-exports yet)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "glob_target"],
        "glob_public_item",
        ItemKind::Function,
    )
    .expect("Failed to find glob_public_item");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for public item via glob re-export failed"
    );
}

#[test]
fn test_spp_glob_reexport_path_submodule() {
    // 8. Glob Re-export (Item in `#[path]` Submodule)
    //    Target: `item_in_glob_sub_path` (defined in glob_target/sub_path.rs)
    //    Expected: Ok(["crate", "glob_sub_path"])
    //    Anticipated Status: FAIL (SPP doesn't handle glob re-exports yet)
    let (graph, tree) = build_tree_for_edge_cases();
    // Item defined in file linked by #[path] to glob_target::glob_sub_path
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "glob_target", "glob_sub_path"], // Definition path
        "item_in_glob_sub_path",
        ItemKind::Function,
    )
    .expect("Failed to find item_in_glob_sub_path");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string(), "glob_sub_path".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item in #[path] submodule via glob re-export failed"
    );
}

#[test]
fn test_spp_glob_reexport_public_submodule() {
    // 9. Glob Re-export (Item in Public Submodule)
    //    Target: `public_item_here` (defined in glob_target::pub_sub_with_restricted)
    //    Expected: Ok(["crate", "pub_sub_with_restricted"])
    //    Anticipated Status: FAIL (SPP doesn't handle glob re-exports yet)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "glob_target", "pub_sub_with_restricted"],
        "public_item_here",
        ItemKind::Function,
    )
    .expect("Failed to find public_item_here");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec![
        "crate".to_string(),
        "pub_sub_with_restricted".to_string(),
    ]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item in public submodule via glob re-export failed"
    );
}

#[test]
fn test_spp_glob_reexport_restricted() {
    // 10. Glob Re-export (Restricted Item)
    //     Target: `super_visible_item` (defined as pub(super) in glob_target::pub_sub_with_restricted)
    //     Expected: Err(ItemNotPubliclyAccessible)
    //     Anticipated Status: PASS (Item has restricted visibility, glob doesn't elevate it)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "glob_target", "pub_sub_with_restricted"],
        "super_visible_item",
        ItemKind::Function,
    )
    .expect("Failed to find super_visible_item");

    let spp_result = tree.shortest_public_path(item_id, &graph);

    assert!(
        matches!(spp_result, Err(ModuleTreeError::ItemNotPubliclyAccessible(id)) if id == item_id),
        "SPP for restricted item via glob should be Err, but was {:?}",
        spp_result
    );
}

#[test]
fn test_spp_restricted_crate() {
    // 11. Restricted Visibility Item (`pub(crate)`)
    //     Target: `crate_func` (defined in restricted_vis)
    //     Expected: Err(ItemNotPubliclyAccessible)
    //     Anticipated Status: PASS (Item is pub(crate))
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "restricted_vis"],
        "crate_func",
        ItemKind::Function,
    )
    .expect("Failed to find crate_func");

    let spp_result = tree.shortest_public_path(item_id, &graph);

    assert!(
        matches!(spp_result, Err(ModuleTreeError::ItemNotPubliclyAccessible(id)) if id == item_id),
        "SPP for pub(crate) item should be Err, but was {:?}",
        spp_result
    );
}

#[test]
fn test_spp_restricted_super() {
    // 12. Restricted Visibility Item (`pub(super)`)
    //     Target: `super_func` (defined in restricted_vis)
    //     Expected: Err(ItemNotPubliclyAccessible)
    //     Anticipated Status: PASS (Item is pub(super) relative to crate)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "restricted_vis"],
        "super_func",
        ItemKind::Function,
    )
    .expect("Failed to find super_func");

    let spp_result = tree.shortest_public_path(item_id, &graph);

    assert!(
        matches!(spp_result, Err(ModuleTreeError::ItemNotPubliclyAccessible(id)) if id == item_id),
        "SPP for pub(super) item should be Err, but was {:?}",
        spp_result
    );
}

#[test]
fn test_spp_restricted_in_path() {
    // 13. Restricted Visibility Item (`pub(in path)`)
    //     Target: `in_path_func` (defined in restricted_vis::inner)
    //     Expected: Err(ItemNotPubliclyAccessible)
    //     Anticipated Status: PASS (Item is pub(in restricted_vis))
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "restricted_vis", "inner"],
        "in_path_func",
        ItemKind::Function,
    )
    .expect("Failed to find in_path_func");

    let spp_result = tree.shortest_public_path(item_id, &graph);

    assert!(
        matches!(spp_result, Err(ModuleTreeError::ItemNotPubliclyAccessible(id)) if id == item_id),
        "SPP for pub(in path) item should be Err, but was {:?}",
        spp_result
    );
}

#[test]
fn test_spp_shadowing_local() {
    // 14. Shadowing (Local Definition)
    //     Target: `shadowed_item` (defined locally in shadowing module)
    //     Expected: Ok(["crate", "shadowing"])
    //     Anticipated Status: PASS (SPP finds the local definition)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "shadowing"],
        "shadowed_item",
        ItemKind::Function,
    )
    .expect("Failed to find local shadowed_item");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string(), "shadowing".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for locally defined shadowed item failed"
    );
}

#[test]
fn test_spp_relative_reexport_super() {
    // 15. Relative Re-export (`super`)
    //     Target: `reexport_super` (re-export of `item_in_relative` inside `relative::inner`)
    //     Expected: Ok(["crate", "relative", "inner"])
    //     Anticipated Status: FAIL (SPP doesn't handle re-exports yet)
    let (graph, tree) = build_tree_for_edge_cases();
    // Find the original item
    let original_item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "relative"],
        "item_in_relative",
        ItemKind::Function,
    )
    .expect("Failed to find item_in_relative");

    let spp_result = tree.shortest_public_path(original_item_id, &graph);
    let expected_result = Ok(vec![
        "crate".to_string(),
        "relative".to_string(),
        "inner".to_string(),
    ]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item re-exported via 'super' failed"
    );
}

#[test]
fn test_spp_relative_reexport_self() {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .try_init();
    // 16. Relative Re-export (`self`)
    //     Target: `reexport_self` (re-export of `item_in_inner` inside `relative`)
    //     Expected: Ok(["crate", "relative"])
    //     Anticipated Status: FAIL (SPP doesn't handle re-exports yet)
    let (graph, tree) = build_tree_for_edge_cases();
    // Find the original item
    let original_item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "relative", "inner"],
        "item_in_inner",
        ItemKind::Function,
    )
    .expect("Failed to find item_in_inner");

    let spp_result = tree.shortest_public_path(original_item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string(), "relative".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item re-exported via 'self' failed"
    );
}

#[test]
fn test_spp_deep_reexport_chain() {
    // 17. Deep Re-export Chain
    //     Target: `final_deep_item` (11-step re-export)
    //     Expected: Ok(["crate"])
    //     Anticipated Status: FAIL (SPP doesn't handle re-exports yet)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "deep1"],
        "deep_item",
        ItemKind::Function,
    )
    .expect("Failed to find deep_item");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for deep re-export chain failed"
    );
}

#[test]
fn test_spp_branching_reexport() {
    // 18. Branching/Converging Re-export
    //     Target: `item_via_a` or `item_via_b` (re-exports of `branch_item`)
    //     Expected: Ok(["crate"])
    //     Anticipated Status: FAIL (SPP doesn't handle re-exports or shortest path selection yet)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "branch_source"],
        "branch_item",
        ItemKind::Function,
    )
    .expect("Failed to find branch_item");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string()]); // Expect shortest path

    assert_eq!(
        spp_result, expected_result,
        "SPP for branching re-export failed"
    );
}

#[test]
fn test_spp_multiple_renames() {
    // 19. Multiple Renames in Chain
    //     Target: `final_renamed_item` (re-export of `multi_rename_item`)
    //     Expected: Ok(["crate"])
    //     Anticipated Status: FAIL (SPP doesn't handle re-exports or renaming yet)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "rename_source"],
        "multi_rename_item",
        ItemKind::Function,
    )
    .expect("Failed to find multi_rename_item");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for multi-rename re-export chain failed"
    );
}

#[test]
fn test_spp_nested_path_level1() {
    // 20. Nested `#[path]` (Level 1 Item)
    //     Target: `item_in_nested_target_1` (defined in nested_path_target_1.rs)
    //     Expected: Ok(["crate", "nested_path_1"])
    //     Anticipated Status: PASS (SPP finds item in its direct module)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "nested_path_1"], // Definition path
        "item_in_nested_target_1",
        ItemKind::Function,
    )
    .expect("Failed to find item_in_nested_target_1");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string(), "nested_path_1".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item in level 1 nested #[path] module failed"
    );
}

#[test]
fn test_spp_nested_path_level2() {
    // 21. Nested `#[path]` (Level 2 Item)
    //     Target: `item_in_nested_target_2` (defined in nested_path_target_2.rs)
    //     Expected: Ok(["crate", "nested_path_1", "nested_target_2"])
    //     Anticipated Status: PASS (SPP finds item in its direct module)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "nested_path_1", "nested_target_2"], // Definition path
        "item_in_nested_target_2",
        ItemKind::Function,
    )
    .expect("Failed to find item_in_nested_target_2");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec![
        "crate".to_string(),
        "nested_path_1".to_string(),
        "nested_target_2".to_string(),
    ]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item in level 2 nested #[path] module failed"
    );
}

#[test]
#[ignore = "Known Limitation P3-00: ModuleTree construction fails on duplicate paths from cfg. See docs/design/known_limitations/P3-00-cfg-duplication.md"]
fn test_spp_cfg_exclusive_a() {
    // 22. Mutually Exclusive `cfg` (Branch A)
    //     Target: `item_in_cfg_a` (defined in `#[cfg(feature = "cfg_a")] cfg_mod`)
    //     Ignored because ModuleTree build fails with fixture_spp_edge_cases due to P3-00.
    //     Expected: Ok(["crate", "cfg_mod"])
    //     Anticipated Status: PASS (SPP finds syntactic path, ignoring cfg evaluation)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "cfg_mod"], // Definition path (same for both cfg branches syntactically)
        "item_in_cfg_a",
        ItemKind::Function,
    )
    .expect("Failed to find item_in_cfg_a");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string(), "cfg_mod".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item in cfg(a) branch failed"
    );
}

#[test]
#[ignore = "Known Limitation P3-00: ModuleTree construction fails on duplicate paths from cfg. See docs/design/known_limitations/P3-00-cfg-duplication.md"]
fn test_spp_cfg_exclusive_not_a() {
    // 23. Mutually Exclusive `cfg` (Branch Not A)
    //     Target: `item_in_cfg_not_a` (defined in `#[cfg(not(feature = "cfg_a"))] cfg_mod`)
    //     Ignored because ModuleTree build fails with fixture_spp_edge_cases due to P3-00.
    //     Expected: Ok(["crate", "cfg_mod"])
    //     Anticipated Status: PASS (SPP finds syntactic path, ignoring cfg evaluation)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "cfg_mod"], // Definition path
        "item_in_cfg_not_a",
        ItemKind::Function,
    )
    .expect("Failed to find item_in_cfg_not_a");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec!["crate".to_string(), "cfg_mod".to_string()]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item in cfg(not a) branch failed"
    );
}

#[test]
#[ignore = "Known Limitation P3-00: ModuleTree construction fails on duplicate paths from cfg. See docs/design/known_limitations/P3-00-cfg-duplication.md"]
fn test_spp_cfg_nested_exclusive_ab() {
    // 24. Nested Mutually Exclusive `cfg` (Branch AB)
    //     Target: `item_in_cfg_ab` (defined in `#[cfg(a)] cfg_mod { #[cfg(b)] nested_cfg }`)
    //     Ignored because ModuleTree build fails with fixture_spp_edge_cases due to P3-00.
    //     Expected: Ok(["crate", "cfg_mod", "nested_cfg"])
    //     Anticipated Status: PASS (SPP finds syntactic path, ignoring cfg evaluation)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "cfg_mod", "nested_cfg"], // Definition path
        "item_in_cfg_ab",
        ItemKind::Function,
    )
    .expect("Failed to find item_in_cfg_ab");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec![
        "crate".to_string(),
        "cfg_mod".to_string(),
        "nested_cfg".to_string(),
    ]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item in nested cfg(a)/cfg(b) branch failed"
    );
}

#[test]
#[ignore = "Known Limitation P3-00: ModuleTree construction fails on duplicate paths from cfg. See docs/design/known_limitations/P3-00-cfg-duplication.md"]
fn test_spp_cfg_nested_exclusive_nac() {
    // 25. Nested Mutually Exclusive `cfg` (Branch NotA C)
    //     Target: `item_in_cfg_nac` (defined in `#[cfg(not a)] cfg_mod { #[cfg(c)] nested_cfg }`)
    //     Ignored because ModuleTree build fails with fixture_spp_edge_cases due to P3-00.
    //     Expected: Ok(["crate", "cfg_mod", "nested_cfg"])
    //     Anticipated Status: PASS (SPP finds syntactic path, ignoring cfg evaluation)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "cfg_mod", "nested_cfg"], // Definition path
        "item_in_cfg_nac",
        ItemKind::Function,
    )
    .expect("Failed to find item_in_cfg_nac");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec![
        "crate".to_string(),
        "cfg_mod".to_string(),
        "nested_cfg".to_string(),
    ]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item in nested cfg(not a)/cfg(c) branch failed"
    );
}

#[test]
#[ignore = "Known Limitation P3-00: ModuleTree construction fails on duplicate paths from cfg. See docs/design/known_limitations/P3-00-cfg-duplication.md"]
fn test_spp_cfg_conflicting() {
    // 26. Conflicting Parent/Child `cfg`
    //     Target: `impossible_item` (defined in `#[cfg(conflict)] parent { #[cfg(not conflict)] child }`)
    //     Ignored because ModuleTree build fails with fixture_spp_edge_cases due to P3-00.
    //     Expected: Ok(["crate", "conflict_parent", "conflict_child"])
    //     Anticipated Status: PASS (SPP finds syntactic path, ignoring cfg impossibility)
    let (graph, tree) = build_tree_for_edge_cases();
    let item_id = find_item_id_by_path_name_kind_checked(
        &graph,
        &["crate", "conflict_parent", "conflict_child"], // Definition path
        "impossible_item",
        ItemKind::Function,
    )
    .expect("Failed to find impossible_item");

    let spp_result = tree.shortest_public_path(item_id, &graph);
    let expected_result = Ok(vec![
        "crate".to_string(),
        "conflict_parent".to_string(),
        "conflict_child".to_string(),
    ]);

    assert_eq!(
        spp_result, expected_result,
        "SPP for item under conflicting cfgs failed (expected syntactic path)"
    );
}
