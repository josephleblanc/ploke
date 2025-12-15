#![cfg(test)]
#![allow(unused_imports)]
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

use colored::Colorize as _;
use itertools::Itertools;
use log::debug;
use syn_parser::error::SynParserError;
use syn_parser::parser::graph::GraphAccess as _;
use syn_parser::parser::nodes::{
    AsAnyNodeId, ModuleNode, ModuleNodeId, PrimaryNodeId, PrimaryNodeIdTrait,
};
use syn_parser::parser::relations::SyntacticRelation;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::resolve::module_tree::ModuleTree;
use syn_parser::resolve::{RelationIndexer, TreeRelation};
use syn_parser::utils::{LogStyle, LOG_TARGET_MOD_TREE_BUILD};

use crate::common::build_tree_for_tests;

// NOTE:
// This test is replaced by unit testing in `parsed_graph.rs`,
//  see parsed_graph::tests::test_build_mod_tree_inners
// #[cfg(not(feature = "type_bearing_ids"))]
// fn test_module_tree_module_count() {
// }
/// **Covers:** Correct population of the `path_index` field within the `ModuleTree`.
/// It verifies that the canonical paths (e.g., `["crate"]`, `["crate", "top_pub_mod"]`,
/// `["crate", "top_pub_mod", "nested_pub"]`, `["crate", "inline_pub_mod"]`) for
/// *definition* modules (both file-based and inline) map to the correct `NodeId`s.
/// This tests the logic within `ModuleTree::add_module` that inserts modules into the `path_index`.
#[test]
#[cfg(test)]
// #[cfg(not(feature = "type_bearing_ids"))]
fn test_module_tree_path_index_correctness() {
    use syn_parser::parser::nodes::AsAnyNodeId as _;

    let _ = env_logger::builder() // Parse RUST_LOG environment variable
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
    // Avoid tuple deconstruction
    let graph_and_tree = build_tree_for_tests(fixture_name);
    let graph = graph_and_tree.0;
    let tree = graph_and_tree.1;

    #[cfg(feature = "validate")]
    graph.validate_unique_rels();

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
        .find_module_by_path_checked(&["crate".to_string(), "inline_pub_mod".to_string()])
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
        *crate_lookup,
        crate_root_id.as_any(),
        "Path 'crate' should map to main.rs module ID"
    );

    // Check top-level file module
    let top_pub_lookup = path_index
        .get(&["crate".to_string(), "top_pub_mod".to_string()][..])
        .expect("Path 'crate::top_pub_mod' not found in index");
    assert_eq!(
        *top_pub_lookup,
        top_pub_mod_id.as_any(),
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
        *nested_pub_lookup,
        nested_pub_id.as_any(),
        "Path 'crate::top_pub_mod::nested_pub' should map to nested_pub.rs module ID"
    );

    // Check inline module
    let inline_pub_lookup = path_index
        .get(&["crate".to_string(), "inline_pub_mod".to_string()][..])
        .expect("Path 'crate::inline_pub_mod' not found in index");
    assert_eq!(
        *inline_pub_lookup,
        inline_pub_mod_id.as_any(),
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
// #[cfg(not(feature = "type_bearing_ids"))]
fn test_module_tree_resolves_to_definition_relation() {
    let fixture_name = "file_dir_detection";
    eprintln!("starting test with target dir: {fixture_name}");
    // Avoid tuple deconstruction
    let graph_and_tree = build_tree_for_tests(fixture_name);
    let graph = graph_and_tree.0;
    let tree = graph_and_tree.1;

    // --- Find Declaration and Definition Nodes ---

    // --- Find Declaration and Definition Nodes ---

    // 1. Find declaration `mod top_pub_mod;` in `main.rs`
    let crate_root_node = graph
        .find_module_by_path_checked(&["crate".to_string()])
        .expect("Crate root not found");
    let crate_child_mods_count = graph
        .get_child_modules(crate_root_node.id)
        .inspect(|m| eprintln!("module node in iter: {}", m.name))
        .count();
    eprintln!("Count of mods as items in root module: {crate_child_mods_count}");

    let root_mod_id = crate_root_node.id;
    let top_pub_mod_decl_node = get_child_mod_decl(&tree, root_mod_id, "top_pub_mod")
        .expect("Should find the module declaration in tree");

    // let top_pub_mod_in_graph = graph
    //     .get_module_checked(top_pub_mod_decl_node_id)
    //     .expect("Should find the module declaration in graph");

    // let top_pub_mod_decl_node = graph
    //     .get_child_modules_decl(crate_root_node.id) // Assuming this helper still works or is adapted
    //     .inspect(|m| eprintln!("module node in iter: {}", m.name))
    //     .find(|m| m.name == "top_pub_mod")
    //     .expect("Declaration 'mod top_pub_mod;' not found in crate root");
    assert!(
        top_pub_mod_decl_node.is_decl(),
        "Expected top_pub_mod node in crate root to be a declaration"
    );
    let decl_id = top_pub_mod_decl_node.id;

    // 2. Find definition `top_pub_mod.rs`
    let top_pub_mod_defn_node = graph
        .find_module_by_path_checked(&["crate".to_string(), "top_pub_mod".to_string()])
        .expect("Definition module 'crate::top_pub_mod' not found");
    assert!(
        top_pub_mod_defn_node.is_file_based(), // Assuming this helper still works
        "Expected 'crate::top_pub_mod' node to be file-based"
    );
    let defn_id = top_pub_mod_defn_node.id;

    // --- Assert Relation Exists in Tree ---
    let expected_relation = SyntacticRelation::ResolvesToDefinition {
        source: decl_id,
        target: defn_id,
    };

    let relation_found = tree
        .tree_relations()
        .iter()
        .any(|tree_rel| *tree_rel.rel() == expected_relation); // Use the getter

    assert!(
        relation_found,
        "Expected ResolvesToDefinition relation not found for top_pub_mod"
    );

    // --- Repeat for nested declaration `mod nested_pub;` in `top_pub_mod.rs` ---

    // 1. Find declaration `mod nested_pub;` in `top_pub_mod.rs`
    let nested_pub_decl_node = get_child_mod_decl(&tree, top_pub_mod_defn_node.id, "nested_pub")
        .expect("Should find the module declaration for nested_pub in tree");
    // TODO: delete after test passes
    // - old way, doesn't work,
    // let nested_pub_decl_node = graph
    //     .get_child_modules_decl(top_pub_mod_defn_node.id) // Children of the definition node
    //     .into_iter()
    //     .find(|m| m.name == "nested_pub")
    //     .expect("Declaration 'mod nested_pub;' not found in top_pub_mod.rs");
    let nested_decl_id = nested_pub_decl_node.id;

    // 2. Find definition `nested_pub.rs`
    let nested_pub_defn_node = graph
        .find_module_by_path_checked(&[
            "crate".to_string(),
            "top_pub_mod".to_string(),
            "nested_pub".to_string(),
        ])
        .expect("Definition module 'crate::top_pub_mod::nested_pub' not found");
    let nested_defn_id = nested_pub_defn_node.id;

    // --- Assert Relation Exists ---
    let expected_nested_relation = SyntacticRelation::ResolvesToDefinition {
        source: nested_decl_id,
        target: nested_defn_id,
    };

    let nested_relation_found = tree
        .tree_relations()
        .iter()
        .any(|tree_rel| *tree_rel.rel() == expected_nested_relation); // Use the getter

    assert!(
        nested_relation_found,
        "Expected ResolvesToDefinition relation not found for nested_pub"
    );
}

fn get_child_mod_decl(
    tree: &ModuleTree,
    root_mod_id: ModuleNodeId,
    name: &str,
) -> Option<ModuleNode> {
    let root_id_any = root_mod_id.as_any();
    let top_pub_mod_decl_node = tree
        .get_iter_relations_from(&root_id_any)
        .expect("root module for fixture to have at least one relation")
        .map(|tr| tr.rel())
        .filter_map(|r: &SyntacticRelation| -> Option<PrimaryNodeId> {
            r.contains_target(root_mod_id)
        })
        .map(ModuleNodeId::try_from)
        .filter_map(|result| result.ok())
        .filter_map(|m_id| tree.get_module_checked(&m_id).ok().cloned())
        .find(|m| m.name == name);
    top_pub_mod_decl_node
}

fn module_path_vec(path: &[&str]) -> Vec<String> {
    path.iter().map(|seg| (*seg).to_string()).collect()
}

fn find_module_id_by_path(
    graph: &ParsedCodeGraph,
    tree: &ModuleTree,
    path: &[&str],
) -> ModuleNodeId {
    let path_vec = module_path_vec(path);
    let module = graph
        .find_module_by_path_checked(&path_vec)
        .expect("module path should exist in parsed graph");
    let module_id = module.id;
    assert!(
        tree.modules().contains_key(&module_id),
        "module tree should also contain module {:?}",
        path_vec
    );
    module_id
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
    let (graph, tree) = build_tree_for_tests(fixture_name);

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

    let imports_module_id = find_module_id_by_path(&graph, &tree, &["crate", "imports"]);
    let export_paths: HashSet<String> = tree
        .pending_exports()
        .iter()
        .filter(|export| export.containing_mod_id() == imports_module_id)
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
    // `imports.rs` currently exposes three variants: public alias (`TraitsMod`), crate-visible alias
    // (`CrateVisibleStruct`), and the scoped alias we use for `RestrictedTraitAlias`.
    let expected_export_paths: HashSet<&str> = HashSet::from([
        "crate::traits",
        "crate::structs::SampleStruct",
        "crate::traits::SimpleTrait",
    ]);
    assert_eq!(
        export_paths.len(),
        expected_export_paths.len(),
        "Expected {:?} pending exports from imports.rs, found: {:?}",
        expected_export_paths,
        export_paths
    );
    for expected in expected_export_paths {
        assert!(
            export_paths.contains(expected),
            "Missing expected export `{expected}`; exports: {:?}",
            export_paths
        );
    }

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
    let (graph, tree) = build_tree_for_tests(fixture_name);

    // --- Check Pending Exports ---
    // The fixture_nodes/imports.rs contains one `pub use` statement: `pub use crate::traits as TraitsMod;`
    let imports_module_id = find_module_id_by_path(&graph, &tree, &["crate", "imports"]);
    let export_paths: Vec<String> = tree
        .pending_exports()
        .iter()
        .filter(|export| export.containing_mod_id() == imports_module_id)
        .map(|p| p.export_node().source_path.join("::"))
        .collect();
    let export_path_set: HashSet<String> = export_paths.into_iter().collect();
    let expected_export_paths: HashSet<String> = [
        "crate::traits",
        "crate::structs::SampleStruct",
        "crate::traits::SimpleTrait",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(
        export_path_set, expected_export_paths,
        "Expected pending exports from fixture_nodes/imports.rs to be {:?}",
        expected_export_paths
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
        ("crate::structs::UnitStruct".to_string(), false, false),
        ("std::collections::HashMap".to_string(), false, false),
        ("std::fmt".to_string(), false, false),
        ("std::sync::Arc".to_string(), false, false),
        ("crate::structs::SampleStruct".to_string(), false, false), // Renamed
        ("crate::structs::CfgOnlyStruct".to_string(), false, false), // CFG-gated alias
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
        ("crate::traits".to_string(), true, false), // Local glob import (`use crate::traits::*;`)
        ("self::sub_imports::SubItem".to_string(), false, false), // Relative self
        ("super::structs::AttributedStruct".to_string(), false, false), // Relative super
        ("crate::type_alias::SimpleId".to_string(), false, false), // Relative crate
        ("::std::time::Duration".to_string(), false, false), // Absolute path
        ("serde".to_string(), false, true),    // Extern crate
        (
            "crate::enums::SampleEnum1::Variant1".to_string(),
            false,
            false,
        ), // Enum variant
        (
            "crate::const_static::TOP_LEVEL_BOOL".to_string(),
            false,
            false,
        ), // Const
        (
            "crate::const_static::TOP_LEVEL_COUNTER".to_string(),
            false,
            false,
        ), // Static
        ("crate::unions::IntOrFloat".to_string(), false, false), // Union
        ("crate::macros::documented_macro".to_string(), false, false), // Macro
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
        // --- From new_test_module.rs -> tests module (`use super::*;`) ---
        ("super".to_string(), true, false),
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
    assert!(!hashmap_import.is_any_reexport());
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
    assert_eq!(glob_import.visible_name, "std::env::*");

    let super_glob_import = tree
        .pending_imports()
        .iter()
        .find(|p| p.import_node().is_glob && p.import_node().source_path.join("::") == "super")
        .map(|p| p.import_node())
        .expect("Glob import 'super::*' not found");
    assert_eq!(super_glob_import.visible_name, "super::*");
    assert!(super_glob_import.is_inherited_use());

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

/// Pending-feature test: expects a backlink from a local definition to its import site.
/// This should only pass once a relation exists that links a defining node to the ImportNode
/// (e.g., StructNodeId -> ImportNodeId) for re-exports/imports.
#[test]
#[ignore = "Backlink relation not yet implemented"]
fn expect_backlink_from_definition_to_import_for_sample_struct() {
    let fixture_name = "fixture_nodes";
    let (graph, tree) = build_tree_for_tests(fixture_name);

    // Locate the defining StructNode for SampleStruct.
    let sample_struct_id = graph
        .defined_types()
        .iter()
        .find_map(|def| match def {
            syn_parser::parser::nodes::TypeDefNode::Struct(s) if s.name == "SampleStruct" => {
                Some(s.id)
            }
            _ => None,
        })
        .expect("SampleStruct definition not found in fixture_nodes");

    // Locate the ImportNode for `use crate::structs::SampleStruct as MySimpleStruct;` in crate::imports.
    let imports_module = graph
        .find_module_by_path_checked(&["crate".to_string(), "imports".to_string()])
        .expect("imports module not found in fixture_nodes");
    let my_simple_struct_import = imports_module
        .imports
        .iter()
        .find(|imp| imp.visible_name == "MySimpleStruct")
        .expect("MySimpleStruct import not found in imports module");
    let import_any_id = syn_parser::parser::nodes::AnyNodeId::from(my_simple_struct_import.id);

    // Expect a relation that points from the definition to the import site.
    let has_backlink = tree.tree_relations().iter().any(|tr| {
        tr.rel().source() == syn_parser::parser::nodes::AnyNodeId::from(sample_struct_id)
            && tr.rel().target() == import_any_id
    });

    assert!(
        has_backlink,
        "Expected a relation linking definition SampleStruct -> import MySimpleStruct; implement the backlink relation to satisfy this test."
    );
}

/// **Covers:** Basic visibility checks using `ModuleTree::is_accessible`.
/// It uses the `file_dir_detection` fixture to test access between modules
use std::io::Write; // Import Write trait for formatting

/// with different visibility levels (public, crate, restricted, inherited).
#[test]
fn test_module_tree_is_accessible() {
    // Initialize logger with custom format for this test
    let _ = env_logger::builder() // Parse RUST_LOG environment variable
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

    let top_pub_mod_id = graph
        .find_module_by_path_checked(&["crate".to_string(), "top_pub_mod".to_string()])
        .expect("Failed to find top_pub_mod")
        .id;

    let top_priv_mod_id = graph
        .find_module_by_path_checked(&["crate".to_string(), "top_priv_mod".to_string()])
        .expect("Failed to find top_priv_mod")
        .id;

    let nested_pub_in_pub_id = graph
        .find_module_by_path_checked(&[
            "crate".to_string(),
            "top_pub_mod".to_string(),
            "nested_pub".to_string(),
        ])
        .expect("Failed to find nested_pub in top_pub_mod")
        .id;

    let nested_priv_in_pub_id = graph
        .find_module_by_path_checked(&[
            "crate".to_string(),
            "top_pub_mod".to_string(),
            "nested_priv".to_string(),
        ])
        .expect("Failed to find nested_priv in top_pub_mod")
        .id;

    let nested_pub_in_priv_id = graph
        .find_module_by_path_checked(&[
            "crate".to_string(),
            "top_priv_mod".to_string(),
            "nested_pub_in_priv".to_string(),
        ])
        .expect("Failed to find nested_pub_in_priv")
        .id;

    let nested_priv_in_priv_id = graph
        .find_module_by_path_checked(&[
            "crate".to_string(),
            "top_priv_mod".to_string(),
            "nested_priv_in_priv".to_string(),
        ])
        .expect("Failed to find nested_priv_in_priv")
        .id;

    let path_visible_mod_id = graph
        .find_module_by_path_checked(&[
            "crate".to_string(),
            "top_pub_mod".to_string(),
            "path_visible_mod".to_string(),
        ])
        .expect("Failed to find path_visible_mod")
        .id; // This one is pub(in crate::top_pub_mod)

    // --- Debugging Step 1: Log relevant relations before assertion ---
    use colored::*; // Ensure colored is in scope for formatting
    use log::debug; // Ensure debug macro is in scope

    // Find the declaration ID for nested_pub within top_pub_mod
    let nested_pub_decl_id = graph
        .modules()
        .iter()
        .find(|module| {
            module.is_decl()
                && module.path()
                    == &[
                        "crate".to_string(),
                        "top_pub_mod".to_string(),
                        "nested_pub".to_string(),
                    ]
        })
        .expect("Declaration 'mod nested_pub;' not found in top_pub_mod.rs")
        .id;

    debug!(target: "mod_tree_vis", "{}", "--- Relation Check Start ---".dimmed().bold());
    debug!(target: "mod_tree_vis", "Checking relations involving:");
    debug!(target: "mod_tree_vis", "  - top_pub_mod (Defn): {}", top_pub_mod_id.to_string().magenta());
    debug!(target: "mod_tree_vis", "  - nested_pub (Defn):  {}", nested_pub_in_pub_id.to_string().magenta());
    debug!(target: "mod_tree_vis", "  - nested_pub (Decl):  {}", nested_pub_decl_id.to_string().magenta());

    let mut found_direct_contains = false;
    let mut found_resolves_to = false;
    let mut found_decl_contains = false;

    for tree_rel in tree.tree_relations() {
        let rel = tree_rel.rel();
        if let Some(target_mod) = rel.contains_target::<ModuleNodeId>(top_pub_mod_id) {
            if target_mod == nested_pub_in_pub_id {
                found_direct_contains = true;
            }
            if target_mod == nested_pub_decl_id {
                found_decl_contains = true;
            }
        }
        if let Some(defn) = rel.resolves_to_defn(nested_pub_decl_id) {
            if defn == nested_pub_in_pub_id {
                found_resolves_to = true;
            }
        }
    }

    debug!(target: "mod_tree_vis", "{}", "--- Relation Check Start ---".dimmed().bold());
    debug!(target: "mod_tree_vis", "    - Direct Contains ({} -> {}): {}", top_pub_mod_id, nested_pub_in_pub_id, if found_direct_contains {"Found".green()} else {"Missing".red()});
    debug!(target: "mod_tree_vis", "    - ResolvesToDefinition ({} -> {}): {}", nested_pub_decl_id, nested_pub_in_pub_id, if found_resolves_to {"Found".green()} else {"Missing".red()});
    debug!(target: "mod_tree_vis", "    - Declaration Contains ({} -> {}): {}", top_pub_mod_id, nested_pub_decl_id, if found_decl_contains {"Found".green()} else {"Missing".red()});
    debug!(target: "mod_tree_vis", "{}", "--- Relation Check End ---".dimmed().bold());

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
        !tree.is_accessible(top_priv_mod_id, top_priv_mod_id),
        "Inherited modules should fail self-access checks because visibility is evaluated from the parent context"
    );
}
