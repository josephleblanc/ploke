//! Tests focusing on the `shortest_public_path` logic in `ModuleTree`.
//!
//! Note: The current implementation of `shortest_public_path` is basic and
//! likely doesn't handle re-exports correctly yet. These tests focus on
//! direct public visibility paths.

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

    assert_eq!(
        spp, expected_path,
        "Shortest public path for a re-exported item should currently resolve to the original definition's module path"
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
