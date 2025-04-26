//! Tests focusing on the construction and initial state of the ModuleTree,
//! before path resolution logic is implemented.
//!
//! # Current Coverage Summary:
//! *   **Well Covered:**
//!     *   Registration of all modules from the graph into the tree's map.
//!     *   Indexing of canonical paths for definition modules (`path_index`).
//!     *   Creation of `ResolvesToDefinition` relations (`tree_relations`, `decl_index`).
//!     *   Segregation and detailed collection of private/inherited imports (`pending_imports`), including various syntax forms and flags (`is_glob`, `is_extern_crate`).
//!     *   Segregation of public re-exports (`pending_exports`) - currently tested only for emptiness.
//! *   **Not Covered (by *these* tests):**
//!     *   **Error Handling:** Tests don't explicitly trigger or check for `ModuleTreeError` conditions.
//!     *   **`#[path]` attribute:** The effect of `#[path]` on module paths and relations isn't specifically tested here.
//!     *   **Actual Re-exports:** The content and correctness of `pending_exports` when `pub use` statements *are* present is not tested.
//!     *   **Resolution Logic:** Methods intended for Phase 3 resolution (`is_accessible`, `shortest_public_path`, `resolve_path`, etc.) are not exercised by these construction tests.
//!     *   **CFG Interaction:** How `#[cfg]` attributes on modules might influence tree structure or indexing isn't directly tested.

use std::collections::HashSet;
use std::path::Path;

use syn_parser::parser::graph::GraphAccess as _;
use syn_parser::parser::nodes::{GraphId, ModuleNodeId};
use syn_parser::parser::relations::{Relation, RelationKind};

use crate::common::build_tree_for_tests;
// Removed unused imports for helpers moved to CodeGraph

/// **Covers:** Basic sanity check. Ensures that the number of `ModuleNode`s stored
/// in the `ModuleTree`'s internal map (`tree.modules()`) matches the total number
/// of `ModuleNode`s present in the input `CodeGraph`. This confirms that
/// `ModuleTree::add_module` was called for every module without losing any.
#[test]
fn test_module_tree_module_count() {
    let fixture_name = "file_dir_detection";
    // Avoid tuple deconstruction
    let graph_and_tree = build_tree_for_tests(fixture_name);
    let graph = graph_and_tree.0;
    let tree = graph_and_tree.1;

    // Assert that the number of modules in the tree's map equals the number in the merged graph
    assert_eq!(
        tree.modules().len(),
        graph.graph.modules.len(),
        "ModuleTree should contain all modules from the merged graph"
    );
}
/// **Covers:** Correct population of the `path_index` field within the `ModuleTree`.
/// It verifies that the canonical paths (e.g., `["crate"]`, `["crate", "top_pub_mod"]`,
/// `["crate", "top_pub_mod", "nested_pub"]`, `["crate", "inline_pub_mod"]`) for
/// *definition* modules (both file-based and inline) map to the correct `NodeId`s.
/// This tests the logic within `ModuleTree::add_module` that inserts modules into the `path_index`.
#[test]
fn test_module_tree_path_index_correctness() {
    let fixture_name = "file_dir_detection";
    // Avoid tuple deconstruction
    let graph_and_tree = build_tree_for_tests(fixture_name);
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

/// **Covers:** Correct creation of `RelationKind::ResolvesToDefinition` relations
/// within the `tree_relations` field. It finds specific module *declarations*
/// (`mod foo;`) and their corresponding *definitions* (`foo.rs` or `mod foo {}`)
/// in the graph and asserts that the expected relation linking them exists in the tree.
/// This primarily tests the `ModuleTree::link_mods_syntactic()` method and implicitly
/// the population of the `decl_index`.
#[test]
fn test_module_tree_resolves_to_definition_relation() {
    let fixture_name = "file_dir_detection";
    // Avoid tuple deconstruction
    let graph_and_tree = build_tree_for_tests(fixture_name);
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
        source: GraphId::Node(decl_id), // Source is the declaration
        target: GraphId::Node(defn_id), // Target is the definition
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
        source: GraphId::Node(nested_decl_id),
        target: GraphId::Node(nested_defn_id),
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

/// **Covers:** Correct separation of `ImportNode`s into the `pending_imports` and
/// `pending_exports` lists based on their visibility (`use` vs `pub use`). It checks
/// that various forms of private/inherited `use` statements (simple, renamed, grouped,
/// glob, relative, absolute, extern crate) end up in `pending_imports` and that
/// (in this specific fixture) `pending_exports` is empty because there are no `pub use`
/// statements. This tests the filtering logic within `ModuleTree::add_module`.
#[test]
fn test_module_tree_import_export_segregation() {
    // Use the fixture_nodes crate, specifically focusing on imports.rs
    let fixture_name = "fixture_nodes";
    let graph_and_tree = build_tree_for_tests(fixture_name);
    let tree = graph_and_tree.1;

    // Collect paths from pending imports and exports
    let import_paths: HashSet<String> = tree
        .pending_imports()
        .iter()
        .map(|p| {
            let node = p.import_node();
            let path_segments = node.source_path();
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
        .map(|p| p.export_node().source_path.join("::"))
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

/// **Covers:** Comprehensive and exact verification of the contents of the
/// `pending_imports` list. It checks not just the paths but also the `is_glob`
/// and `is_extern_crate` flags for *all* expected private/inherited imports
/// gathered from the *entire* `fixture_nodes` crate. It ensures that the details
/// extracted during `process_use_tree` and stored by `ModuleTree::add_module`
/// are accurate for various import syntaxes.
#[test]
fn test_module_tree_imports_fixture_nodes() {
    let fixture_name = "fixture_nodes";
    let graph_and_tree = build_tree_for_tests(fixture_name);
    let tree = graph_and_tree.1; // We only need the tree for this test

    // --- Check Pending Exports ---
    assert!(
        tree.pending_exports().is_empty(),
        "Expected no pending exports from fixture_nodes/imports.rs, found: {:?}",
        tree.pending_exports()
            .iter()
            .map(|p| p.export_node().source_path.join("::"))
            .collect::<Vec<_>>()
    );

    // --- Check Pending Imports ---
    // Collect details for easier assertion
    let pending_imports_details: HashSet<(String, bool, bool)> = tree
        .pending_imports()
        .iter()
        .map(|p| {
            let node = p.import_node();
            let path_str = node.source_path.join("::");
            // Return tuple: (path_string, is_glob, is_extern_crate)
            (path_str, node.is_glob, node.is_extern_crate())
        })
        .collect();

    // Define expected imports (path_string, is_glob, is_extern_crate)
    let expected_imports: HashSet<(String, bool, bool)> = [
        // --- From imports.rs top level ---
        ("crate::structs::TupleStruct".to_string(), false, false),
        ("std::collections::HashMap".to_string(), false, false),
        ("std::fmt".to_string(), false, false),
        ("std::sync::Arc".to_string(), false, false),
        ("crate::structs::SampleStruct".to_string(), false, false), // Renamed
        ("std::io::Result".to_string(), false, false),              // Renamed
        ("crate::enums::EnumWithData".to_string(), false, false),   // Grouped
        ("crate::enums::SampleEnum1".to_string(), false, false),    // Grouped
        ("crate::traits::GenericTrait".to_string(), false, false),  // Grouped + Renamed
        ("crate::traits::SimpleTrait".to_string(), false, false),   // Grouped
        ("std::fs".to_string(), false, false),                      // Grouped (module)
        ("std::fs::File".to_string(), false, false),                // Grouped (item)
        ("std::path::Path".to_string(), false, false),              // Grouped
        ("std::path::PathBuf".to_string(), false, false),           // Grouped
        ("std::env".to_string(), true, false), // Glob import (path is to the module)
        ("self::sub_imports::SubItem".to_string(), false, false), // Relative self
        ("super::structs::AttributedStruct".to_string(), false, false), // Relative super
        ("crate::type_alias::SimpleId".to_string(), false, false), // Relative crate
        ("::std::time::Duration".to_string(), false, false), // Absolute path
        ("serde".to_string(), false, true),    // Extern crate
        // Renamed extern crate 'serde as SerdeAlias' also has path "serde"
        // --- From imports.rs -> sub_imports module ---
        ("super::fmt".to_string(), false, false),
        ("crate::enums::DocumentedEnum".to_string(), false, false),
        ("std::sync::Arc".to_string(), false, false), // Duplicate path, but different context (ok)
        ("self::nested_sub::NestedItem".to_string(), false, false),
        (
            "super::super::structs::TupleStruct".to_string(),
            false,
            false,
        ),
        // --- Imports from other files in fixture_nodes ---
        ("std::fmt::Debug".to_string(), false, false), // From traits.rs
        ("super::SimpleStruct".to_string(), false, false), // From impls.rs (use super::structs::SimpleStruct) - Path relative to impls.rs
        ("super::SimpleTrait".to_string(), false, false), // From traits.rs inner module (use super::SimpleTrait) - Path relative to traits/inner
    ]
    .into_iter()
    .map(|(s, g, e)| (s.to_string(), g, e)) // Ensure owned Strings
    .collect();

    // Assert equality between the sets
    assert_eq!(
        pending_imports_details,
        expected_imports,
        "Mismatch in pending imports.
Actual: {:#?}\n
Expected: {:#?}\n
In Expected missing from Actual: {:#?}\n
In Actual missing from Expected: {:#?}\n",
        pending_imports_details
            .iter()
            .map(|(path, is_glob, is_extern)| format!(
                "path: {: <35} is_glob: {: <5}, is_extern: {: <5}",
                path, is_glob, is_extern
            ))
            .collect::<Vec<_>>(),
        expected_imports
            .iter()
            .map(|(path, is_glob, is_extern)| format!(
                "path: {: <35} is_glob: {: <5}, is_extern: {: <5}",
                path, is_glob, is_extern
            ))
            .collect::<Vec<_>>(),
        expected_imports
            .iter()
            .filter(|item| !pending_imports_details.contains(item))
            .map(|(path, is_glob, is_extern)| format!(
                "path: {: <35} is_glob: {: <5}, is_extern: {: <5}",
                path, is_glob, is_extern
            ))
            .collect::<Vec<_>>(),
        pending_imports_details
            .iter()
            .filter(|item| !expected_imports.contains(item))
            .map(|(path, is_glob, is_extern)| format!(
                "path: {: <35} is_glob: {: <5}, is_extern: {: <5}",
                path, is_glob, is_extern
            ))
            .collect::<Vec<_>>(),
    );

    // Optional: Spot check specific nodes for more details if needed
    let hashmap_import = tree
        .pending_imports()
        .iter()
        .find(|p| p.import_node().source_path.join("::") == "std::collections::HashMap")
        .map(|p| p.import_node())
        .expect("HashMap import not found");
    assert!(!hashmap_import.is_local_reexport());
    assert!(hashmap_import.is_inherited_use()); // Should be inherited visibility or extern crate

    let renamed_struct_import = tree
        .pending_imports()
        .iter()
        .find(|p| {
            let node = p.import_node();
            node.source_path.join("::") == "crate::structs::SampleStruct"
                && node.visible_name == "MySimpleStruct"
        })
        .map(|p| p.import_node())
        .expect("Renamed SampleStruct import not found");
    assert_eq!(
        renamed_struct_import.original_name,
        Some("SampleStruct".to_string())
    );

    let glob_import = tree
        .pending_imports()
        .iter()
        .find(|p| p.import_node().is_glob && p.import_node().source_path.join("::") == "std::env")
        .map(|p| p.import_node())
        .expect("Glob import 'std::env::*' not found");
    assert_eq!(glob_import.visible_name, "*");

    let extern_serde = tree
        .pending_imports()
        .iter()
        .find(|p| {
            p.import_node().is_extern_crate() && p.import_node().source_path.join("::") == "serde"
        })
        .map(|p| p.import_node())
        .expect("Extern crate serde not found");
    assert!(extern_serde.is_inherited_use()); // Extern crates are treated as inherited for pending list
}

/// **Covers:** Basic visibility checks using `ModuleTree::is_accessible`.
/// It uses the `file_dir_detection` fixture to test access between modules
use std::io::Write; // Import Write trait for formatting

/// with different visibility levels (public, crate, restricted, inherited).
#[test]
fn test_module_tree_is_accessible() {
    // Initialize logger with custom format for this test
    let _ = env_logger::builder()
        // Parse RUST_LOG environment variable
        .parse_filters(&std::env::var("RUST_LOG").unwrap_or_default())
        // Define custom format without timestamp
        .format(|buf, record| {
            writeln!(
                buf,
                // Example format: [LEVEL TARGET] Message
                "[{:<5} {}] {}", // Adjust padding as needed
                record.level(),
                record.target(),
                record.args()
            )
        })
        .try_init(); // Use try_init to avoid panic if already initialized

    let fixture_name = "file_dir_detection";
    let graph_and_tree = build_tree_for_tests(fixture_name);
    let graph = graph_and_tree.0;
    let tree = graph_and_tree.1;

    // --- Get Module IDs ---
    let crate_root_id = tree.root(); // ID of main.rs

    let top_pub_mod_id = ModuleNodeId::new(
        graph
            .find_module_by_defn_path_checked(&["crate".to_string(), "top_pub_mod".to_string()])
            .expect("Failed to find top_pub_mod")
            .id,
    );

    let top_priv_mod_id = ModuleNodeId::new(
        graph
            .find_module_by_defn_path_checked(&["crate".to_string(), "top_priv_mod".to_string()])
            .expect("Failed to find top_priv_mod")
            .id,
    );

    let nested_pub_in_pub_id = ModuleNodeId::new(
        graph
            .find_module_by_defn_path_checked(&[
                "crate".to_string(),
                "top_pub_mod".to_string(),
                "nested_pub".to_string(),
            ])
            .expect("Failed to find nested_pub in top_pub_mod")
            .id,
    );

    let nested_priv_in_pub_id = ModuleNodeId::new(
        graph
            .find_module_by_defn_path_checked(&[
                "crate".to_string(),
                "top_pub_mod".to_string(),
                "nested_priv".to_string(),
            ])
            .expect("Failed to find nested_priv in top_pub_mod")
            .id,
    );

    let nested_pub_in_priv_id = ModuleNodeId::new(
        graph
            .find_module_by_defn_path_checked(&[
                "crate".to_string(),
                "top_priv_mod".to_string(),
                "nested_pub_in_priv".to_string(),
            ])
            .expect("Failed to find nested_pub_in_priv")
            .id,
    );

    let nested_priv_in_priv_id = ModuleNodeId::new(
        graph
            .find_module_by_defn_path_checked(&[
                "crate".to_string(),
                "top_priv_mod".to_string(),
                "nested_priv_in_priv".to_string(),
            ])
            .expect("Failed to find nested_priv_in_priv")
            .id,
    );

    let path_visible_mod_id = ModuleNodeId::new(
        graph
            .find_module_by_defn_path_checked(&[
                "crate".to_string(),
                "top_pub_mod".to_string(),
                "path_visible_mod".to_string(),
            ])
            .expect("Failed to find path_visible_mod")
            .id,
    ); // This one is pub(in crate::top_pub_mod)

    // --- Debugging Step 1: Log relevant relations before assertion ---
    use colored::*; // Ensure colored is in scope for formatting
    use log::debug; // Ensure debug macro is in scope

    // Find the declaration ID for nested_pub within top_pub_mod
    let top_pub_mod_defn_node = graph
        .get_module_checked(top_pub_mod_id.into_inner())
        .expect("top_pub_mod definition node not found in graph");
    let nested_pub_decl_node = graph
        .get_child_modules_decl(top_pub_mod_defn_node.id) // Use graph method
        .into_iter()
        .find(|m| m.name == "nested_pub")
        .expect("Declaration 'mod nested_pub;' not found in top_pub_mod.rs");
    let nested_pub_decl_id = nested_pub_decl_node.id;

    debug!(target: "mod_tree_vis", "{}", "--- Relation Check Start ---".dimmed().bold());
    debug!(target: "mod_tree_vis", "Checking relations involving:");
    debug!(target: "mod_tree_vis", "  - top_pub_mod (Defn): {}", top_pub_mod_id.to_string().magenta());
    debug!(target: "mod_tree_vis", "  - nested_pub (Defn):  {}", nested_pub_in_pub_id.to_string().magenta());
    debug!(target: "mod_tree_vis", "  - nested_pub (Decl):  {}", nested_pub_decl_id.to_string().magenta());

    let relevant_ids = [
        top_pub_mod_id.into_inner(),
        nested_pub_in_pub_id.into_inner(),
        nested_pub_decl_id,
    ];

    let mut found_direct_contains = false;
    let mut found_resolves_to = false;
    let mut found_decl_contains = false;

    for tree_rel in tree.tree_relations() {
        let rel = tree_rel.relation();
        let source_id_opt = match rel.source {
            GraphId::Node(id) => Some(id),
            _ => None,
        };
        let target_id_opt = match rel.target {
            GraphId::Node(id) => Some(id),
            _ => None,
        };

        // Check if either source or target is one of our relevant IDs
        if let (Some(source_id), Some(target_id)) = (source_id_opt, target_id_opt) {
            if relevant_ids.contains(&source_id) || relevant_ids.contains(&target_id) {
                // Format the relation for logging
                // Use graph.find_node to get names, fallback to "?"
                let source_name = graph.find_node(source_id).map(|n| n.name()).unwrap_or("?");
                let target_name = graph.find_node(target_id).map(|n| n.name()).unwrap_or("?");
                debug!(target: "mod_tree_vis", "  Found Relation: {} ({}) --{:?}--> {} ({})",
                    source_name.yellow(),
                    source_id.to_string().magenta(),
                    rel.kind,
                    target_name.blue(),
                    target_id.to_string().magenta()
                );

                // Check for the specific relations needed by get_parent_module_id
                // 1. Direct Contains (Parent -> Definition)
                if source_id == top_pub_mod_id.into_inner()
                    && target_id == nested_pub_in_pub_id.into_inner()
                    && rel.kind == RelationKind::Contains
                {
                    found_direct_contains = true;
                }
                // 2. ResolvesToDefinition (Declaration -> Definition)
                if source_id == nested_pub_decl_id
                    && target_id == nested_pub_in_pub_id.into_inner()
                    && rel.kind == RelationKind::ResolvesToDefinition
                {
                    found_resolves_to = true;
                }
                // 3. Declaration Contains (Parent -> Declaration)
                if source_id == top_pub_mod_id.into_inner()
                    && target_id == nested_pub_decl_id
                    && rel.kind == RelationKind::Contains
                {
                    found_decl_contains = true;
                }
            }
        }
    }

    // Log summary of findings
    debug!(target: "mod_tree_vis", "  Check Summary:");
    debug!(target: "mod_tree_vis", "    - Direct Contains ({} -> {}): {}", top_pub_mod_id, nested_pub_in_pub_id, if found_direct_contains {"Found".green()} else {"Missing".red()});
    debug!(target: "mod_tree_vis", "    - ResolvesToDefinition ({} -> {}): {}", nested_pub_decl_id, nested_pub_in_pub_id, if found_resolves_to {"Found".green()} else {"Missing".red()});
    debug!(target: "mod_tree_vis", "    - Declaration Contains ({} -> {}): {}", top_pub_mod_id, nested_pub_decl_id, if found_decl_contains {"Found".green()} else {"Missing".red()});
    debug!(target: "mod_tree_vis", "{}", "--- Relation Check End ---".dimmed().bold());
    // --- End Debugging Step 1 ---

    // --- Assertions ---

    // 1. Public access: top_pub_mod should be accessible from anywhere (e.g., crate root)
    assert!(
        tree.is_accessible(crate_root_id, top_pub_mod_id),
        "Public module (top_pub_mod) should be accessible from crate root"
    );
    assert!(
        tree.is_accessible(top_priv_mod_id, top_pub_mod_id),
        "Public module (top_pub_mod) should be accessible from private sibling"
    );
    assert!(
        tree.is_accessible(nested_pub_in_pub_id, top_pub_mod_id),
        "Public module (top_pub_mod) should be accessible from its public child"
    );

    // 2. Crate access: crate_visible_mod.rs (implicitly pub(crate))
    //    Need to add this module to the fixture and find its ID first.
    //    Skipping crate visibility tests for now as the fixture lacks a clear pub(crate) module.

    // 3. Restricted access: path_visible_mod is pub(in crate::top_pub_mod)
    assert!(
        tree.is_accessible(top_pub_mod_id, path_visible_mod_id),
        "Restricted module (path_visible_mod) should be accessible from within its restriction scope (top_pub_mod)"
    );
    assert!(
        tree.is_accessible(nested_pub_in_pub_id, path_visible_mod_id),
        "Restricted module (path_visible_mod) should be accessible from descendant of restriction scope (nested_pub_in_pub)"
    );
    assert!(
        !tree.is_accessible(crate_root_id, path_visible_mod_id),
        "Restricted module (path_visible_mod) should NOT be accessible from outside its restriction scope (crate_root)"
    );
    assert!(
        !tree.is_accessible(top_priv_mod_id, path_visible_mod_id),
        "Restricted module (path_visible_mod) should NOT be accessible from sibling outside restriction scope (top_priv_mod)"
    );

    // 4. Inherited access (private):
    //    - nested_priv_in_pub should only be accessible from top_pub_mod
    //    - nested_priv_in_priv should only be accessible from top_priv_mod
    assert!(
        tree.is_accessible(top_pub_mod_id, nested_priv_in_pub_id),
        "Inherited module (nested_priv_in_pub) should be accessible from its parent (top_pub_mod)"
    );
    assert!(
        !tree.is_accessible(crate_root_id, nested_priv_in_pub_id),
        "Inherited module (nested_priv_in_pub) should NOT be accessible from grandparent (crate_root)"
    );
    assert!(
        !tree.is_accessible(nested_pub_in_pub_id, nested_priv_in_pub_id),
        "Inherited module (nested_priv_in_pub) should NOT be accessible from sibling (nested_pub_in_pub)"
    );

    assert!(
        tree.is_accessible(top_priv_mod_id, nested_priv_in_priv_id),
        "Inherited module (nested_priv_in_priv) should be accessible from its parent (top_priv_mod)"
    );
    assert!(
        !tree.is_accessible(crate_root_id, nested_priv_in_priv_id),
        "Inherited module (nested_priv_in_priv) should NOT be accessible from grandparent (crate_root)"
    );
    assert!(
        !tree.is_accessible(nested_pub_in_priv_id, nested_priv_in_priv_id),
        "Inherited module (nested_priv_in_priv) should NOT be accessible from sibling (nested_pub_in_priv)"
    );

    // 5. Accessing self
    assert!(
        tree.is_accessible(top_pub_mod_id, top_pub_mod_id),
        "Module should be accessible from itself"
    );
    assert!(
        tree.is_accessible(top_priv_mod_id, top_priv_mod_id),
        "Module should be accessible from itself"
    );
}
