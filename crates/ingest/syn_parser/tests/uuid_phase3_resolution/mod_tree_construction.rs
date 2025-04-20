//! Tests focusing on the construction and initial state of the ModuleTree,
//! before path resolution logic is implemented.

use std::collections::HashSet;
use std::path::Path;

use ploke_core::NodeId;
use syn_parser::parser::module_tree::ModuleTree;
use syn_parser::parser::nodes::{GraphNode, ModuleDef, ModuleNodeId};
use syn_parser::parser::relations::{GraphId, Relation, RelationKind};
use syn_parser::CodeGraph;

// Use existing helpers if suitable, or define local ones
// Assuming these helpers exist or are adapted in common::uuid_ids_utils
use crate::common::uuid_ids_utils::{
    find_module_node_by_defn_path, find_module_node_by_file_path, run_phases_and_collect,
};

// Helper to build the tree for tests
fn build_tree_for_fixture(fixture_name: &str) -> (CodeGraph, ModuleTree) {
    let results = run_phases_and_collect(fixture_name);
    let mut graphs: Vec<CodeGraph> = Vec::new();
    for parsed_graph in results {
        // Directly use the ParsedCodeGraph's graph field
        graphs.push(parsed_graph.graph);
    }
    let merged_graph = CodeGraph::merge_new(graphs).expect("Failed to merge graphs");
    let tree = merged_graph
        .build_module_tree()
        .expect("Failed to build module tree");
    // Return graph and tree separately, avoiding tuple deconstruction
    (merged_graph, tree)
}

#[test]
fn test_module_tree_module_count() {
    let fixture_name = "file_dir_detection";
    // Avoid tuple deconstruction
    let graph_and_tree = build_tree_for_fixture(fixture_name);
    let graph = graph_and_tree.0;
    let tree = graph_and_tree.1;

    // Assert that the number of modules in the tree's map equals the number in the merged graph
    assert_eq!(
        tree.modules().len(),
        graph.modules.len(),
        "ModuleTree should contain all modules from the merged graph"
    );
}

#[test]
fn test_module_tree_path_index_correctness() {
    let fixture_name = "file_dir_detection";
    // Avoid tuple deconstruction
    let graph_and_tree = build_tree_for_fixture(fixture_name);
    let graph = graph_and_tree.0;
    let tree = graph_and_tree.1;

    // --- Find expected NodeIds from the graph FIRST ---

    // 1. Crate root (main.rs)
    let crate_root_path = Path::new("src/main.rs"); // Relative to fixture root
    let crate_root_node = find_module_node_by_file_path(&graph, crate_root_path)
        .expect("Could not find crate root module node (main.rs)");
    let crate_root_id = crate_root_node.id;

    // 2. Top-level file module (top_pub_mod.rs)
    let top_pub_mod_path = Path::new("src/top_pub_mod.rs");
    let top_pub_mod_node = find_module_node_by_file_path(&graph, top_pub_mod_path)
        .expect("Could not find top_pub_mod.rs module node");
    let top_pub_mod_id = top_pub_mod_node.id;

    // 3. Nested file module (nested_pub.rs)
    let nested_pub_path = Path::new("src/top_pub_mod/nested_pub.rs");
    let nested_pub_node = find_module_node_by_file_path(&graph, nested_pub_path)
        .expect("Could not find nested_pub.rs module node");
    let nested_pub_id = nested_pub_node.id;

    // 4. Inline module (inline_pub_mod in main.rs)
    // We need to find it by its definition path within the crate root
    let inline_pub_mod_node =
        find_module_node_by_defn_path(&graph, &["crate".to_string(), "inline_pub_mod".to_string()])
            .expect("Could not find inline_pub_mod node");
    assert!(
        inline_pub_mod_node.is_inline(),
        "Expected inline_pub_mod to be inline"
    );
    let inline_pub_mod_id = inline_pub_mod_node.id;

    // --- Assertions on the tree's path_index ---

    let path_index = tree.path_index(); // Get a reference to the index

    // Check crate root
    let crate_lookup = path_index
        .get(&["crate".to_string()][..])
        .expect("Path 'crate' not found in index");
    assert_eq!(
        *crate_lookup, crate_root_id,
        "Path 'crate' should map to main.rs module ID"
    );

    // Check top-level file module
    let top_pub_lookup = path_index
        .get(&["crate".to_string(), "top_pub_mod".to_string()][..])
        .expect("Path 'crate::top_pub_mod' not found in index");
    assert_eq!(
        *top_pub_lookup, top_pub_mod_id,
        "Path 'crate::top_pub_mod' should map to top_pub_mod.rs module ID"
    );

    // Check nested file module
    let nested_pub_lookup = path_index
        .get(
            &[
                "crate".to_string(),
                "top_pub_mod".to_string(),
                "nested_pub".to_string(),
            ][..],
        )
        .expect("Path 'crate::top_pub_mod::nested_pub' not found in index");
    assert_eq!(
        *nested_pub_lookup, nested_pub_id,
        "Path 'crate::top_pub_mod::nested_pub' should map to nested_pub.rs module ID"
    );

    // Check inline module
    let inline_pub_lookup = path_index
        .get(&["crate".to_string(), "inline_pub_mod".to_string()][..])
        .expect("Path 'crate::inline_pub_mod' not found in index");
    assert_eq!(
        *inline_pub_lookup, inline_pub_mod_id,
        "Path 'crate::inline_pub_mod' should map to inline_pub_mod module ID"
    );
}

#[test]
fn test_module_tree_resolves_to_definition_relation() {
    let fixture_name = "file_dir_detection";
    // Avoid tuple deconstruction
    let graph_and_tree = build_tree_for_fixture(fixture_name);
    let graph = graph_and_tree.0;
    let tree = graph_and_tree.1;

    // --- Find Declaration and Definition Nodes ---

    // 1. Find declaration `mod top_pub_mod;` in `main.rs`
    let crate_root_node = find_module_node_by_defn_path(&graph, &["crate".to_string()])
        .expect("Crate root not found");
    let top_pub_mod_decl_node = graph
        .get_child_modules_decl(crate_root_node.id)
        .into_iter()
        .find(|m| m.name == "top_pub_mod")
        .expect("Declaration 'mod top_pub_mod;' not found in crate root");
    assert!(
        top_pub_mod_decl_node.is_declaration(),
        "Expected top_pub_mod node in crate root to be a declaration"
    );
    let decl_id = top_pub_mod_decl_node.id;

    // 2. Find definition `top_pub_mod.rs`
    let top_pub_mod_defn_node =
        find_module_node_by_defn_path(&graph, &["crate".to_string(), "top_pub_mod".to_string()])
            .expect("Definition module 'crate::top_pub_mod' not found");
    assert!(
        top_pub_mod_defn_node.is_file_based(),
        "Expected 'crate::top_pub_mod' node to be file-based"
    );
    let defn_id = top_pub_mod_defn_node.id;

    // --- Assert Relation Exists in Tree ---
    let expected_relation = Relation {
        source: GraphId::Node(defn_id), // Source is the definition
        target: GraphId::Node(decl_id), // Target is the declaration
        kind: RelationKind::ResolvesToDefinition,
    };

    let relation_found = tree
        .tree_relations()
        .iter()
        .any(|tree_rel| tree_rel.0 == expected_relation);

    assert!(
        relation_found,
        "Expected ResolvesToDefinition relation not found for top_pub_mod"
    );

    // --- Repeat for nested declaration `mod nested_pub;` in `top_pub_mod.rs` ---

    // 1. Find declaration `mod nested_pub;` in `top_pub_mod.rs`
    let nested_pub_decl_node = graph
        .get_child_modules_decl(top_pub_mod_defn_node.id) // Children of the definition node
        .into_iter()
        .find(|m| m.name == "nested_pub")
        .expect("Declaration 'mod nested_pub;' not found in top_pub_mod.rs");
    let nested_decl_id = nested_pub_decl_node.id;

    // 2. Find definition `nested_pub.rs`
    let nested_pub_defn_node = find_module_node_by_defn_path(
        &graph,
        &[
            "crate".to_string(),
            "top_pub_mod".to_string(),
            "nested_pub".to_string(),
        ],
    )
    .expect("Definition module 'crate::top_pub_mod::nested_pub' not found");
    let nested_defn_id = nested_pub_defn_node.id;

    // --- Assert Relation Exists ---
    let expected_nested_relation = Relation {
        source: GraphId::Node(nested_defn_id),
        target: GraphId::Node(nested_decl_id),
        kind: RelationKind::ResolvesToDefinition,
    };

    let nested_relation_found = tree
        .tree_relations()
        .iter()
        .any(|tree_rel| tree_rel.0 == expected_nested_relation);

    assert!(
        nested_relation_found,
        "Expected ResolvesToDefinition relation not found for nested_pub"
    );
}

#[test]
fn test_module_tree_import_export_segregation() {
    let fixture_name = "file_dir_detection";
    // Avoid tuple deconstruction
    let graph_and_tree = build_tree_for_fixture(fixture_name);
    // let graph = graph_and_tree.0; // Not needed for this test
    let tree = graph_and_tree.1;

    // Collect paths from pending imports and exports for easier assertion
    let import_paths: HashSet<String> = tree
        .pending_imports()
        .iter()
        .map(|p| p.import_node.path.join("::"))
        .collect();

    let export_paths: HashSet<String> = tree
        .pending_exports()
        .iter()
        .map(|p| p.export_node.path.join("::"))
        .collect();

    // --- Assertions for Private Imports ---
    assert!(
        import_paths.contains("std::path::Path"),
        "Expected private import 'std::path::Path' in main.rs"
    );
    assert!(
        import_paths.contains("std::collections::HashMap"),
        "Expected private import 'std::collections::HashMap' in inline_pub_mod"
    );
    assert!(
        import_paths.contains("super::SampleStruct"),
        "Expected private import 'super::SampleStruct' in second_sibling.rs"
    );
    assert!(
        import_paths.contains("super::*"),
        "Expected private import 'super::*' in public_module (second_sibling.rs)"
    );
    // Check that a known re-export is NOT in imports
    assert!(
        !import_paths.contains("crate::top_pub_mod::top_pub_func"),
        "Re-export 'crate::top_pub_mod::top_pub_func' should not be in pending_imports"
    );

    // --- Assertions for Re-Exports ---
    assert!(
        export_paths.contains("crate::top_pub_mod::top_pub_func"),
        "Expected re-export 'crate::top_pub_mod::top_pub_func' in main.rs"
    );
    assert!(
        export_paths.contains("super::outer::middle::inner::deep_function"),
        "Expected re-export 'super::outer::middle::inner::deep_function' in intermediate (second_sibling.rs)"
    );
    assert!(
        export_paths.contains("super::DefaultTrait"),
        "Expected re-export 'super::DefaultTrait' in intermediate (second_sibling.rs)"
    );
    // Check that a known private import is NOT in exports
    assert!(
        !export_paths.contains("std::path::Path"),
        "Private import 'std::path::Path' should not be in pending_exports"
    );

    // Optional: More specific checks on counts if needed, but path checking is robust.
    // let expected_import_count = ...; // Count manually from fixture
    // assert_eq!(tree.pending_imports().len(), expected_import_count);
    // let expected_export_count = ...; // Count manually from fixture
    // assert_eq!(tree.pending_exports().len(), expected_export_count);
}

// NOTE: test_module_tree_duplicate_path_error requires a dedicated fixture
// as described in the previous plan. Skipping implementation for now.
