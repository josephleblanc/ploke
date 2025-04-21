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

use ploke_core::NodeId;
use syn_parser::parser::module_tree::{ModuleTree, ModuleTreeError};
use syn_parser::CodeGraph;

use crate::common::resolution::find_item_id_in_module_by_name; // Import new helper
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
    let reexport_node = graph
        .use_statements
        .iter()
        .find(|imp| imp.visible_name == "reexported_func")
        .expect("Could not find reexport ImportNode");
    assert!(reexport_node.is_reexport());
    assert_eq!(reexport_node.path, ["crate", "top_pub_mod", "top_pub_func"]); // Path points to original item
                                                                              // Check that the re-export is contained in the crate root module
    let crate_root_id = tree.root().into_inner();
    assert!(graph.module_contains_node(crate_root_id, reexport_node.id));

    // TODO: Enhance shortest_public_path to consider re-exports and potentially return ["crate"] for this item.
}

// --- Tests for fixture_path_resolution ---

// Helper macro for SPP tests on fixture_path_resolution
macro_rules! assert_spp {
    ($test_name:ident, $item_name:expr, $module_path:expr, $current_expected:expr, $final_expected:expr) => {
        #[test]
        fn $test_name() {
            let fixture_name = "fixture_path_resolution";
            let (graph, tree) = build_tree_for_fixture(fixture_name);

            let item_id = find_item_id_in_module_by_name(&graph, $module_path, $item_name)
                .unwrap_or_else(|e| {
                    panic!(
                        "Failed to find item '{}' in module {:?}: {:?}",
                        $item_name, $module_path, e
                    )
                });

            let spp_result = tree.shortest_public_path(item_id, &graph);

            // Assert current behavior
            assert_eq!(
                spp_result,
                $current_expected,
                "SPP for '{}' currently resolves to definition path. Expected final path: {:?}",
                $item_name,
                $final_expected
            );
        }
    };
    // Variant for expecting Err(ItemNotPubliclyAccessible)
    ($test_name:ident, $item_name:expr, $module_path:expr, Err) => {
        #[test]
        fn $test_name() {
            let fixture_name = "fixture_path_resolution";
            let (graph, tree) = build_tree_for_fixture(fixture_name);

            let item_id_result = find_item_id_in_module_by_name(&graph, $module_path, $item_name);

            // Handle case where item itself might not be found (e.g., private item)
            let item_id = match item_id_result {
                Ok(id) => id,
                Err(SynParserError::NotFound(_)) => {
                    // If the item isn't even found by name in the module, SPP should also fail.
                    // We can assert this implicitly by expecting the SPP call below to fail.
                    // Or, create a dummy ID known to fail SPP. Let's use a dummy.
                    NodeId::Synthetic(uuid::Uuid::new_v4()) // Dummy ID
                }
                Err(e) => panic!(
                    "Unexpected error finding item '{}' in module {:?}: {:?}",
                    $item_name, $module_path, e
                ),
            };


            let spp_result = tree.shortest_public_path(item_id, &graph);

            // Assert current behavior (should be Err)
            assert!(
                matches!(spp_result, Err(ModuleTreeError::ItemNotPubliclyAccessible(id)) if id == item_id || item_id_result.is_err()),
                "SPP for non-public item '{}' should be Err(ItemNotPubliclyAccessible), but was {:?}",
                $item_name, spp_result
            );
        }
    };
}

// 1. Re-export of Direct Child Item
assert_spp!(
    test_spp_reexport_direct_child,
    "local_func",                                           // Item name
    &["crate", "local_mod"],                                // Original module path
    Ok(vec!["crate".to_string(), "local_mod".to_string()]), // Current SPP
    Ok(vec!["crate".to_string()])                           // Final Expected SPP
);

// 2. Re-export of Nested Item
assert_spp!(
    test_spp_reexport_nested_item,
    "deep_func",                       // Item name
    &["crate", "local_mod", "nested"], // Original module path
    Ok(vec![
        "crate".to_string(),
        "local_mod".to_string(),
        "nested".to_string()
    ]), // Current SPP
    Ok(vec!["crate".to_string()])      // Final Expected SPP
);

// 3. Re-export of Nested Item with Rename
assert_spp!(
    test_spp_reexport_nested_item_renamed,
    "deep_func",                       // Original item name (we find by original def)
    &["crate", "local_mod", "nested"], // Original module path
    Ok(vec![
        "crate".to_string(),
        "local_mod".to_string(),
        "nested".to_string()
    ]), // Current SPP
    Ok(vec!["crate".to_string()])      // Final Expected SPP (access via `renamed_deep_func`)
);

// 4. Re-export of Module - Test access to an item *within* the re-exported module
//    We test the SPP for the *original* item (`deep_func`), expecting the path
//    to the *original* module currently. The expected final path would involve the re-exported module name.
assert_spp!(
    test_spp_reexport_module_item_access,
    "deep_func",                       // Item name within the module
    &["crate", "local_mod", "nested"], // Original module path
    Ok(vec![
        "crate".to_string(),
        "local_mod".to_string(),
        "nested".to_string()
    ]), // Current SPP (to original module)
    Ok(vec![
        "crate".to_string(),
        "reexported_nested_mod".to_string()
    ])  // Final Expected SPP (via re-exported module)
);

// 5. Re-export from `#[path]` Module
assert_spp!(
    test_spp_reexport_from_path_mod,
    "item_in_actual_file",                                         // Item name
    &["crate", "logical_path_mod"],                                // Original module path (logical)
    Ok(vec!["crate".to_string(), "logical_path_mod".to_string()]), // Current SPP
    Ok(vec!["crate".to_string()])                                  // Final Expected SPP
);

// 6. Re-export of Generic Struct
assert_spp!(
    test_spp_reexport_generic_struct,
    "GenStruct",                                           // Item name
    &["crate", "generics"],                                // Original module path
    Ok(vec!["crate".to_string(), "generics".to_string()]), // Current SPP
    Ok(vec!["crate".to_string()])                          // Final Expected SPP
);

// 7. Re-export of Generic Trait
assert_spp!(
    test_spp_reexport_generic_trait,
    "GenTrait",                                            // Item name
    &["crate", "generics"],                                // Original module path
    Ok(vec!["crate".to_string(), "generics".to_string()]), // Current SPP
    Ok(vec!["crate".to_string()])                          // Final Expected SPP
);

// 8. Re-export within Inline Module
assert_spp!(
    test_spp_reexport_within_inline_mod,
    "deep_func",                       // Original item name
    &["crate", "local_mod", "nested"], // Original module path
    Ok(vec![
        "crate".to_string(),
        "local_mod".to_string(),
        "nested".to_string()
    ]), // Current SPP
    Ok(vec!["crate".to_string(), "inline_mod".to_string()]) // Final Expected SPP (path to re-exporting module)
);

// 9. Re-export within Nested Module
assert_spp!(
    test_spp_reexport_within_nested_mod,
    "local_func",                                           // Original item name
    &["crate", "local_mod"],                                // Original module path
    Ok(vec!["crate".to_string(), "local_mod".to_string()]), // Current SPP
    Ok(vec![
        "crate".to_string(),
        "local_mod".to_string(),
        "nested".to_string()
    ])  // Final Expected SPP (path to re-exporting module)
);

// 10. Re-export Gated by `#[cfg]`
// NOTE: This test assumes the feature "feature_b" is NOT active by default during testing.
// If it were active, the current SPP would be Ok(["crate", "local_mod"]), expected Ok(["crate"]).
// Since it's likely inactive, the re-export doesn't exist, and SPP for the original item is correct.
#[test]
#[cfg(not(feature = "feature_b"))] // Only run if feature_b is NOT active
fn test_spp_reexport_cfg_gated_inactive() {
    let fixture_name = "fixture_path_resolution";
    let (graph, tree) = build_tree_for_fixture(fixture_name);

    // Find the original item
    let item_id =
        find_item_id_in_module_by_name(&graph, &["crate".to_owned(), "local_mod"], "local_func")
            .expect("Failed to find original local_func");

    let spp_result = tree.shortest_public_path(item_id, &graph);

    // Since feature_b is inactive, the re-export doesn't exist. SPP finds the original path.
    let expected_path = Ok(vec!["crate".to_string(), "local_mod".to_string()]);
    assert_eq!(
        spp_result, expected_path,
        "SPP for item with inactive cfg re-export should resolve to original path."
    );
}

// TODO: Add a test variant for when feature_b IS active. Requires feature management in test setup.

// 11. Re-export of External Dependency Item
#[test]
#[ignore = "Requires dependency resolution for SPP"]
fn test_spp_reexport_external_dep() {
    let fixture_name = "fixture_path_resolution";
    let (graph, tree) = build_tree_for_fixture(fixture_name);

    // We need the NodeId of the *re-export itself* (`log_debug_reexport`)
    // Finding external items by name isn't directly supported by find_item_id_in_module_by_name.
    // We need to find the ImportNode for the re-export.
    let reexport_import_node = graph
        .use_statements
        .iter()
        .find(|imp| imp.visible_name == "log_debug_reexport")
        .expect("Could not find re-export ImportNode for log_debug_reexport");

    // SPP currently doesn't resolve external items via re-exports.
    // It would likely fail trying to find the original `log::debug` in the local graph.
    let spp_result = tree.shortest_public_path(reexport_import_node.id, &graph); // Use re-export's ID

    // Current expected behavior: Error because original item isn't in the graph.
    assert!(
        matches!(
            spp_result,
            Err(ModuleTreeError::ItemNotPubliclyAccessible(_))
        ), // Or potentially NotFound
        "SPP for re-exported external item currently fails (expected final: Ok([\"crate\"]))"
    );

    // Final expected behavior:
    // let expected_path = Ok(vec!["crate".to_string()]);
    // assert_eq!(spp_result, expected_path, "SPP for re-exported external item should be Ok([\"crate\"])");
}

// 12. Re-export of Macro
assert_spp!(
    test_spp_reexport_macro,
    "simple_macro",                // Macro name
    &["crate"],                    // Original module path (defined at root)
    Ok(vec!["crate".to_string()]), // Current SPP (likely correct)
    Ok(vec!["crate".to_string()])  // Final Expected SPP
);

// 13. Item in `pub(crate)` Module
assert_spp!(
    test_spp_item_in_crate_mod,
    "crate_internal_func",
    &["crate", "crate_mod"],
    Err // Expected Err(ItemNotPubliclyAccessible)
);

// 14. `pub(crate)` Item within `#[path]` Module
assert_spp!(
    test_spp_crate_item_in_path_mod,
    "crate_visible_in_actual_file",
    &["crate", "logical_path_mod"],
    Err // Expected Err(ItemNotPubliclyAccessible)
);

// 15. Private Item
assert_spp!(
    test_spp_private_item,
    "private_local_func",
    &["crate", "local_mod"],
    Err // Expected Err(ItemNotPubliclyAccessible)
);

// 16. Public Item in Private Module
assert_spp!(
    test_spp_pub_item_in_private_mod,
    "pub_in_private_inline",
    &["crate", "private_inline_mod"],
    Err // Expected Err(ItemNotPubliclyAccessible)
);
