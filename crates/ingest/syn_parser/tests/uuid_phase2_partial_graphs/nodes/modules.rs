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

    // Find the module node using the helper
    let module_node = find_module_node_paranoid(&results, fixture_name, &module_path, true);

    // --- Assertions ---

    // Basic Properties
    assert_eq!(module_node.name(), module_name);
    assert_eq!(module_node.path, module_path);
    assert_eq!(module_node.visibility(), VisibilityKind::Public); // File-level modules
                                                                  // `VisibilityKind::Inherited by default`

    assert!(
        module_node.attributes.is_empty(),
        "Expected no attributes on top_pub_mod definition"
    );
    assert!(
        module_node.docstring.is_none(),
        "Expected no docstring on top_pub_mod definiton"
    );
    assert!(
        module_node.tracking_hash.is_none(),
        "Tracking hash should be NOT present on top_pub_mod definition"
    );

    // Contents (Items and Submodules defined in top_pub_mod.rs)
    // Note: Phase 2 populates ModuleNode.items based on what's parsed *in that specific file*.
    // So, we check the graph corresponding to top_pub_mod.rs for these items.

    let definition_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(definition_file))
        .expect("Graph for definition file not found");
    let definition_graph = &definition_graph_data.graph;

    #[cfg(feature = "verbose_debug")]
    {
        println!("{:=^80}", " definition_graph.modules ");
        println!(
            "definition_graph.modules: {:#?}\n",
            definition_graph.modules
        );
        println!("{:=^80}", "top_pub_func found by name");
        println!(
            "top_pub_func: {:#?}",
            definition_graph
                .functions
                .iter()
                .find(|f| f.name == "top_pub_func"),
        );
    }
    let top_pub_module_path = vec!["crate".to_string(), "top_pub_mod".to_string()];
    let func_id_debug =
        find_node_id_by_path_and_name(definition_graph, &top_pub_module_path, "top_pub_func");
    println!(
        "find_node_id_by_path_and_name(definition_graph, &module_path_crate_only, \"top_pub_func\"): {:?}",
        func_id_debug
    );
    let find_relation = definition_graph
        .relations
        .iter()
        .find(|r| r.target == GraphId::Node(func_id_debug.unwrap()));

    #[cfg(feature = "verbose_debug")]
    println!(
        "definition_graph.relations.iter().find(|r| r.target == GraphId::Node(func_id_debug)): {:?}",
        find_relation
    );

    // Find items expected to be defined *directly* within top_pub_mod.rs
    let func_id = find_node_id_by_path_and_name(definition_graph, &module_path, "top_pub_func")
        .expect("Failed to find NodeId for top_pub_func");

    let priv_func_id =
        find_node_id_by_path_and_name(definition_graph, &module_path, "top_pub_priv_func")
            .expect("Failed to find NodeId for top_pub_priv_func");

    let duplicate_func_id =
        find_node_id_by_path_and_name(definition_graph, &module_path, "duplicate_name")
            .expect("Failed to find NodeId for duplicate_name in top_pub_mod");

    // Find submodule IDs declared within top_pub_mod.rs
    let nested_pub_mod_id = find_node_id_by_path_and_name(
        definition_graph,
        &module_path, // Parent path
        "nested_pub", // Submodule name
    )
    .expect("Failed to find NodeId for nested_pub module");

    let nested_priv_mod_id =
        find_node_id_by_path_and_name(definition_graph, &module_path, "nested_priv")
            .expect("Failed to find NodeId for nested_priv module");

    let path_vis_mod_id =
        find_node_id_by_path_and_name(definition_graph, &module_path, "path_visible_mod")
            .expect("Failed to find NodeId for path_visible_mod module");

    // Check ModuleNode.items contains these IDs (order doesn't matter)
    let expected_item_ids = vec![
        func_id,
        priv_func_id,
        duplicate_func_id,
        nested_pub_mod_id,
        nested_priv_mod_id,
        path_vis_mod_id,
    ];
    assert_eq!(
        module_node
            .items()
            .expect("Cannot take length of items for non-in-line modules")
            .len(),
        expected_item_ids.len(),
        "Mismatch in number of items for module {}",
        module_name
    );
    for id in &expected_item_ids {
        assert!(
            module_node.items().is_some_and(|m| m.contains(id)),
            "Expected item ID {:?} not found in module {}",
            id,
            module_name
        );
    }

    // Check ModuleNode.submodules (Phase 2 might not populate this reliably, check items instead)
    // Let's assert it's empty for now, as `items` is the primary check in Phase 2.
    // We might revisit this if the visitor logic changes.
    //
    // assert!(
    //     module_node.submodules.is_empty(),
    //     "Expected submodules list to be empty in Phase 2 for {}",
    //     module_name
    // ); // Old implementation, `submodules` field no longer exists.

    // Check ModuleNode.imports (Should be empty for top_pub_mod.rs)
    assert!(
        module_node.imports.is_empty(),
        "Expected imports list to be empty for {}",
        module_name
    );

    // Check ModuleNode.exports (Should be empty)
    assert!(
        module_node.exports.is_empty(),
        "Expected exports list to be empty for {}",
        module_name
    );

    // --- Basic Relation Check ---
    // Check that the 'crate' module (from main.rs graph) contains a declaration for this module
    // We only know this because we wrote the fixtures, for testing purposes this is equivalent to
    // testing that a given target module declaration, e.g. `pub mod some_mod;` exists in main.rs
    //
    // The relation from the file main.rs as a file-level module to the file-level module
    // top_pub_mod can only be known in phase 3 (since main.rs may or may not have a module
    // declaration for top_pub_mod), once we merge the partial code graphs.
    // For now, this is just a basic check that the module declaration exists in the fixture as we
    // expect.
    let main_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with("src/main.rs"))
        .expect("Graph for main.rs not found");
    let main_graph = &main_graph_data.graph;

    let main_module_debug = &main_graph_data
        .graph
        .modules
        .iter()
        .filter(|m| m.name() == "top_pub_mod" && m.is_declaration());

    let crate_module =
        find_mod_decl_by_path_and_name(main_graph, &["crate".to_string()], "top_pub_mod")
            .expect("top_pub_mod module declaration not found in main.rs graph");

    assert_relation_exists(
        main_graph, // Check in the graph where the declaration happens
        GraphId::Node(crate_module.id()),
        GraphId::Node(module_node.id()),
        RelationKind::Contains,
        "Expected 'crate' module in main.rs to Contain 'top_pub_mod'",
    );
}
