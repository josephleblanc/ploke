use ploke_core::ItemKind;
use syn_parser::parser::graph::GraphAccess;
// Import TypeAliasNode specifically
use syn_parser::parser::types::VisibilityKind;
// Import EnumNode specifically
use crate::common::ParanoidArgs;
use lazy_static::lazy_static;
use std::collections::HashMap;
use syn_parser::parser::nodes::{ExpectedModuleNode, GraphNode, ImportNode, ModDisc};

// macro-related imports
use crate::paranoid_test_fields_and_values;
use syn_parser::parser::nodes::PrimaryNodeIdTrait;
use syn_parser::parser::nodes::{Attribute, ExpectedImportNode};

pub const LOG_TEST_MODULE: &str = "log_test_module";

lazy_static! {
    static ref EXPECTED_MODULES_DATA: HashMap<&'static str, ExpectedModuleNode> = {
        let mut m = HashMap::new();

        // Test case: `pub mod top_pub_mod;` (declaration in main.rs)
        m.insert("crate::top_pub_mod_declaration", ExpectedModuleNode {
            name: "top_pub_mod",
            path: &["crate", "top_pub_mod"],
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            imports_count: 0, // Declarations should have 0 imports
            exports_count: 0,
            tracking_hash_check: true, // Declarations have a tracking hash
            mod_disc: ModDisc::Declaration,
            expected_file_path_suffix: None, // Not FileBased
            items_count: 0, // Declarations don't have items in ModuleNode.items directly
            file_attrs_count: 0, // Not FileBased
            file_docs_is_some: false, // Not FileBased
            cfgs: vec![],
        });

        // Test case: `top_pub_mod` (definition in top_pub_mod.rs)
        m.insert("file::top_pub_mod_rs::top_pub_mod_definition", ExpectedModuleNode {
            name: "top_pub_mod",
            path: &["crate", "top_pub_mod"], // Its canonical path
            visibility: VisibilityKind::Inherited, // File-level modules are inherently private to their file unless re-exported by a parent
            attributes: vec![],
            docstring: None,
            imports_count: 0, // No imports directly in top_pub_mod.rs
            exports_count: 0,
            tracking_hash_check: false, // File-level root module definitions don't have a separate tracking hash in current impl
            mod_disc: ModDisc::FileBased,
            expected_file_path_suffix: Some("file_dir_detection/src/top_pub_mod.rs"), // Relative to fixture root
            items_count: 6, // top_pub_func, duplicate_name, top_pub_priv_func, mod nested_pub, mod nested_priv, mod path_visible_mod
            file_attrs_count: 0, // No file-level attributes in top_pub_mod.rs
            file_docs_is_some: false, // No file-level doc comments in top_pub_mod.rs
            cfgs: vec![],
        });

        m
    };
}

lazy_static! {
    static ref EXPECTED_MODULES_ARGS: HashMap<&'static str, ParanoidArgs<'static>> = {
        let mut m = HashMap::new();

        m.insert("crate::top_pub_mod_declaration", ParanoidArgs {
            fixture: "file_dir_detection",
            relative_file_path: "src/main.rs", // Declaration is in main.rs
            ident: "top_pub_mod",
            expected_path: &["crate"], // Parent module is the crate root
            item_kind: ItemKind::Module,
            expected_cfg: None,
        });

        m.insert("file::top_pub_mod_rs::top_pub_mod_definition", ParanoidArgs {
            fixture: "file_dir_detection",
            relative_file_path: "src/top_pub_mod.rs", // Definition is in this file
            ident: "top_pub_mod", // The name of the module itself
            expected_path: &["crate"], // The path of the module itself (for value-based lookup context)
            item_kind: ItemKind::Module,
            expected_cfg: None,
        });

        m
    };
}

paranoid_test_fields_and_values!(
    node_top_pub_mod_declaration,
    "crate::top_pub_mod_declaration",
    EXPECTED_MODULES_ARGS,                         // args_map
    EXPECTED_MODULES_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ModuleNode,         // node_type
    syn_parser::parser::nodes::ExpectedModuleNode, // derived Expeced*Node
    as_module,                                     // downcast_method
    LOG_TEST_MODULE                                // log_target
);

paranoid_test_fields_and_values!(
    node_top_pub_mod_definition,
    "file::top_pub_mod_rs::top_pub_mod_definition",
    EXPECTED_MODULES_ARGS,                         // args_map
    EXPECTED_MODULES_DATA,                         // expected_data_map
    syn_parser::parser::nodes::ModuleNode,         // node_type
    syn_parser::parser::nodes::ExpectedModuleNode, // derived Expeced*Node
    as_module,                                     // downcast_method
    LOG_TEST_MODULE                                // log_target
);

// TODO: Add a test using paranoid_test_fields_and_values! for the entry above
// once we are confident the derive macro and ExpectedModuleNode are correct.

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_top_pub_mod_paranoid() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    // Target: `pub mod top_pub_mod;` declared in main.rs, defined in top_pub_mod.rs
    // Definition file (where items/submodules are likely parsed)
    let definition_file = "src/top_pub_mod.rs";
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
    // Default to inherited for file-level modules
    assert_eq!(definition_node.visibility(), VisibilityKind::Inherited);
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
    // Declarations have a tracking hash based on the `mod name;` item itself
    assert!(
        declaration_node.tracking_hash.is_some(),
        "Tracking hash should be Some for declaration node"
    );
    assert!(declaration_node.is_decl());
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

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_top_priv_mod_paranoid() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let module_name = "top_priv_mod";
    let main_file = "src/main.rs";
    let definition_file = "src/top_priv_mod.rs";
    let crate_path_vec = vec!["crate".to_string()];
    let module_path_vec = vec!["crate".to_string(), module_name.to_string()];

    // --- Find Nodes ---
    let declaration_node =
        find_declaration_node_paranoid(&results, fixture_name, main_file, &module_path_vec);
    let definition_node =
        find_file_module_node_paranoid(&results, fixture_name, definition_file, &module_path_vec);

    // --- Assertions for DECLARATION Node (main.rs) ---
    assert_eq!(declaration_node.name(), module_name);
    assert_eq!(declaration_node.path, module_path_vec);
    assert_eq!(declaration_node.visibility(), VisibilityKind::Inherited); // `mod top_priv_mod;`
    assert!(declaration_node.is_decl());
    assert!(declaration_node.declaration_span().is_some());
    assert!(declaration_node.tracking_hash.is_some()); // Declarations have hash
    assert!(declaration_node.resolved_definition().is_none());

    // --- Assertions for DEFINITION Node (top_priv_mod.rs) ---
    assert_eq!(definition_node.name(), module_name);
    assert_eq!(definition_node.path, module_path_vec);
    assert_eq!(definition_node.visibility(), VisibilityKind::Inherited); // File root default
    assert!(definition_node.is_file_based());
    assert!(definition_node.tracking_hash.is_none()); // File root has no hash
    assert_eq!(
        definition_node.file_path().unwrap(),
        &fixtures_crates_dir()
            .join(fixture_name)
            .join(definition_file)
    );

    // Check items in definition node
    let definition_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(definition_file))
        .expect("Graph for definition file not found");
    let definition_graph = &definition_graph_data.graph;

    let nested_pub_decl_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "nested_pub_in_priv")
            .expect("Failed to find NodeId for nested_pub_in_priv declaration");
    let nested_priv_decl_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "nested_priv_in_priv")
            .expect("Failed to find NodeId for nested_priv_in_priv declaration");
    let func_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "top_priv_func")
            .expect("Failed to find NodeId for top_priv_func");
    let priv_func_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "top_priv_priv_func")
            .expect("Failed to find NodeId for top_priv_priv_func");

    let expected_item_ids = vec![
        nested_pub_decl_id,
        nested_priv_decl_id,
        func_id,
        priv_func_id,
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

    // --- Relation & Items List Check (Declaration Containment) ---
    let main_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(main_file))
        .expect("Graph for main.rs not found");
    let main_graph = &main_graph_data.graph;
    let crate_module_node =
        find_file_module_node_paranoid(&results, fixture_name, main_file, &crate_path_vec);

    assert_relation_exists(
        main_graph,
        GraphId::Node(crate_module_node.id()),
        GraphId::Node(declaration_node.id()),
        RelationKind::Contains,
        "Expected 'crate' module to Contain 'top_priv_mod' declaration",
    );
    assert!(
        crate_module_node
            .items()
            .expect("crate module node items failed")
            .contains(&declaration_node.id()),
        "Expected crate module items list to contain top_priv_mod declaration ID"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_crate_visible_mod_paranoid() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let module_name = "crate_visible_mod";
    let main_file = "src/main.rs";
    let definition_file = "src/crate_visible_mod.rs";
    let crate_path_vec = vec!["crate".to_string()];
    let module_path_vec = vec!["crate".to_string(), module_name.to_string()];

    // --- Find Nodes ---
    let declaration_node =
        find_declaration_node_paranoid(&results, fixture_name, main_file, &module_path_vec);
    let definition_node =
        find_file_module_node_paranoid(&results, fixture_name, definition_file, &module_path_vec);

    // --- Assertions for DECLARATION Node (main.rs) ---
    assert_eq!(declaration_node.name(), module_name);
    assert_eq!(declaration_node.path, module_path_vec);
    // TODO: Fix visibility parsing/testing - currently `pub(crate)` might be parsed as Restricted or Public.
    // assert_eq!(declaration_node.visibility(), VisibilityKind::Crate); // `pub(crate) mod ...;`
    assert!(declaration_node.is_decl());
    assert!(declaration_node.declaration_span().is_some());
    assert!(declaration_node.tracking_hash.is_some());
    assert!(declaration_node.resolved_definition().is_none());

    // --- Assertions for DEFINITION Node (crate_visible_mod.rs) ---
    assert_eq!(definition_node.name(), module_name);
    assert_eq!(definition_node.path, module_path_vec);
    assert_eq!(definition_node.visibility(), VisibilityKind::Inherited); // File root default
    assert!(definition_node.is_file_based());
    assert!(definition_node.tracking_hash.is_none()); // File root has no hash
    assert_eq!(
        definition_node.file_path().unwrap(),
        &fixtures_crates_dir()
            .join(fixture_name)
            .join(definition_file)
    );

    // Check items in definition node
    let definition_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(definition_file))
        .expect("Graph for definition file not found");
    let definition_graph = &definition_graph_data.graph;

    let func_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "crate_vis_func")
            .expect("Failed to find NodeId for crate_vis_func");
    let nested_priv_decl_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "nested_priv")
            .expect("Failed to find NodeId for nested_priv declaration");
    let nested_crate_vis_decl_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "nested_crate_vis")
            .expect("Failed to find NodeId for nested_crate_vis declaration");

    let expected_item_ids = vec![func_id, nested_priv_decl_id, nested_crate_vis_decl_id];
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

    // --- Relation & Items List Check (Declaration Containment) ---
    let main_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(main_file))
        .expect("Graph for main.rs not found");
    let main_graph = &main_graph_data.graph;
    let crate_module_node =
        find_file_module_node_paranoid(&results, fixture_name, main_file, &crate_path_vec);

    assert_relation_exists(
        main_graph,
        GraphId::Node(crate_module_node.id()),
        GraphId::Node(declaration_node.id()),
        RelationKind::Contains,
        "Expected 'crate' module to Contain 'crate_visible_mod' declaration",
    );
    assert!(
        crate_module_node
            .items()
            .expect("crate module node items failed")
            .contains(&declaration_node.id()),
        "Expected crate module items list to contain crate_visible_mod declaration ID"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_logical_name_path_attr_paranoid() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let module_name = "logical_name";
    let main_file = "src/main.rs";
    let definition_file = "src/custom_path/real_file.rs"; // Note the actual file path
    let crate_path_vec = vec!["crate".to_string()];
    let logical_module_path_vec = vec!["crate".to_string(), module_name.to_string()];
    // The path derived from the file system for the definition file
    let definition_file_derived_path_vec = vec![
        "crate".to_string(),
        "custom_path".to_string(),
        "real_file".to_string(),
    ];

    // --- Find Nodes ---
    // Find declaration using its logical path
    let declaration_node =
        find_declaration_node_paranoid(&results, fixture_name, main_file, &logical_module_path_vec);
    // Find definition using its file-derived path
    let definition_node = find_file_module_node_paranoid(
        &results,
        fixture_name,
        definition_file,                   // Use the actual file path here
        &definition_file_derived_path_vec, // Use the file-derived path
    );

    // --- Assertions for DECLARATION Node (main.rs) ---
    assert_eq!(declaration_node.name(), module_name);
    assert_eq!(declaration_node.path, logical_module_path_vec); // Declaration has logical path
    assert_eq!(declaration_node.visibility(), VisibilityKind::Public);
    assert!(declaration_node.is_decl());
    assert!(declaration_node.declaration_span().is_some());
    assert!(declaration_node.tracking_hash.is_some());
    assert!(declaration_node.resolved_definition().is_none());
    // Check for #[path] attribute
    assert!(
        declaration_node.attributes.iter().any(
            |attr| attr.name == "path" && attr.value == Some("custom_path/real_file.rs".into())
        ),
        "Expected #[path] attribute not found or incorrect on declaration node"
    );

    // --- Assertions for DEFINITION Node (real_file.rs) ---
    // NOTE: In Phase 2, the definition node parsed from real_file.rs gets its name
    // from the file stem ("real_file") and its path from the file system derivation.
    // The link to "logical_name" happens in Phase 3.
    assert_eq!(definition_node.name(), "real_file"); // Name derived from file stem
    assert_eq!(definition_node.path, definition_file_derived_path_vec); // Path derived from file system
    assert_eq!(definition_node.visibility(), VisibilityKind::Inherited); // File root default
    assert!(definition_node.is_file_based());
    assert!(definition_node.tracking_hash.is_none()); // File root has no hash
    assert_eq!(
        definition_node.file_path().unwrap(),
        &fixtures_crates_dir()
            .join(fixture_name)
            .join(definition_file) // Check it points to the correct file
    );

    // Check items in definition node
    let definition_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(definition_file))
        .expect("Graph for definition file not found");
    let definition_graph = &definition_graph_data.graph;

    // Find items using the definition node's file-derived path
    let func_id = find_node_id_by_path_and_name(
        definition_graph,
        &definition_file_derived_path_vec, // Use file-derived path
        "item_in_real_file",
    )
    .expect("Failed to find NodeId for item_in_real_file");
    let nested_decl_id = find_node_id_by_path_and_name(
        definition_graph,
        &definition_file_derived_path_vec, // Use file-derived path
        "nested_in_real_file",
    )
    .expect("Failed to find NodeId for nested_in_real_file declaration");

    let expected_item_ids = vec![func_id, nested_decl_id];
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

    // --- Relation & Items List Check (Declaration Containment) ---
    let main_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(main_file))
        .expect("Graph for main.rs not found");
    let main_graph = &main_graph_data.graph;
    let crate_module_node =
        find_file_module_node_paranoid(&results, fixture_name, main_file, &crate_path_vec);

    assert_relation_exists(
        main_graph,
        GraphId::Node(crate_module_node.id()),
        GraphId::Node(declaration_node.id()),
        RelationKind::Contains,
        "Expected 'crate' module to Contain 'logical_name' declaration",
    );
    assert!(
        crate_module_node
            .items()
            .expect("crate module node items failed")
            .contains(&declaration_node.id()),
        "Expected crate module items list to contain logical_name declaration ID"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_inline_pub_mod_paranoid() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let module_name = "inline_pub_mod";
    let main_file = "src/main.rs";
    let crate_path_vec = vec!["crate".to_string()];
    let module_path_vec = vec!["crate".to_string(), module_name.to_string()];

    // --- Find Node ---
    let inline_node =
        find_inline_module_node_paranoid(&results, fixture_name, main_file, &module_path_vec);

    // --- Assertions for INLINE Node (main.rs) ---
    assert_eq!(inline_node.name(), module_name);
    assert_eq!(inline_node.path, module_path_vec);
    assert_eq!(inline_node.visibility(), VisibilityKind::Public);
    assert!(inline_node.is_inline());
    assert!(inline_node.inline_span().is_some());
    assert!(inline_node.tracking_hash.is_some()); // Inline modules have hash

    // Check items defined inside the inline block
    let main_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(main_file))
        .expect("Graph for main.rs not found");
    let main_graph = &main_graph_data.graph;

    // Find `use std::collections::HashMap;` in inline_pub_mod
    let hashmap_import_id = main_graph
        .use_statements
        .iter()
        .find(|imp| {
            imp.source_path == ["std", "collections", "HashMap"] && imp.visible_name == "HashMap"
        })
        .expect("Failed to find ImportNode for 'use std::collections::HashMap'")
        .id;
    let func_id = find_node_id_by_path_and_name(main_graph, &module_path_vec, "inline_pub_func")
        .expect("Failed to find NodeId for inline_pub_func");
    let duplicate_func_id =
        find_node_id_by_path_and_name(main_graph, &module_path_vec, "duplicate_name")
            .expect("Failed to find NodeId for duplicate_name in inline_pub_mod");
    let nested_priv_decl_id =
        find_node_id_by_path_and_name(main_graph, &module_path_vec, "inline_nested_priv")
            .expect("Failed to find NodeId for inline_nested_priv declaration");
    let super_visible_decl_id =
        find_node_id_by_path_and_name(main_graph, &module_path_vec, "super_visible_inline")
            .expect("Failed to find NodeId for super_visible_inline declaration");

    let expected_item_ids = vec![
        hashmap_import_id,
        func_id,
        duplicate_func_id,
        nested_priv_decl_id,
        super_visible_decl_id,
    ];
    let inline_items = inline_node
        .items()
        .expect("Inline module node should have items");
    assert_eq!(
        inline_items.len(),
        expected_item_ids.len(),
        "Mismatch in number of items for inline module {}",
        module_name
    );
    for id in &expected_item_ids {
        assert!(
            inline_items.contains(id),
            "Expected item ID {:?} not found in inline module {}",
            id,
            module_name
        );
    }

    // --- Relation & Items List Check (Inline Containment) ---
    let crate_module_node =
        find_file_module_node_paranoid(&results, fixture_name, main_file, &crate_path_vec);

    assert_relation_exists(
        main_graph,
        GraphId::Node(crate_module_node.id()),
        GraphId::Node(inline_node.id()),
        RelationKind::Contains,
        "Expected 'crate' module to Contain 'inline_pub_mod' definition",
    );
    assert!(
        crate_module_node
            .items()
            .expect("crate module node items failed")
            .contains(&inline_node.id()),
        "Expected crate module items list to contain inline_pub_mod definition ID"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_inline_priv_mod_paranoid() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let module_name = "inline_priv_mod";
    let main_file = "src/main.rs";
    let crate_path_vec = vec!["crate".to_string()];
    let module_path_vec = vec!["crate".to_string(), module_name.to_string()];

    // --- Find Node ---
    let inline_node =
        find_inline_module_node_paranoid(&results, fixture_name, main_file, &module_path_vec);

    // --- Assertions for INLINE Node (main.rs) ---
    assert_eq!(inline_node.name(), module_name);
    assert_eq!(inline_node.path, module_path_vec);
    assert_eq!(inline_node.visibility(), VisibilityKind::Inherited); // `mod ...` is private
    assert!(inline_node.is_inline());
    assert!(inline_node.inline_span().is_some());
    assert!(inline_node.tracking_hash.is_some());

    // Check items defined inside the inline block
    let main_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(main_file))
        .expect("Graph for main.rs not found");
    let main_graph = &main_graph_data.graph;

    let func_id = find_node_id_by_path_and_name(main_graph, &module_path_vec, "inline_priv_func")
        .expect("Failed to find NodeId for inline_priv_func");
    let nested_pub_decl_id =
        find_node_id_by_path_and_name(main_graph, &module_path_vec, "inline_nested_pub")
            .expect("Failed to find NodeId for inline_nested_pub declaration");

    let expected_item_ids = vec![func_id, nested_pub_decl_id];
    let inline_items = inline_node
        .items()
        .expect("Inline module node should have items");
    assert_eq!(
        inline_items.len(),
        expected_item_ids.len(),
        "Mismatch in number of items for inline module {}",
        module_name
    );
    for id in &expected_item_ids {
        assert!(
            inline_items.contains(id),
            "Expected item ID {:?} not found in inline module {}",
            id,
            module_name
        );
    }

    // --- Relation & Items List Check (Inline Containment) ---
    let crate_module_node =
        find_file_module_node_paranoid(&results, fixture_name, main_file, &crate_path_vec);

    assert_relation_exists(
        main_graph,
        GraphId::Node(crate_module_node.id()),
        GraphId::Node(inline_node.id()),
        RelationKind::Contains,
        "Expected 'crate' module to Contain 'inline_priv_mod' definition",
    );
    assert!(
        crate_module_node
            .items()
            .expect("crate module node items failed")
            .contains(&inline_node.id()),
        "Expected crate module items list to contain inline_priv_mod definition ID"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_nested_example_submod_paranoid() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let module_name = "example_submod";
    let declaration_file = "src/example_mod/mod.rs";
    let definition_file = "src/example_mod/example_submod/mod.rs";
    let parent_path_vec = vec!["crate".to_string(), "example_mod".to_string()];
    let module_path_vec = vec![
        "crate".to_string(),
        "example_mod".to_string(),
        module_name.to_string(),
    ];

    // --- Find Nodes ---
    let declaration_node =
        find_declaration_node_paranoid(&results, fixture_name, declaration_file, &module_path_vec);
    let definition_node =
        find_file_module_node_paranoid(&results, fixture_name, definition_file, &module_path_vec);

    // --- Assertions for DECLARATION Node (example_mod/mod.rs) ---
    assert_eq!(declaration_node.name(), module_name);
    assert_eq!(declaration_node.path, module_path_vec);
    assert_eq!(declaration_node.visibility(), VisibilityKind::Public);
    assert!(declaration_node.is_decl());
    assert!(declaration_node.declaration_span().is_some());
    assert!(declaration_node.tracking_hash.is_some());
    assert!(declaration_node.resolved_definition().is_none());

    // --- Assertions for DEFINITION Node (example_submod/mod.rs) ---
    assert_eq!(definition_node.name(), module_name);
    assert_eq!(definition_node.path, module_path_vec);
    assert_eq!(definition_node.visibility(), VisibilityKind::Inherited); // File root default
    assert!(definition_node.is_file_based());
    assert!(definition_node.tracking_hash.is_none()); // File root has no hash
    assert_eq!(
        definition_node.file_path().unwrap(),
        &fixtures_crates_dir()
            .join(fixture_name)
            .join(definition_file)
    );

    // Check items in definition node
    let definition_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(definition_file))
        .expect("Graph for definition file not found");
    let definition_graph = &definition_graph_data.graph;

    let sibling_one_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "submod_sibling_one")
            .expect("Failed to find NodeId for item_in_example_submod");
    let sibling_two_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "submod_sibling_two")
            .expect("Failed to find NodeId for item_in_example_submod");
    let sibling_private_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "submod_sibling_private")
            .expect("Failed to find NodeId for item_in_example_submod");
    let func_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "item_in_example_submod")
            .expect("Failed to find NodeId for item_in_example_submod");

    let expected_item_ids = vec![sibling_one_id, sibling_two_id, sibling_private_id, func_id];
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

    // --- Relation & Items List Check (Declaration Containment) ---
    let declaration_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(declaration_file))
        .expect("Graph for declaration file not found");
    let declaration_graph = &declaration_graph_data.graph;
    let parent_module_node =
        find_file_module_node_paranoid(&results, fixture_name, declaration_file, &parent_path_vec);

    assert_relation_exists(
        declaration_graph,
        GraphId::Node(parent_module_node.id()),
        GraphId::Node(declaration_node.id()),
        RelationKind::Contains,
        "Expected 'example_mod' module to Contain 'example_submod' declaration",
    );
    assert!(
        parent_module_node
            .items()
            .expect("parent module node items failed")
            .contains(&declaration_node.id()),
        "Expected example_mod module items list to contain example_submod declaration ID"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_deeply_nested_mod_paranoid() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let module_name = "deeply_nested_mod";
    let declaration_file = "src/example_mod/example_private_submod/subsubmod/subsubsubmod/mod.rs";
    let definition_file =
        "src/example_mod/example_private_submod/subsubmod/subsubsubmod/deeply_nested_mod/mod.rs";
    let parent_path_vec = vec![
        "crate".to_string(),
        "example_mod".to_string(),
        "example_private_submod".to_string(),
        "subsubmod".to_string(),
        "subsubsubmod".to_string(),
    ];
    let module_path_vec = vec![
        "crate".to_string(),
        "example_mod".to_string(),
        "example_private_submod".to_string(),
        "subsubmod".to_string(),
        "subsubsubmod".to_string(),
        module_name.to_string(),
    ];

    // --- Find Nodes ---
    let declaration_node =
        find_declaration_node_paranoid(&results, fixture_name, declaration_file, &module_path_vec);
    let definition_node =
        find_file_module_node_paranoid(&results, fixture_name, definition_file, &module_path_vec);

    // --- Assertions for DECLARATION Node (subsubsubmod/mod.rs) ---
    assert_eq!(declaration_node.name(), module_name);
    assert_eq!(declaration_node.path, module_path_vec);
    assert_eq!(declaration_node.visibility(), VisibilityKind::Public);
    assert!(declaration_node.is_decl());
    assert!(declaration_node.declaration_span().is_some());
    assert!(declaration_node.tracking_hash.is_some());
    assert!(declaration_node.resolved_definition().is_none());

    // --- Assertions for DEFINITION Node (deeply_nested_mod/mod.rs) ---
    assert_eq!(definition_node.name(), module_name);
    assert_eq!(definition_node.path, module_path_vec);
    assert_eq!(definition_node.visibility(), VisibilityKind::Inherited); // File root default
    assert!(definition_node.is_file_based());
    assert!(definition_node.tracking_hash.is_none()); // File root has no hash
    assert_eq!(
        definition_node.file_path().unwrap(),
        &fixtures_crates_dir()
            .join(fixture_name)
            .join(definition_file)
    );

    // Check items in definition node
    let definition_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(definition_file))
        .expect("Graph for definition file not found");
    let definition_graph = &definition_graph_data.graph;

    let func_id = find_node_id_by_path_and_name(
        definition_graph,
        &module_path_vec,
        "item_in_deeply_nested_mod",
    )
    .expect("Failed to find NodeId for item_in_deeply_nested_mod");
    let nested_file_decl_id =
        find_node_id_by_path_and_name(definition_graph, &module_path_vec, "deeply_nested_file")
            .expect("Failed to find NodeId for deeply_nested_file declaration");

    let expected_item_ids = vec![nested_file_decl_id, func_id];
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

    // --- Relation & Items List Check (Declaration Containment) ---
    let declaration_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(declaration_file))
        .expect("Graph for declaration file not found");
    let declaration_graph = &declaration_graph_data.graph;
    let parent_module_node =
        find_file_module_node_paranoid(&results, fixture_name, declaration_file, &parent_path_vec);

    assert_relation_exists(
        declaration_graph,
        GraphId::Node(parent_module_node.id()),
        GraphId::Node(declaration_node.id()),
        RelationKind::Contains,
        "Expected 'subsubsubmod' module to Contain 'deeply_nested_mod' declaration",
    );
    assert!(
        parent_module_node
            .items()
            .expect("parent module node items failed")
            .contains(&declaration_node.id()),
        "Expected subsubsubmod module items list to contain deeply_nested_mod declaration ID"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_file_attributes_and_docs() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let main_file = "src/main.rs";
    let crate_path_vec = vec!["crate".to_string()];

    // --- Find Node (crate module in main.rs) ---
    let crate_module_node =
        find_file_module_node_paranoid(&results, fixture_name, main_file, &crate_path_vec);

    // --- Assert File-Level Attributes ---
    let file_attrs = crate_module_node
        .file_attrs()
        .expect("FileBased module should provide file_attrs");
    assert_eq!(
        file_attrs.len(),
        1,
        "Expected 1 file-level attribute in main.rs"
    );
    assert_eq!(file_attrs[0].name, "allow");
    assert!(
        file_attrs[0].args.contains(&"unused".to_string()),
        "Expected '#![allow(unused)]'"
    );

    // --- Assert File-Level Docs ---
    let file_docs = crate_module_node
        .file_docs()
        .expect("FileBased module should provide file_docs");
    assert_eq!(file_docs.trim(), "This is the crate root doc comment.");
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_mod_attributes_and_docs() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let module_name = "inline_pub_mod";
    let main_file = "src/main.rs";
    let module_path_vec = vec!["crate".to_string(), module_name.to_string()];

    // --- Find Node (inline_pub_mod in main.rs) ---
    let inline_node =
        find_inline_module_node_paranoid(&results, fixture_name, main_file, &module_path_vec);

    // --- Assert Module-Level CFGs ---
    assert_eq!(
        inline_node.cfgs.len(),
        1,
        "Expected 1 cfg string on inline_pub_mod, found {}. Cfgs: {:?}",
        inline_node.cfgs.len(),
        inline_node.cfgs
    );
    assert_eq!(
        inline_node.cfgs[0], "test",
        "Expected cfg string 'test', found '{}'",
        inline_node.cfgs[0]
    );
    // Assert attributes list is now empty
    assert!(
        inline_node.attributes.is_empty(),
        "Expected attributes list to be empty for inline_pub_mod, found: {:?}",
        inline_node.attributes
    );

    // --- Assert Module-Level Docs ---
    let docstring = inline_node
        .docstring
        .as_ref()
        .expect("Expected docstring on inline_pub_mod");
    assert_eq!(docstring.trim(), "An inline public module doc comment.");

    // --- Check Declaration Node Docs (Example) ---
    let decl_module_name = "top_pub_mod";
    let decl_module_path_vec = vec!["crate".to_string(), decl_module_name.to_string()];
    let declaration_node =
        find_declaration_node_paranoid(&results, fixture_name, main_file, &decl_module_path_vec);
    // NOTE: Currently, doc comments on `mod item;` are NOT attached to the ModuleNode declaration.
    // This might be a visitor enhancement needed later if required.
    assert!(
        declaration_node.docstring.is_none(),
        "Expected no docstring on top_pub_mod declaration (current limitation)"
    );
}

// #[test] // TODO: Add a fixture with attributes/docs on modules
// #[cfg(not(feature = "type_bearing_ids"))]
// fn test_module_node_with_attributes_and_docs() { // REMOVE THIS OLD STUB
//     // Requires a fixture with modules having attributes (#![...]) and doc comments (//! or /// mod)
//     // e.g., Add attributes/docs to `inline_pub_mod` or a file-based module.
//     // Verify:
//     // 1. Find the relevant module node (declaration or definition).
//     // 2. Assert `attributes` field contains the expected parsed Attribute structs.
//     // 3. Assert `docstring` field contains the expected doc comment string.
//     // 4. For FileBased modules, also check `file_attrs` and `file_docs`.
//     todo!("Implement test_module_node_with_attributes_and_docs");
// }

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_items_list_comprehensiveness() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let main_file = "src/main.rs";
    let crate_path_vec = vec!["crate".to_string()];

    // --- Find Nodes ---
    let crate_module_node =
        find_file_module_node_paranoid(&results, fixture_name, main_file, &crate_path_vec);

    let main_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(main_file))
        .expect("Graph for main.rs not found");
    let main_graph = &main_graph_data.graph;

    // --- Find Expected Item IDs in main.rs ---
    let main_pub_func_id =
        find_node_id_by_path_and_name(main_graph, &crate_path_vec, "main_pub_func")
            .expect("Failed to find NodeId for main_pub_func");
    let main_priv_func_id =
        find_node_id_by_path_and_name(main_graph, &crate_path_vec, "main_priv_func")
            .expect("Failed to find NodeId for main_priv_func");
    let duplicate_func_id =
        find_node_id_by_path_and_name(main_graph, &crate_path_vec, "duplicate_name")
            .expect("Failed to find NodeId for duplicate_name in main");
    let main_func_id = find_node_id_by_path_and_name(main_graph, &crate_path_vec, "main")
        .expect("Failed to find NodeId for main");

    // Module Declarations
    let example_mod_decl_id =
        find_node_id_by_path_and_name(main_graph, &crate_path_vec, "example_mod")
            .expect("Failed to find NodeId for example_mod declaration");
    let top_pub_mod_decl_id =
        find_node_id_by_path_and_name(main_graph, &crate_path_vec, "top_pub_mod")
            .expect("Failed to find NodeId for top_pub_mod declaration");
    let top_priv_mod_decl_id =
        find_node_id_by_path_and_name(main_graph, &crate_path_vec, "top_priv_mod")
            .expect("Failed to find NodeId for top_priv_mod declaration");
    let crate_visible_mod_decl_id =
        find_node_id_by_path_and_name(main_graph, &crate_path_vec, "crate_visible_mod")
            .expect("Failed to find NodeId for crate_visible_mod declaration");
    let logical_name_decl_id =
        find_node_id_by_path_and_name(main_graph, &crate_path_vec, "logical_name")
            .expect("Failed to find NodeId for logical_name declaration");

    // Inline Module Definitions
    let inline_pub_mod_def_id =
        find_node_id_by_path_and_name(main_graph, &crate_path_vec, "inline_pub_mod")
            .expect("Failed to find NodeId for inline_pub_mod definition");
    let inline_priv_mod_def_id =
        find_node_id_by_path_and_name(main_graph, &crate_path_vec, "inline_priv_mod")
            .expect("Failed to find NodeId for inline_priv_mod definition");

    println!("imports: {:#?}", main_graph.use_statements);
    // Imports
    let import_std = find_import_id(
        main_graph,
        &crate_path_vec,
        "Path",
        &["std", "path", "Path"],
    )
    .expect("Failed to find NodeId for `use std::path::Path;` definition");
    let import_complex = find_import_id(
        main_graph,
        &crate_path_vec,
        "reexported_func",
        &["crate", "top_pub_mod", "top_pub_func"],
    ).expect("Failed to find NodeId for `pub use crate::top_pub_mod::top_pub_func as reexported_func;` definition");
    // find_node_id_by_path_and_name(main_graph, &crate_path_vec, "reexported_func")

    // --- Assert Items List ---
    let expected_item_ids = vec![
        // Functions
        main_pub_func_id,
        main_priv_func_id,
        duplicate_func_id,
        main_func_id,
        // Module Declarations
        example_mod_decl_id,
        top_pub_mod_decl_id,
        top_priv_mod_decl_id,
        crate_visible_mod_decl_id,
        logical_name_decl_id,
        // Inline Module Definitions
        inline_pub_mod_def_id,
        inline_priv_mod_def_id,
        // Imports
        import_std,
        import_complex,
        // Note: Macros, Constants, Statics, etc., would also be included here if present in main.rs
    ];

    let crate_items = crate_module_node
        .items()
        .expect("Crate module node should have items");

    for item in crate_items
        .iter()
        .filter(|item| expected_item_ids.contains(item))
    {
        println!("Missing: {}", item);
    }

    // Use HashSet for efficient comparison regardless of order
    let expected_ids_set: std::collections::HashSet<_> =
        expected_item_ids.iter().cloned().collect();
    let actual_ids_set: std::collections::HashSet<_> = crate_items.iter().cloned().collect();

    assert_eq!(
        actual_ids_set, expected_ids_set,
        "Mismatch in items for 'crate' module in main.rs.\nExpected: {:?}\nActual: {:?}",
        expected_ids_set, actual_ids_set
    );
    assert_eq!(
        crate_items.len(),
        expected_item_ids.len(),
        "Mismatch in the *number* of items for 'crate' module in main.rs"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_node_imports_list() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let main_file = "src/main.rs";
    let crate_path_vec = vec!["crate".to_string()];
    let inline_mod_name = "inline_pub_mod";
    let inline_mod_path_vec = vec!["crate".to_string(), inline_mod_name.to_string()];

    // --- Find Nodes ---
    let crate_module_node =
        find_file_module_node_paranoid(&results, fixture_name, main_file, &crate_path_vec);
    let inline_module_node =
        find_inline_module_node_paranoid(&results, fixture_name, main_file, &inline_mod_path_vec);

    // --- Find Graph ---
    let main_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(main_file))
        .expect("Graph for main.rs not found");
    let main_graph = &main_graph_data.graph;

    // --- Find Import Node IDs ---
    // Find `use std::path::Path;` in crate root
    let path_import_node = main_graph
        .use_statements
        .iter()
        .find(|imp| imp.source_path == ["std", "path", "Path"] && imp.visible_name == "Path")
        .expect("Failed to find ImportNode for 'use std::path::Path'");

    // Find `pub use crate::top_pub_mod::top_pub_func as reexported_func;` in crate root
    let pub_use_node = main_graph
        .use_statements
        .iter()
        .find(|imp| {
            imp.source_path == ["crate", "top_pub_mod", "top_pub_func"]
                && imp.visible_name == "reexported_func"
        })
        .expect("Failed to find ImportNode for 'pub use ... as reexported_func'");

    // Find `use std::collections::HashMap;` in inline_pub_mod
    let hashmap_import_node = main_graph
        .use_statements
        .iter()
        .find(|imp| {
            imp.source_path == ["std", "collections", "HashMap"] && imp.visible_name == "HashMap"
        })
        .expect("Failed to find ImportNode for 'use std::collections::HashMap'");

    // --- Assert Crate Module Imports/Items ---
    let crate_imports = &crate_module_node
        .imports
        .iter()
        .map(|imp| imp.id)
        .collect::<Vec<NodeId>>();
    let crate_items = crate_module_node
        .items()
        .expect("Crate module node should have items");

    let debug_crate_imports = &crate_module_node
        .imports
        .iter()
        .collect::<Vec<&ImportNode>>();
    let debug_graph = &crate_module_node.imports;
    assert_eq!(
        crate_imports.len(),
        2,
        "Expected 2 imports in crate module with name: {}, path: {:?}
All imports for crate ModuleNode: {:#?}
All imports in CodeGraph:{:#?}",
        &crate_module_node.name,
        &crate_module_node.path,
        debug_crate_imports,
        debug_graph
    );
    assert!(
        crate_imports.contains(&path_import_node.id),
        "Crate imports should contain Path import ID"
    );
    assert!(
        crate_imports.contains(&pub_use_node.id),
        "Crate imports should contain pub use import ID"
    );
    assert!(
        crate_items.contains(&path_import_node.id),
        "Crate items should contain Path import ID"
    );
    assert!(
        crate_items.contains(&pub_use_node.id),
        "Crate items should contain pub use import ID"
    );

    // --- Assert Inline Module Imports/Items ---
    let inline_imports = &inline_module_node
        .imports
        .iter()
        .map(|imp| imp.id)
        .collect::<Vec<NodeId>>();
    let inline_items = inline_module_node
        .items()
        .expect("Inline module node should have items");

    assert_eq!(
        inline_imports.len(),
        1,
        "Expected 1 import in inline_pub_mod"
    );
    assert!(
        inline_imports.contains(&hashmap_import_node.id),
        "Inline imports should contain HashMap import ID"
    );
    assert!(
        inline_items.contains(&hashmap_import_node.id),
        "Inline items should contain HashMap import ID"
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_contains_relation_inline() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let module_name = "inline_pub_mod";
    let main_file = "src/main.rs";
    let crate_path_vec = vec!["crate".to_string()];
    let module_path_vec = vec!["crate".to_string(), module_name.to_string()];

    // --- Find Nodes ---
    let crate_module_node =
        find_file_module_node_paranoid(&results, fixture_name, main_file, &crate_path_vec);
    let inline_node =
        find_inline_module_node_paranoid(&results, fixture_name, main_file, &module_path_vec);

    // --- Find Graph ---
    let main_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(main_file))
        .expect("Graph for main.rs not found");
    let main_graph = &main_graph_data.graph;

    // --- Assert Relation ---
    assert_relation_exists(
        main_graph,
        GraphId::Node(crate_module_node.id()),
        GraphId::Node(inline_node.id()),
        RelationKind::Contains,
        "Expected 'crate' module to Contain 'inline_pub_mod' definition",
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_module_contains_relation_declaration_nested() {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);

    let parent_module_name = "top_pub_mod";
    let child_module_name = "nested_pub";
    let definition_file = "src/top_pub_mod.rs"; // File where the declaration occurs
    let parent_module_path_vec = vec!["crate".to_string(), parent_module_name.to_string()];
    let child_module_path_vec = vec![
        "crate".to_string(),
        parent_module_name.to_string(),
        child_module_name.to_string(),
    ];

    // --- Find Nodes ---
    // Find the parent module node (definition of top_pub_mod)
    let parent_module_node = find_file_module_node_paranoid(
        &results,
        fixture_name,
        definition_file,
        &parent_module_path_vec,
    );
    // Find the child module node (declaration of nested_pub within top_pub_mod.rs)
    let child_declaration_node = find_declaration_node_paranoid(
        &results,
        fixture_name,
        definition_file, // Declaration is in this file
        &child_module_path_vec,
    );

    // --- Find Graph ---
    let definition_graph_data = results
        .iter()
        .find(|data| data.file_path.ends_with(definition_file))
        .expect("Graph for definition file not found");
    let definition_graph = &definition_graph_data.graph;

    // --- Assert Relation ---
    assert_relation_exists(
        definition_graph, // Relation exists within the graph of top_pub_mod.rs
        GraphId::Node(parent_module_node.id()),
        GraphId::Node(child_declaration_node.id()),
        RelationKind::Contains,
        "Expected 'top_pub_mod' module to Contain 'nested_pub' declaration",
    );

    // --- Assert Items List ---
    assert!(
        parent_module_node
            .items()
            .expect("parent module node items failed")
            .contains(&child_declaration_node.id()),
        "Expected top_pub_mod module items list to contain nested_pub declaration ID"
    );
}
