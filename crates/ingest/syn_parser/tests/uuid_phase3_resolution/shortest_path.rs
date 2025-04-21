//! Tests focusing on the `shortest_public_path` logic in `ModuleTree`.
//!
//! # Current Implementation Status (as of 2025-04-21):
//! The `shortest_public_path` function currently performs a BFS from the crate root,
//! considering only direct module containment (`Contains` relations) and basic
//! public visibility (`VisibilityKind::Public`). It finds the path to the module
//! containing the item's *original definition*.
//!
//! # `fixture_path_resolution` Coverage & Expected Behavior:
//!
//! ## Scenarios Handled Correctly (Expected Path = Path to Definition Module):
//! *   Direct public items in root (`root_func`, `RootStruct`, `RootError`).
//! *   Direct public items in public file modules (`local_mod::local_func`).
//! *   Direct public items in nested public file modules (`local_mod::nested::deep_func`).
//! *   Direct public items in public inline modules (`inline_mod::inline_func`).
//! *   Items in modules defined via `#[path]` (`logical_path_mod::item_in_actual_file`).
//! *   Items in modules defined via `#[path]` outside `src/` (`common_import_mod::function_in_common_file`).
//! *   Generic items (`generics::GenStruct`, `generics::GenTrait`, `generics::gen_func`).
//! *   Macros defined at root (`simple_macro!`).
//! *   Items under `#[cfg]` attributes (assuming the feature flags are set such that the item exists in the graph).
//!
//! ## Scenarios NOT Handled Correctly (Re-exports):
//! The current implementation will return the path to the *original definition module*,
//! not the shorter path created by the re-export.
//! *   `pub use local_mod::local_func;` at root (Current: `crate::local_mod`, Expected: `crate`)
//! *   `pub use local_mod::nested::deep_func;` at root (Current: `crate::local_mod::nested`, Expected: `crate`)
//! *   `pub use log::debug as log_debug_reexport;` at root (Current: Needs external resolution, Expected: `crate`)
//! *   `pub use local_mod::nested::deep_func as renamed_deep_func;` at root (Current: `crate::local_mod::nested`, Expected: `crate`)
//! *   `pub use local_mod::nested as reexported_nested_mod;` at root (Current: `crate::local_mod::nested`, Expected: `crate`)
//! *   `pub use logical_path_mod::item_in_actual_file;` at root (Current: `crate::logical_path_mod`, Expected: `crate`)
//! *   `pub use self::generics::GenStruct as PublicGenStruct;` at root (Current: `crate::generics`, Expected: `crate`)
//! *   `pub use self::generics::GenTrait as PublicGenTrait;` at root (Current: `crate::generics`, Expected: `crate`)
//! *   `pub use super::local_mod::nested::deep_func as deep_reexport_inline;` in `inline_mod` (Current: `crate::local_mod::nested`, Expected: `crate::inline_mod`)
//! *   `pub use super::local_func as parent_local_func_reexport;` in `local_mod::nested` (Current: `crate::local_mod`, Expected: `crate::local_mod::nested`)
//! *   `#[cfg(feature = "feature_b")] pub use crate::local_mod::local_func as pub_aliased_func_b;` at root (Current: `crate::local_mod`, Expected: `crate`, depends on cfg)
//!
//! ## Scenarios NOT Handled Correctly (Visibility & Access):
//! The current implementation only checks for `VisibilityKind::Public`. It needs enhancement
//! to correctly determine accessibility based on the *source* module and handle other visibilities.
//! It will likely return `Err(ItemNotPubliclyAccessible)` incorrectly for items that *should* be accessible
//! via non-public paths (e.g., crate-visible items accessed from within the crate).
//! *   Items in `pub(crate)` modules (`crate_mod::crate_internal_func`).
//! *   Items in `pub(super)` modules (`local_mod::super_visible_func_in_local`).
//! *   Items in `pub(in path)` modules (`restricted_vis_mod::restricted_func`).
//! *   `pub(crate)` items within `#[path]` targets (`logical_path_mod::crate_visible_in_actual_file`).
//!
//! ## Scenarios Returning `Err(ItemNotPubliclyAccessible)` (Correctly):
//! *   Items explicitly marked private (`local_mod::private_local_func`, `local_mod::nested::private_deep_func`).
//! *   Items inside private modules (`private_inline_mod::private_inline_func`, `private_inline_mod::pub_in_private_inline`).
//!
//! # TODO:
//! 1.  Enhance `shortest_public_path` to correctly handle `pub use` re-exports, finding the truly shortest path.
//! 2.  Enhance `shortest_public_path` (or related visibility logic) to handle `pub(crate)`, `pub(super)`, and `pub(in path)`.
//! 3.  Add tests specifically targeting the `fixture_path_resolution` crate for the scenarios listed above.

use syn_parser::parser::module_tree::{ModuleTree, ModuleTreeError};
// Removed unused import: use syn_parser::parser::nodes::ModuleNodeId;
use syn_parser::CodeGraph;

use crate::common::uuid_ids_utils::run_phases_and_collect;

// Helper to build the tree for tests
fn build_tree_for_fixture(fixture_name: &str) -> (CodeGraph, ModuleTree) {
    let results = run_phases_and_collect(fixture_name);
    let mut graphs: Vec<CodeGraph> = Vec::new();
    for parsed_graph in results {
        graphs.push(parsed_graph.graph);
    }
    let merged_graph = CodeGraph::merge_new(graphs).expect("Failed to merge graphs");
    let tree = merged_graph
        .build_module_tree()
        .expect("Failed to build module tree");
    (merged_graph, tree)
}

#[test]
fn test_spp_public_item_in_root() {
    let fixture_name = "file_dir_detection";
    let (graph, tree) = build_tree_for_fixture(fixture_name);

    // Find the public function `main_pub_func` in the crate root
    let main_pub_func_id = graph
        .functions
        .iter()
        .find(|f| f.name == "main_pub_func")
        .expect("Could not find main_pub_func")
        .id;

    // Find the crate root module ID
    // Calculate shortest public path starting from the crate root
    // Pass the graph as required by the new signature
    let spp = tree.shortest_public_path(main_pub_func_id, &graph);

    // Expected path: Ok(["crate"]) (path to the containing module)
    let expected_path = Ok(vec!["crate".to_string()]); // Path to the containing module

    assert_eq!(
        spp, expected_path,
        "Shortest public path for root public function should be Ok([\"crate\"])"
    );
}

#[test]
fn test_spp_public_item_in_public_mod() {
    let fixture_name = "file_dir_detection";
    let (graph, tree) = build_tree_for_fixture(fixture_name);

    // Find the public function `top_pub_func` in `top_pub_mod`
    let top_pub_func_id = graph
        .functions
        .iter()
        .find(|f| {
            f.name == "top_pub_func" && graph.get_item_module_path(f.id) == ["crate", "top_pub_mod"]
        })
        .expect("Could not find top_pub_func in top_pub_mod")
        .id;

    // Find the crate root module ID
    // Calculate shortest public path starting from the crate root
    // Pass the graph as required by the new signature
    let spp = tree.shortest_public_path(top_pub_func_id, &graph);

    // Expected path: Ok(["crate", "top_pub_mod"])
    let expected_path = Ok(vec!["crate".to_string(), "top_pub_mod".to_string()]);

    assert_eq!(
        spp, expected_path,
        "Shortest public path for public function in public module should be Ok([\"crate\", \"top_pub_mod\"])"
    );
}

#[test]
fn test_spp_public_item_in_nested_public_mod() {
    let fixture_name = "file_dir_detection";
    let (graph, tree) = build_tree_for_fixture(fixture_name);

    // Find the public function `nested_pub_func` in `top_pub_mod::nested_pub`
    let nested_pub_func_id = graph
        .functions
        .iter()
        .find(|f| {
            f.name == "nested_pub_func"
                && graph.get_item_module_path(f.id) == ["crate", "top_pub_mod", "nested_pub"]
        })
        .expect("Could not find nested_pub_func in top_pub_mod::nested_pub")
        .id;

    // Find the crate root module ID
    // Calculate shortest public path starting from the crate root
    // Pass the graph as required by the new signature
    let spp = tree.shortest_public_path(nested_pub_func_id, &graph);

    // Expected path: Ok(["crate", "top_pub_mod", "nested_pub"])
    let expected_path = Ok(vec![
        "crate".to_string(),
        "top_pub_mod".to_string(),
        "nested_pub".to_string(),
    ]);

    assert_eq!(
        spp, expected_path,
        "Shortest public path for public function in nested public module should be Ok([\"crate\", \"top_pub_mod\", \"nested_pub\"])"
    );
}

#[test]
fn test_spp_private_item_in_public_mod() {
    let fixture_name = "file_dir_detection";
    let (graph, tree) = build_tree_for_fixture(fixture_name);

    // Find the private function `top_pub_priv_func` in `top_pub_mod`
    let top_pub_priv_func_id = graph
        .functions
        .iter()
        .find(|f| {
            f.name == "top_pub_priv_func"
                && graph.get_item_module_path(f.id) == ["crate", "top_pub_mod"]
        })
        .expect("Could not find top_pub_priv_func in top_pub_mod")
        .id;

    // Find the crate root module ID
    // Calculate shortest public path starting from the crate root
    // Pass the graph as required by the new signature
    let spp_result = tree.shortest_public_path(top_pub_priv_func_id, &graph);

    // Expected: Err(ItemNotPubliclyAccessible)
    assert!(
        matches!(spp_result, Err(ModuleTreeError::ItemNotPubliclyAccessible(id)) if id == top_pub_priv_func_id),
        "Shortest public path for private function should be Err(ItemNotPubliclyAccessible), but was {:?}", spp_result
    );
}

#[test]
fn test_spp_item_in_private_mod() {
    let fixture_name = "file_dir_detection";
    let (graph, tree) = build_tree_for_fixture(fixture_name);

    // Find the public function `nested_pub_func` in `top_priv_mod::nested_pub_in_priv`
    // Even though the function is pub, its containing module `top_priv_mod` is private.
    let nested_pub_in_priv_func_id = graph
        .functions
        .iter()
        .find(|f| {
            f.name == "nested_pub_func"
                && graph.get_item_module_path(f.id)
                    == ["crate", "top_priv_mod", "nested_pub_in_priv"]
        })
        .expect("Could not find nested_pub_func in top_priv_mod::nested_pub_in_priv")
        .id;

    // Find the crate root module ID
    // Calculate shortest public path starting from the crate root
    // Pass the graph as required by the new signature
    let spp_result = tree.shortest_public_path(nested_pub_in_priv_func_id, &graph);

    // Expected: Err(ItemNotPubliclyAccessible)
    assert!(
        matches!(spp_result, Err(ModuleTreeError::ItemNotPubliclyAccessible(id)) if id == nested_pub_in_priv_func_id),
        "Shortest public path for item in private module should be Err(ItemNotPubliclyAccessible), but was {:?}", spp_result
    );
}

// TODO: Add tests for re-exported items once the shortest_public_path implementation handles them.
// Example:
// #[test]
// fn test_spp_reexported_item() {
//     let fixture_name = "reexport_fixture"; // Need a fixture with re-exports
//     let (graph, tree) = build_tree_for_fixture(fixture_name);
//     // ... find original item ID and re-exporting module ID ...
//     let spp = tree.shortest_public_path(original_item_id, crate_root_id);
//     // Assert spp matches the shorter, re-exported path
// }

#[test]
fn test_spp_reexported_item_finds_original_path() {
    let fixture_name = "file_dir_detection";
    let (graph, tree) = build_tree_for_fixture(fixture_name);

    // Find the *original* public function `top_pub_func` in `top_pub_mod`.
    // This function is re-exported as `reexported_func` in the crate root.
    let original_func_id = graph
        .functions
        .iter()
        .find(|f| {
            f.name == "top_pub_func" && graph.get_item_module_path(f.id) == ["crate", "top_pub_mod"]
        })
        .expect("Could not find original top_pub_func in top_pub_mod")
        .id;

    // Calculate the shortest public path for the *original* function's ID.
    let spp = tree.shortest_public_path(original_func_id, &graph);

    // Expected path: Ok(["crate", "top_pub_mod"])
    // The current implementation finds the path to the module containing the
    // item's definition, it does not yet account for shorter paths via re-exports.
    let expected_path = Ok(vec!["crate".to_string(), "top_pub_mod".to_string()]);

    // NOTE: This assertion checks the *current* behavior.
    // Once shortest_public_path handles re-exports correctly, this test *should fail*,
    // and the expected_path should become Ok(vec!["crate".to_string()]).
    assert_eq!(
        spp, expected_path,
        "EXPECTED BEHAVIOR (PRE-REEXPORT): SPP for re-exported item resolves to original module path. This test WILL FAIL once re-exports are handled."
    );

    // We can also verify that the re-export itself exists as an ImportNode
    let reexport_node = graph.use_statements.iter().find(|imp| imp.visible_name == "reexported_func").expect("Could not find reexport ImportNode");
    assert!(reexport_node.is_reexport());
    assert_eq!(reexport_node.path, ["crate", "top_pub_mod", "top_pub_func"]); // Path points to original item
    // Check that the re-export is contained in the crate root module
    let crate_root_id = tree.root().into_inner();
    assert!(graph.module_contains_node(crate_root_id, reexport_node.id));

    // TODO: Enhance shortest_public_path to consider re-exports and potentially return ["crate"] for this item.
}
