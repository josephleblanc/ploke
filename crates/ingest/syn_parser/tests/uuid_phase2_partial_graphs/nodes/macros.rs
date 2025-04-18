use crate::common::paranoid::*; // Use re-exports from paranoid mod
use crate::common::uuid_ids_utils::*;
use ploke_common::fixtures_crates_dir;
use ploke_core::{NodeId, TrackingHash};
use syn_parser::parser::nodes::{MacroKind, ProcMacroKind}; // Import macro kinds
use syn_parser::parser::types::VisibilityKind;
use syn_parser::parser::{
    graph::CodeGraph,
    nodes::{MacroNode, Visible}, // Import MacroNode
    relations::{GraphId, RelationKind},
};

// Test Plan: docs/plans/uuid_refactor/testing/macros_testing.md

// Helper function for Tier 2 tests to find a node without full paranoia
fn find_macro_node_basic<'a>(
    graph: &'a CodeGraph,
    module_path: &[String],
    macro_name: &str,
) -> &'a MacroNode {
    // Find the module node first
    let module_node = graph
        .modules
        .iter()
        .find(|m| m.defn_path() == module_path)
        .unwrap_or_else(|| {
            panic!(
                "ModuleNode not found for definition path: {:?} while looking for '{}'",
                module_path, macro_name
            )
        });

    let module_items = module_node.items().unwrap_or_else(|| {
        panic!(
            "ModuleNode {:?} ({}) does not have items (neither Inline nor FileBased?)",
            module_node.path, module_node.name,
        )
    });

    // Find the macro node by name and ensure it's in the module's items
    graph
        .macros
        .iter()
        .find(|m| m.name == macro_name && module_items.contains(&m.id()))
        .unwrap_or_else(|| {
            panic!(
                "MacroNode '{}' not found within module path {:?}",
                macro_name, module_path
            )
        })
}

// --- Tier 1: Basic Smoke Tests ---
#[test]
fn test_macro_node_basic_smoke_test_full_parse() {
    let results = run_phase1_phase2("fixture_nodes");
    assert!(!results.is_empty(), "Phase 1 & 2 failed to produce results");

    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");

    let target_data = results
        .iter()
        .find_map(|res| match res {
            Ok(data) if data.file_path == fixture_path => Some(data),
            _ => None,
        })
        .unwrap_or_else(|| panic!("ParsedCodeGraph for '{}' not found", fixture_path.display()));

    let graph = &target_data.graph;

    // (name, expected_kind_discriminant, expected_visibility_discriminant)
    let expected_items = vec![
        ("exported_macro", "Declarative", "Public"),
        ("local_macro", "Declarative", "Inherited"),
        ("documented_macro", "Declarative", "Public"),
        ("attributed_macro", "Declarative", "Public"),
        ("function_like_proc_macro", "ProcFunc", "Public"),
        ("derive_proc_macro", "ProcDerive", "Public"),
        ("attribute_proc_macro", "ProcAttr", "Public"),
        ("documented_proc_macro", "ProcDerive", "Public"), // Derive takes precedence if multiple proc attrs
        ("inner_exported_macro", "Declarative", "Public"),
        ("inner_local_macro", "Declarative", "Inherited"),
        ("inner_proc_macro", "ProcFunc", "Public"),
    ];

    assert!(!graph.macros.is_empty(), "CodeGraph contains no MacroNodes");

    for (name, kind_disc, vis_disc) in expected_items {
        let node = graph
            .macros
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("MacroNode '{}' not found in graph.macros", name));

        assert!(
            matches!(node.id, NodeId::Synthetic(_)),
            "Node '{}': ID should be Synthetic, found {:?}",
            name,
            node.id
        );
        assert!(
            matches!(node.tracking_hash, Some(TrackingHash(_))),
            "Node '{}': tracking_hash should be Some(TrackingHash), found {:?}",
            name,
            node.tracking_hash
        );

        // Check Kind Discriminant
        match (&node.kind, kind_disc) {
            (MacroKind::DeclarativeMacro, "Declarative") => {} // Match
            (
                MacroKind::ProcedureMacro {
                    kind: ProcMacroKind::Function,
                },
                "ProcFunc",
            ) => {} // Match
            (
                MacroKind::ProcedureMacro {
                    kind: ProcMacroKind::Derive,
                },
                "ProcDerive",
            ) => {} // Match
            (
                MacroKind::ProcedureMacro {
                    kind: ProcMacroKind::Attribute,
                },
                "ProcAttr",
            ) => {} // Match
            _ => panic!(
                "Node '{}': Kind mismatch. Expected discriminant '{}', found {:?}",
                name, kind_disc, node.kind
            ),
        }

        // Check Visibility Discriminant
        match (&node.visibility, vis_disc) {
            (VisibilityKind::Public, "Public") => {}       // Match
            (VisibilityKind::Inherited, "Inherited") => {} // Match
            _ => panic!(
                "Node '{}': Visibility mismatch. Expected discriminant '{}', found {:?}",
                name, vis_disc, node.visibility
            ),
        }
    }
}

// --- Tier 2: Targeted Field Verification ---

#[test]
fn test_macro_node_field_id_regeneration() {
    // Target: exported_macro
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let crate_namespace = target_data.crate_namespace;
    let file_path = &target_data.file_path;
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "exported_macro";

    let node = find_macro_node_basic(graph, &module_path, macro_name);
    // let actual_span = node.span; // Span no longer used

    // All macros use ItemKind::Macro for ID generation
    let item_kind = ploke_core::ItemKind::Macro;

    // Find the containing module node to get its ID for the parent scope
    let module_node = graph
        .modules
        .iter()
        .find(|m| m.defn_path() == module_path)
        .unwrap_or_else(|| {
            panic!(
                "ModuleNode not found for path: {:?} in file '{}' while testing '{}'",
                module_path,
                file_path.display(),
                macro_name
            )
        });

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path,
        &module_path,
        macro_name,
        item_kind,            // Pass ItemKind::Macro
        Some(module_node.id), // Pass the containing module's ID
        None,                 // Assume no relevant CFGs for this test case
    );

    assert!(
        matches!(node.id, NodeId::Synthetic(_)),
        "Node '{}': ID should be Synthetic, found {:?}",
        macro_name,
        node.id
    );
    assert_eq!(
        node.id, regenerated_id,
        "Mismatch for ID field. Expected (regen): {}, Actual: {}",
        regenerated_id, node.id
    );
}

#[test]
fn test_macro_node_field_name() {
    // Target: local_macro
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "local_macro";

    let node = find_macro_node_basic(graph, &module_path, macro_name);

    assert_eq!(
        node.name, macro_name,
        "Mismatch for name field. Expected: '{}', Actual: '{}'",
        macro_name, node.name
    );
}

#[test]
fn test_macro_node_field_visibility_public() {
    // Target: exported_macro
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "exported_macro";
    let expected_visibility = VisibilityKind::Public;

    let node = find_macro_node_basic(graph, &module_path, macro_name);

    assert_eq!(
        node.visibility, expected_visibility,
        "Mismatch for visibility field on '{}'. Expected: {:?}, Actual: {:?}",
        macro_name, expected_visibility, node.visibility
    );
}

#[test]
fn test_macro_node_field_visibility_inherited() {
    // Target: local_macro
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "local_macro";
    let expected_visibility = VisibilityKind::Inherited;

    let node = find_macro_node_basic(graph, &module_path, macro_name);

    assert_eq!(
        node.visibility, expected_visibility,
        "Mismatch for visibility field on '{}'. Expected: {:?}, Actual: {:?}",
        macro_name, expected_visibility, node.visibility
    );
}

#[test]
fn test_macro_node_field_kind_declarative() {
    // Target: exported_macro
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "exported_macro";
    let expected_kind = MacroKind::DeclarativeMacro;

    let node = find_macro_node_basic(graph, &module_path, macro_name);

    assert_eq!(
        node.kind, expected_kind,
        "Mismatch for kind field on '{}'. Expected: {:?}, Actual: {:?}",
        macro_name, expected_kind, node.kind
    );
}

#[test]
fn test_macro_node_field_kind_proc_func() {
    // Target: function_like_proc_macro
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "function_like_proc_macro";
    let expected_kind = MacroKind::ProcedureMacro {
        kind: ProcMacroKind::Function,
    };

    let node = find_macro_node_basic(graph, &module_path, macro_name);

    assert_eq!(
        node.kind, expected_kind,
        "Mismatch for kind field on '{}'. Expected: {:?}, Actual: {:?}",
        macro_name, expected_kind, node.kind
    );
}

#[test]
fn test_macro_node_field_kind_proc_derive() {
    // Target: derive_proc_macro
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "derive_proc_macro";
    let expected_kind = MacroKind::ProcedureMacro {
        kind: ProcMacroKind::Derive,
    };

    let node = find_macro_node_basic(graph, &module_path, macro_name);

    assert_eq!(
        node.kind, expected_kind,
        "Mismatch for kind field on '{}'. Expected: {:?}, Actual: {:?}",
        macro_name, expected_kind, node.kind
    );
}

#[test]
fn test_macro_node_field_kind_proc_attr() {
    // Target: attribute_proc_macro
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "attribute_proc_macro";
    let expected_kind = MacroKind::ProcedureMacro {
        kind: ProcMacroKind::Attribute,
    };

    let node = find_macro_node_basic(graph, &module_path, macro_name);

    assert_eq!(
        node.kind, expected_kind,
        "Mismatch for kind field on '{}'. Expected: {:?}, Actual: {:?}",
        macro_name, expected_kind, node.kind
    );
}

#[test]
fn test_macro_node_field_attributes() {
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];

    // Target 1: attributed_macro (declarative)
    let macro_name1 = "attributed_macro";
    let node1 = find_macro_node_basic(graph, &module_path, macro_name1);
    assert_eq!(
        node1.attributes.len(),
        2, // #[macro_export], #[allow(...)]
        "Node '{}': Expected 2 attributes, found {}",
        macro_name1,
        node1.attributes.len()
    );
    assert!(
        node1.attributes.iter().any(|a| a.name == "macro_export"),
        "Node '{}': Missing 'macro_export' attribute",
        macro_name1
    );
    assert!(
        node1.attributes.iter().any(|a| a.name == "allow"),
        "Node '{}': Missing 'allow' attribute",
        macro_name1
    );

    // Target 2: documented_proc_macro (procedural)
    let macro_name2 = "documented_proc_macro";
    let node2 = find_macro_node_basic(graph, &module_path, macro_name2);
    assert_eq!(
        node2.attributes.len(),
        2, // #[proc_macro_derive(...)], #[deprecated]
        "Node '{}': Expected 2 attributes, found {}",
        macro_name2,
        node2.attributes.len()
    );
    assert!(
        node2
            .attributes
            .iter()
            .any(|a| a.name == "proc_macro_derive"),
        "Node '{}': Missing 'proc_macro_derive' attribute",
        macro_name2
    );
    assert!(
        node2.attributes.iter().any(|a| a.name == "deprecated"),
        "Node '{}': Missing 'deprecated' attribute",
        macro_name2
    );
}

#[test]
fn test_macro_node_field_docstring() {
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];

    // Target 1: documented_macro (declarative)
    let macro_name1 = "documented_macro";
    let node1 = find_macro_node_basic(graph, &module_path, macro_name1);
    assert!(
        node1.docstring.is_some(),
        "Node '{}': Expected docstring, found None",
        macro_name1
    );
    assert!(
        node1
            .docstring
            .as_deref()
            .unwrap_or("")
            .contains("documented macro_rules"),
        "Node '{}': Docstring content mismatch",
        macro_name1
    );

    // Target 2: documented_proc_macro (procedural)
    let macro_name2 = "documented_proc_macro";
    let node2 = find_macro_node_basic(graph, &module_path, macro_name2);
    assert!(
        node2.docstring.is_some(),
        "Node '{}': Expected docstring, found None",
        macro_name2
    );
    assert!(
        node2
            .docstring
            .as_deref()
            .unwrap_or("")
            .contains("documented procedural macro"),
        "Node '{}': Docstring content mismatch",
        macro_name2
    );

    // Target 3: local_macro (no doc)
    let macro_name3 = "local_macro";
    let node3 = find_macro_node_basic(graph, &module_path, macro_name3);
    assert!(
        node3.docstring.is_none(),
        "Node '{}': Expected no docstring, found Some(...)",
        macro_name3
    );
}

#[test]
fn test_macro_node_field_body() {
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];

    // Target 1: exported_macro (declarative)
    let macro_name1 = "exported_macro";
    let node1 = find_macro_node_basic(graph, &module_path, macro_name1);
    assert!(
        node1.body.is_some(),
        "Node '{}': Expected body, found None",
        macro_name1
    );
    let body1 = node1.body.as_deref().unwrap_or("");
    // Check for content within the macro definition braces/parens
    assert!(
        body1.contains("println ! (\"Exported!\")"), // Check token stream content
        "Node '{}': Body content mismatch. Found: '{}'",
        macro_name1,
        body1
    );

    // Target 2: function_like_proc_macro (procedural)
    let macro_name2 = "function_like_proc_macro";
    let node2 = find_macro_node_basic(graph, &module_path, macro_name2);
    assert!(
        node2.body.is_some(),
        "Node '{}': Expected body, found None",
        macro_name2
    );
    let body2 = node2.body.as_deref().unwrap_or("");
    // Check for content within the function body braces
    assert!(
        body2.contains("input"), // Check function body content
        "Node '{}': Body content mismatch. Found: '{}'",
        macro_name2,
        body2
    );
}

#[test]
fn test_macro_node_field_tracking_hash_presence() {
    // Target: local_macro
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "local_macro";

    let node = find_macro_node_basic(graph, &module_path, macro_name);

    assert!(
        node.tracking_hash.is_some(),
        "Node '{}': tracking_hash field should be Some. Actual: {:?}",
        macro_name,
        node.tracking_hash
    );
    assert!(
        matches!(node.tracking_hash, Some(TrackingHash(_))),
        "Node '{}': tracking_hash should contain a Uuid. Actual: {:?}",
        macro_name,
        node.tracking_hash
    );
}

#[test]
fn test_macro_node_field_span() {
    // Target: exported_macro
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("macros.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "exported_macro";

    let node = find_macro_node_basic(graph, &module_path, macro_name);

    // Basic check: span start and end should not be zero
    assert_ne!(
        node.span,
        (0, 0),
        "Node '{}': Span should not be (0, 0). Actual: {:?}",
        macro_name,
        node.span
    );
    assert!(
        node.span.1 > node.span.0,
        "Node '{}': Span end should be greater than start. Actual: {:?}",
        macro_name,
        node.span
    );
}

// --- Tier 4: Basic Connection Tests ---

#[test]
fn test_macro_node_relation_contains_file_module() {
    // Target: exported_macro in "crate::macros" module
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/macros.rs";
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "exported_macro";

    let successful_graphs = run_phases_and_collect(fixture_name);

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .unwrap_or_else(|| panic!("ParsedCodeGraph for macros.rs not found"));
    let graph = &target_data.graph;

    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    let module_id = module_node.id();
    let macro_node = find_macro_node_basic(graph, &module_path, macro_name);
    let macro_id = macro_node.id();

    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(macro_id),
        RelationKind::Contains,
        &format!(
            "Expected Module '{}' ({}) to Contain Macro '{}' ({})",
            module_node.name, module_id, macro_name, macro_id
        ),
    );

    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&macro_id)),
        "MacroNode ID {} not found in items list for Module '{}' ({})",
        macro_id,
        module_node.name,
        module_id
    );
}

#[test]
fn test_macro_node_relation_contains_inline_module() {
    // Target: inner_local_macro in "crate::macros::inner_macros"
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/macros.rs"; // Defined in this file
    let module_path = vec![
        "crate".to_string(),
        "macros".to_string(),
        "inner_macros".to_string(),
    ];
    let macro_name = "inner_local_macro";

    let successful_graphs = run_phases_and_collect(fixture_name);

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .unwrap_or_else(|| panic!("ParsedCodeGraph for macros.rs not found"));
    let graph = &target_data.graph;

    let module_node = find_inline_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    let module_id = module_node.id();
    let macro_node = find_macro_node_basic(graph, &module_path, macro_name);
    let macro_id = macro_node.id();

    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(macro_id),
        RelationKind::Contains,
        &format!(
            "Expected Module '{}' ({}) to Contain Macro '{}' ({})",
            module_node.name, module_id, macro_name, macro_id
        ),
    );

    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&macro_id)),
        "MacroNode ID {} not found in items list for Module '{}' ({})",
        macro_id,
        module_node.name,
        module_id
    );
}

// --- Tier 5: Extreme Paranoia Tests ---

#[test]
fn test_macro_node_paranoid_declarative() {
    // Target: documented_macro
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/macros.rs";
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "documented_macro";

    let successful_graphs = run_phases_and_collect(fixture_name);

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;

    // 1. Find node using paranoid helper
    let node = find_macro_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
        macro_name,
    );

    // 2. Assert all fields
    assert_eq!(node.name, macro_name, "Name mismatch");
    assert_ne!(node.span, (0, 0), "Span should not be default");
    assert_eq!(
        node.visibility,
        VisibilityKind::Public,
        "Visibility mismatch"
    );
    assert_eq!(node.kind, MacroKind::DeclarativeMacro, "Kind mismatch");
    assert_eq!(
        node.attributes.len(),
        1, // #[macro_export]
        "Attribute count mismatch"
    );
    assert!(
        node.attributes.iter().any(|a| a.name == "macro_export"),
        "Missing 'macro_export' attribute"
    );
    assert!(node.docstring.is_some(), "Expected docstring");
    assert!(
        node.docstring
            .as_deref()
            .unwrap_or("")
            .contains("documented macro_rules"),
        "Docstring content mismatch"
    );
    assert!(node.body.is_some(), "Expected body");
    assert!(
        node.body.as_deref().unwrap_or("").contains("stringify !"),
        "Body content mismatch"
    );
    assert!(node.tracking_hash.is_some(), "Tracking hash should be Some");

    // 3. Verify Relation
    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        "Missing Contains relation from module to macro node",
    );
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&node.id())),
        "MacroNode ID not found in module items list"
    );

    // 4. Verify Uniqueness
    let duplicate_id_count = graph.macros.iter().filter(|m| m.id == node.id).count();
    assert_eq!(
        duplicate_id_count, 1,
        "Found duplicate MacroNode ID {} in graph.macros",
        node.id
    );
    // Name uniqueness checked by paranoid helper's module filtering
}

#[test]
fn test_macro_node_paranoid_procedural() {
    // Target: documented_proc_macro
    let fixture_name = "fixture_nodes";
    let file_path_rel = "src/macros.rs";
    let module_path = vec!["crate".to_string(), "macros".to_string()];
    let macro_name = "documented_proc_macro";

    let successful_graphs = run_phases_and_collect(fixture_name);

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .expect("ParsedCodeGraph for macros.rs not found");
    let graph = &target_data.graph;

    // 1. Find node using paranoid helper
    let node = find_macro_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
        macro_name,
    );

    // 2. Assert all fields
    assert_eq!(node.name, macro_name, "Name mismatch");
    assert_ne!(node.span, (0, 0), "Span should not be default");
    assert_eq!(
        node.visibility,
        VisibilityKind::Public, // Proc macros are effectively public
        "Visibility mismatch"
    );
    assert_eq!(
        node.kind,
        MacroKind::ProcedureMacro {
            kind: ProcMacroKind::Derive
        }, // Derive attribute takes precedence
        "Kind mismatch"
    );
    assert_eq!(
        node.attributes.len(),
        2, // #[proc_macro_derive(...)], #[deprecated]
        "Attribute count mismatch"
    );
    assert!(
        node.attributes
            .iter()
            .any(|a| a.name == "proc_macro_derive"),
        "Missing 'proc_macro_derive' attribute"
    );
    assert!(
        node.attributes.iter().any(|a| a.name == "deprecated"),
        "Missing 'deprecated' attribute"
    );
    assert!(node.docstring.is_some(), "Expected docstring");
    assert!(
        node.docstring
            .as_deref()
            .unwrap_or("")
            .contains("documented procedural macro"),
        "Docstring content mismatch"
    );
    assert!(node.body.is_some(), "Expected body (function body)");
    assert!(
        node.body.as_deref().unwrap_or("").contains("input"), // Check function body
        "Body content mismatch"
    );
    assert!(node.tracking_hash.is_some(), "Tracking hash should be Some");

    // 3. Verify Relation
    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture_name,
        file_path_rel,
        &module_path,
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        "Missing Contains relation from module to macro node",
    );
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&node.id())),
        "MacroNode ID not found in module items list"
    );

    // 4. Verify Uniqueness
    let duplicate_id_count = graph.macros.iter().filter(|m| m.id == node.id).count();
    assert_eq!(
        duplicate_id_count, 1,
        "Found duplicate MacroNode ID {} in graph.macros",
        node.id
    );
}
