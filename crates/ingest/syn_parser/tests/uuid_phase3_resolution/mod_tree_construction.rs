//! Tests focusing on the construction and initial state of the ModuleTree,
//! before path resolution logic is implemented.

use std::collections::HashSet;
use std::path::Path;

use syn_parser::parser::module_tree::ModuleTree;
use syn_parser::parser::relations::{GraphId, Relation, RelationKind};
use syn_parser::CodeGraph;

// Removed unused imports for helpers moved to CodeGraph
use crate::common::uuid_ids_utils::run_phases_and_collect;

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

    // --- Find expected NodeIds from the graph FIRST ---
    // Use the new CodeGraph methods with error handling

    // 1. Crate root (main.rs)
    let crate_root_path = Path::new("src/main.rs"); // Relative to fixture root
    let crate_root_node = graph
        .find_module_by_file_path_checked(crate_root_path)
        .expect("Could not find crate root module node (main.rs)");
    let crate_root_id = crate_root_node.id;

    // 2. Top-level file module (top_pub_mod.rs)
    let top_pub_mod_path = Path::new("src/top_pub_mod.rs");
    let top_pub_mod_node = graph
        .find_module_by_file_path_checked(top_pub_mod_path)
        .expect("Could not find top_pub_mod.rs module node");
    let top_pub_mod_id = top_pub_mod_node.id;

    // 3. Nested file module (nested_pub.rs)
    let nested_pub_path = Path::new("src/top_pub_mod/nested_pub.rs");
    let nested_pub_node = graph
        .find_module_by_file_path_checked(nested_pub_path)
        .expect("Could not find nested_pub.rs module node");
    let nested_pub_id = nested_pub_node.id;

    // 4. Inline module (inline_pub_mod in main.rs)
    // We need to find it by its definition path within the crate root
    let inline_pub_mod_node = graph
        .find_module_by_defn_path_checked(&["crate".to_string(), "inline_pub_mod".to_string()])
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

    // --- Find Declaration and Definition Nodes ---

    // 1. Find declaration `mod top_pub_mod;` in `main.rs`
    let crate_root_node = graph
        .find_module_by_defn_path_checked(&["crate".to_string()])
        .expect("Crate root not found");
    let top_pub_mod_decl_node = graph
        .get_child_modules_decl(crate_root_node.id) // Assuming this helper still works or is adapted
        .into_iter()
        .find(|m| m.name == "top_pub_mod")
        .expect("Declaration 'mod top_pub_mod;' not found in crate root");
    assert!(
        top_pub_mod_decl_node.is_declaration(),
        "Expected top_pub_mod node in crate root to be a declaration"
    );
    let decl_id = top_pub_mod_decl_node.id;

    // 2. Find definition `top_pub_mod.rs`
    let top_pub_mod_defn_node = graph
        .find_module_by_defn_path_checked(&["crate".to_string(), "top_pub_mod".to_string()])
        .expect("Definition module 'crate::top_pub_mod' not found");
    assert!(
        top_pub_mod_defn_node.is_file_based(), // Assuming this helper still works
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
        .any(|tree_rel| *tree_rel.relation() == expected_relation); // Use the getter

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
    let nested_pub_defn_node = graph
        .find_module_by_defn_path_checked(&[
            "crate".to_string(),
            "top_pub_mod".to_string(),
            "nested_pub".to_string(),
        ])
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
        .any(|tree_rel| *tree_rel.relation() == expected_nested_relation); // Use the getter

    assert!(
        nested_relation_found,
        "Expected ResolvesToDefinition relation not found for nested_pub"
    );
}

#[test]
fn test_module_tree_import_export_segregation() {
    // Use the fixture_nodes crate, specifically focusing on imports.rs
    let fixture_name = "fixture_nodes";
    let graph_and_tree = build_tree_for_fixture(fixture_name);
    let tree = graph_and_tree.1;

    // Collect paths from pending imports and exports
    let import_paths: HashSet<String> = tree
        .pending_imports()
        .iter()
        .map(|p| {
            let node = p.import_node();
            let path_segments = node.path();
            let base_path_str = if path_segments.first().is_some_and(|s| s.is_empty()) {
                // Handle absolute paths like ::std::time::Duration
                format!("::{}", path_segments[1..].join("::"))
            } else {
                path_segments.join("::")
            };

            // Append "::*" if it's a glob import
            if node.is_glob {
                // Handle edge case where path itself might be empty (e.g., `use ::*;` - unlikely but possible)
                if base_path_str.is_empty() || base_path_str == "::" {
                    format!("{}*", base_path_str) // Results in "*" or "::*"
                } else {
                    format!("{}::*", base_path_str)
                }
            } else {
                base_path_str
            }
        })
        .collect();

    let export_paths: HashSet<String> = tree
        .pending_exports()
        .iter()
        .map(|p| p.export_node().path.join("::"))
        .collect();

    // --- Assertions for Private Imports (from imports.rs) ---
    // Check a few representative private imports
    assert!(
        import_paths.contains("std::collections::HashMap"),
        "Expected private import 'std::collections::HashMap'"
    );
    assert!(
        import_paths.contains("crate::structs::SampleStruct"), // Note: Path uses original name
        "Expected private import 'crate::structs::SampleStruct' (renamed)"
    );
    assert!(
        import_paths.contains("crate::traits::SimpleTrait"),
        "Expected private import 'crate::traits::SimpleTrait'"
    );
    assert!(
        import_paths.contains("std::fs"), // Group import `fs::{self, File}` includes `fs` itself
        "Expected private import 'std::fs'"
    );
    assert!(
        import_paths.contains("std::fs::File"),
        "Expected private import 'std::fs::File'"
    );
    assert!(
        import_paths.contains("std::env::*"), // Check glob import representation
        "Expected private glob import 'std::env::*'"
    );
    assert!(
        import_paths.contains("self::sub_imports::SubItem"),
        "Expected private import 'self::sub_imports::SubItem'"
    );
    assert!(
        import_paths.contains("super::structs::AttributedStruct"),
        "Expected private import 'super::structs::AttributedStruct'"
    );
    assert!(
        import_paths.contains("crate::type_alias::SimpleId"),
        "Expected private import 'crate::type_alias::SimpleId'"
    );
    assert!(
        import_paths.contains("::std::time::Duration"), // Check absolute path import
        "Expected private import '::std::time::Duration'"
    );
    // Check imports from within the nested `sub_imports` module
    assert!(
        import_paths.contains("super::fmt"),
        "Expected private import 'super::fmt' from sub_imports"
    );
    assert!(
        import_paths.contains("crate::enums::DocumentedEnum"),
        "Expected private import 'crate::enums::DocumentedEnum' from sub_imports"
    );
    assert!(
        import_paths.contains("self::nested_sub::NestedItem"),
        "Expected private import 'self::nested_sub::NestedItem' from sub_imports"
    );
    assert!(
        import_paths.contains("super::super::structs::TupleStruct"),
        "Expected private import 'super::super::structs::TupleStruct' from sub_imports"
    );

    // --- Assertions for Re-Exports ---
    // The imports.rs fixture does not contain any `pub use` statements.
    assert!(
        export_paths.is_empty(),
        "Expected no pending exports from imports.rs, found: {:?}",
        export_paths
    );

    // --- Assertions for Extern Crates (Check if they appear as pending imports) ---
    // The current ModuleTree::add_module logic likely treats extern crates like private imports
    assert!(
        import_paths.contains("serde"),
        "Expected extern crate 'serde' to be treated as a pending import"
    );
    // Note: The renamed extern crate 'serde as SerdeAlias' should still have the path "serde"
    // in the ImportNode, but the test setup doesn't easily distinguish between the two extern
    // crate statements based solely on path. We just check that "serde" is present once.
}

// NOTE: test_module_tree_duplicate_path_error requires a dedicated fixture
// as described in the previous plan. Skipping implementation for now.
