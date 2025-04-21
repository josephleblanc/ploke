//! Tests focusing on the `shortest_public_path` logic in `ModuleTree`.
//!
//! Note: The current implementation of `shortest_public_path` is basic and
//! likely doesn't handle re-exports correctly yet. These tests focus on
//! direct public visibility paths.

use syn_parser::parser::module_tree::ModuleTree;
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

    // Expected path: ["crate"] (path to the containing module)
    // NOTE: The current shortest_public_path implementation might only return the module path.
    // Adjust assertion based on actual implementation behavior.
    // For now, let's assume it should return the module path containing the item.
    let expected_path = Some(vec!["crate".to_string()]); // Path to the containing module

    assert_eq!(
        spp, expected_path,
        "Shortest public path for root public function"
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
            f.name == "top_pub_func"
                && graph.get_item_module_path(f.id) == ["crate", "top_pub_mod"]
        })
        .expect("Could not find top_pub_func in top_pub_mod")
        .id;

    // Find the crate root module ID
    // Calculate shortest public path starting from the crate root
    // Pass the graph as required by the new signature
    let spp = tree.shortest_public_path(top_pub_func_id, &graph);

    // Expected path: ["crate", "top_pub_mod"] (path to the containing public module)
    let expected_path = Some(vec!["crate".to_string(), "top_pub_mod".to_string()]);

    assert_eq!(
        spp, expected_path,
        "Shortest public path for public function in public module"
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

    // Expected path: ["crate", "top_pub_mod", "nested_pub"]
    let expected_path = Some(vec![
        "crate".to_string(),
        "top_pub_mod".to_string(),
        "nested_pub".to_string(),
    ]);

    assert_eq!(
        spp, expected_path,
        "Shortest public path for public function in nested public module"
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
    let spp = tree.shortest_public_path(top_pub_priv_func_id, &graph);

    // Expected path: None (item is private)
    let expected_path: Option<Vec<String>> = None;

    assert_eq!(
        spp, expected_path,
        "Shortest public path for private function should be None"
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
    let spp = tree.shortest_public_path(nested_pub_in_priv_func_id, &graph);

    // Expected path: None (containing module is private)
    let expected_path: Option<Vec<String>> = None;

    assert_eq!(
        spp, expected_path,
        "Shortest public path for item in private module should be None"
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
