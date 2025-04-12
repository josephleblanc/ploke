#![cfg(feature = "uuid_ids")] // Gate the whole module
use crate::common::uuid_ids_utils::*;
use ploke_common::{fixtures_crates_dir, workspace_root};
use ploke_core::{NodeId, TypeId};
use std::{collections::HashMap, path::Path};
use syn_parser::parser::nodes::Attribute;
use syn_parser::parser::nodes::TypeAliasNode; // Import TypeAliasNode specifically
use syn_parser::parser::types::VisibilityKind;
use syn_parser::parser::{nodes::EnumNode, types::TypeKind}; // Import EnumNode specifically
use syn_parser::{
    discovery::{run_discovery_phase, DiscoveryOutput},
    parser::{
        analyze_files_parallel,
        graph::CodeGraph,
        nodes::{
            FieldNode, FunctionNode, ImplNode, ImportNode, ModuleNode, StructNode, TraitNode,
            TypeDefNode, ValueNode, Visible,
        },
        relations::{GraphId, Relation, RelationKind},
        types::{GenericParamKind, TypeNode},
        visitor::ParsedCodeGraph,
    },
};
// ----- paranoid helper functions ------
use crate::common::paranoid::{
    find_declaration_node_paranoid, find_file_module_node_paranoid,
    find_inline_module_node_paranoid,
};

#[test]
fn test_module_node_top_pub_mod_paranoid() {
    let fixture_name = "file_dir_detection";

    // --- Test Setup: Directly call Phase 1 & 2 ---
    // Note: Departing from run_phase1_phase2 helper to directly test analyze_files_parallel output handling.
    let crate_path = fixtures_crates_dir().join(fixture_name);
    let project_root = workspace_root(); // Use workspace root for context
    let discovery_output = run_discovery_phase(&project_root, &[crate_path.clone()])
        .expect("Phase 1 Discovery failed");

    let results_with_errors: Vec<Result<ParsedCodeGraph, syn::Error>> =
        analyze_files_parallel(&discovery_output, 0); // num_workers ignored by rayon bridge

    // Collect successful results, panicking if any file failed to parse in Phase 2
    let results: Vec<ParsedCodeGraph> = results_with_errors
        .into_iter()
        .map(|res| res.expect("Phase 2 parsing failed for a file"))
        .collect();
    // --- End Test Setup ---

    // Target: `pub mod top_pub_mod;` declared in main.rs, defined in top_pub_mod.rs
    // Definition file (where items/submodules are likely parsed)
    let definition_file = "src/top_pub_mod.rs";
    let module_path = vec!["crate".to_string(), "top_pub_mod".to_string()];
    let module_name = "top_pub_mod";
    let crate_path_vec = vec!["crate".to_string()];
    let module_path_vec = vec!["crate".to_string(), module_name.to_string()];

    // --- Find Nodes ---
    // Find the DEFINITION node (in src/top_pub_mod.rs)
    let definition_node = find_file_module_node_paranoid(
        &results,
        fixture_name,
        definition_file, // "src/top_pub_mod.rs"
        &module_path_vec,
    );

    // Find the DECLARATION node (in src/main.rs)
    let declaration_node = find_declaration_node_paranoid(
        &results,
        fixture_name,
        "src/main.rs",
        &module_path_vec, // The path of the module being declared
    );

    // --- Assertions for DEFINITION Node (src/top_pub_mod.rs) ---

    // Basic Properties
    assert_eq!(definition_node.name(), module_name);
    assert_eq!(definition_node.path, module_path_vec);
    // The file-level module node itself is considered Public within its own file context in Phase 2
    assert_eq!(definition_node.visibility(), VisibilityKind::Public);
    assert!(
        definition_node.attributes.is_empty(),
        "Expected no attributes on top_pub_mod definition node"
    );
    assert!(
        definition_node.docstring.is_none(),
        "Expected no docstring on top_pub_mod definition node"
    );
    // File-level root modules don't have a separate tracking hash in current impl
    assert!(
        definition_node.tracking_hash.is_none(),
        "Tracking hash should be None for file-level root module definition"
    );
    assert!(definition_node.is_file_based());
    assert_eq!(
        definition_node.file_path().unwrap(),
        &fixtures_crates_dir()
            .join(fixture_name)
            .join(definition_file)
    );

    // Contents (Items defined in top_pub_mod.rs)
    let definition_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(definition_file))
        .expect("Graph for definition file not found");
    let definition_graph = &definition_graph_data.graph;

    // Find item IDs expected to be defined *directly* within top_pub_mod.rs
    let func_id = find_node_id_by_path_and_name(definition_graph, &module_path_vec, "top_pub_func")
        .expect("Failed to find NodeId for top_pub_func");
    let priv_func_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "top_pub_priv_func")
            .expect("Failed to find NodeId for top_pub_priv_func");
    let duplicate_func_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "duplicate_name")
            .expect("Failed to find NodeId for duplicate_name in top_pub_mod");

    // Find submodule declaration IDs within top_pub_mod.rs
    let nested_pub_decl_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "nested_pub")
            .expect("Failed to find NodeId for nested_pub module declaration");
    let nested_priv_decl_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "nested_priv")
            .expect("Failed to find NodeId for nested_priv module declaration");
    let path_vis_decl_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "path_visible_mod")
            .expect("Failed to find NodeId for path_visible_mod module declaration");

    // Check definition_node.items contains these IDs (order doesn't matter)
    let expected_item_ids = vec![
        func_id,
        priv_func_id,
        duplicate_func_id,
        nested_pub_decl_id,  // Declaration ID from top_pub_mod.rs
        nested_priv_decl_id, // Declaration ID from top_pub_mod.rs
        path_vis_decl_id,    // Declaration ID from top_pub_mod.rs
    ];
    let definition_items = definition_node
        .items()
        .expect("FileBased module node should have items");
    assert_eq!(
        definition_items.len(),
        expected_item_ids.len(),
        "Mismatch in number of items for module definition {}",
        module_name
    );
    for id in &expected_item_ids {
        assert!(
            definition_items.contains(id),
            "Expected item ID {:?} not found in module definition {}",
            id,
            module_name
        );
    }

    // Check definition_node.imports (Should be empty for top_pub_mod.rs)
    assert!(
        definition_node.imports.is_empty(),
        "Expected imports list to be empty for definition node {}",
        module_name
    );

    // Check definition_node.exports (Should be empty)
    assert!(
        definition_node.exports.is_empty(),
        "Expected exports list to be empty for definition node {}",
        module_name
    );

    // --- Assertions for DECLARATION Node (src/main.rs) ---
    assert_eq!(declaration_node.name(), module_name);
    assert_eq!(declaration_node.path, module_path_vec);
    assert_eq!(declaration_node.visibility(), VisibilityKind::Public); // Visibility from `pub mod ...;`
    assert!(
        declaration_node.attributes.is_empty(),
        "Expected no attributes on top_pub_mod declaration node"
    );
    assert!(
        declaration_node.docstring.is_none(),
        "Expected no docstring on top_pub_mod declaration node"
    );
    // Declarations don't have their own content hash
    assert!(
        declaration_node.tracking_hash.is_none(),
        "Tracking hash should be None for declaration node"
    );
    assert!(declaration_node.is_declaration());
    assert!(declaration_node.declaration_span().is_some()); // Should have the span of `mod ...;`
    assert!(declaration_node.resolved_definition().is_none()); // Not resolved in Phase 2

    // --- Relation Check (Declaration Containment) ---
    // Check that the 'crate' module (from main.rs graph) contains the declaration for this module.
    let main_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with("src/main.rs"))
        .expect("Graph for main.rs not found");
    let main_graph = &main_graph_data.graph;

    // Find the 'crate' module node (file-level root of main.rs)
    let crate_module_node = find_file_module_node_paranoid(
        &results,
        fixture_name,
        "src/main.rs",
        &crate_path_vec, // ["crate"]
    );

    // Assert the 'crate' module node contains the 'top_pub_mod' declaration node
    assert_relation_exists(
        main_graph, // Check in the graph where the declaration happens (main.rs)
        GraphId::Node(crate_module_node.id()), // Source: crate module in main.rs
        GraphId::Node(declaration_node.id()), // Target: top_pub_mod declaration in main.rs
        RelationKind::Contains,
        "Expected 'crate' module in main.rs to Contain 'top_pub_mod' declaration",
    );

    // Also check the declaration node's ID is in the crate module's items list
    assert!(
        crate_module_node
            .items()
            .expect("crate module node items failed")
            .contains(&declaration_node.id()),
        "Expected crate module items list to contain top_pub_mod declaration ID"
    );
}
