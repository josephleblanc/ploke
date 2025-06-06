#![cfg(test)]

// Imports mirrored from functions.rs, adjust as needed
use crate::common::ParanoidArgs;
use crate::paranoid_test_fields_and_values;
use anyhow::Result;
use lazy_static::lazy_static;
use ploke_core::ItemKind; // Import TypeKind from ploke_core
use std::collections::HashMap;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::PrimaryNodeIdTrait;
use syn_parser::parser::{
    nodes::{Attribute, ExpectedStructNode}, // Added ExpectedStructNode, Attribute
    types::VisibilityKind,                  // Remove TypeKind from here
}; // Import the test macro

pub const LOG_TEST_STRUCT: &str = "log_test_struct";

lazy_static! {
    static ref EXPECTED_STRUCTS_DATA: HashMap<&'static str, ExpectedStructNode> = {
        let mut m = HashMap::new();
        m.insert(
            "crate::structs::SampleStruct",
            ExpectedStructNode {
                name: "SampleStruct",
                visibility: VisibilityKind::Public,
                fields_count: 1, // Has one field: pub field: String
                generic_params_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
                // Note: `value` field from ConstNode/StaticNode is not applicable here.
                // Note: `type_id_check` from ConstNode/StaticNode is not applicable here.
            },
        );
        m.insert(
            "crate::structs::TupleStruct",
            ExpectedStructNode {
                name: "TupleStruct",
                visibility: VisibilityKind::Public,
                fields_count: 2,
                generic_params_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::structs::UnitStruct",
            ExpectedStructNode {
                name: "UnitStruct",
                visibility: VisibilityKind::Public,
                fields_count: 0,
                generic_params_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::structs::GenericStruct",
            ExpectedStructNode {
                name: "GenericStruct",
                visibility: VisibilityKind::Public,
                fields_count: 1,
                generic_params_count: 1,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::structs::AttributedStruct",
            ExpectedStructNode {
                name: "AttributedStruct",
                visibility: VisibilityKind::Public,
                fields_count: 1,
                generic_params_count: 0,
                attributes: vec![Attribute {
                    name: "derive".to_string(),
                    args: vec!["Debug".to_string()],
                    value: None,
                }],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::structs::DocumentedStruct",
            ExpectedStructNode {
                name: "DocumentedStruct",
                visibility: VisibilityKind::Public,
                fields_count: 1,
                generic_params_count: 0,
                attributes: vec![],
                docstring: Some("This is a documented struct"),
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m
    };
}

lazy_static! {
    static ref EXPECTED_STRUCTS_ARGS: HashMap<&'static str, ParanoidArgs<'static>> = {
        let mut m = HashMap::new();
        m.insert(
            "crate::structs::SampleStruct",
            ParanoidArgs {
                fixture: "fixture_nodes",
                relative_file_path: "src/structs.rs",
                ident: "SampleStruct",
                expected_path: &["crate", "structs"],
                item_kind: ItemKind::Struct,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::structs::TupleStruct",
            ParanoidArgs {
                fixture: "fixture_nodes",
                relative_file_path: "src/structs.rs",
                ident: "TupleStruct",
                expected_path: &["crate", "structs"],
                item_kind: ItemKind::Struct,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::structs::UnitStruct",
            ParanoidArgs {
                fixture: "fixture_nodes",
                relative_file_path: "src/structs.rs",
                ident: "UnitStruct",
                expected_path: &["crate", "structs"],
                item_kind: ItemKind::Struct,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::structs::GenericStruct",
            ParanoidArgs {
                fixture: "fixture_nodes",
                relative_file_path: "src/structs.rs",
                ident: "GenericStruct",
                expected_path: &["crate", "structs"],
                item_kind: ItemKind::Struct,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::structs::AttributedStruct",
            ParanoidArgs {
                fixture: "fixture_nodes",
                relative_file_path: "src/structs.rs",
                ident: "AttributedStruct",
                expected_path: &["crate", "structs"],
                item_kind: ItemKind::Struct,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::structs::DocumentedStruct",
            ParanoidArgs {
                fixture: "fixture_nodes",
                relative_file_path: "src/structs.rs",
                ident: "DocumentedStruct",
                expected_path: &["crate", "structs"],
                item_kind: ItemKind::Struct,
                expected_cfg: None,
            },
        );
        m
    };
}

paranoid_test_fields_and_values!(
    test_sample_struct_fields_and_values,
    "crate::structs::SampleStruct",
    EXPECTED_STRUCTS_ARGS,
    EXPECTED_STRUCTS_DATA,
    syn_parser::parser::nodes::StructNode,
    syn_parser::parser::nodes::ExpectedStructNode,
    as_struct,
    LOG_TEST_STRUCT
);

paranoid_test_fields_and_values!(
    test_tuple_struct_fields_and_values,
    "crate::structs::TupleStruct",
    EXPECTED_STRUCTS_ARGS,
    EXPECTED_STRUCTS_DATA,
    syn_parser::parser::nodes::StructNode,
    syn_parser::parser::nodes::ExpectedStructNode,
    as_struct,
    LOG_TEST_STRUCT
);

paranoid_test_fields_and_values!(
    test_unit_struct_fields_and_values,
    "crate::structs::UnitStruct",
    EXPECTED_STRUCTS_ARGS,
    EXPECTED_STRUCTS_DATA,
    syn_parser::parser::nodes::StructNode,
    syn_parser::parser::nodes::ExpectedStructNode,
    as_struct,
    LOG_TEST_STRUCT
);

paranoid_test_fields_and_values!(
    test_generic_struct_fields_and_values,
    "crate::structs::GenericStruct",
    EXPECTED_STRUCTS_ARGS,
    EXPECTED_STRUCTS_DATA,
    syn_parser::parser::nodes::StructNode,
    syn_parser::parser::nodes::ExpectedStructNode,
    as_struct,
    LOG_TEST_STRUCT
);

paranoid_test_fields_and_values!(
    test_attributed_struct_fields_and_values,
    "crate::structs::AttributedStruct",
    EXPECTED_STRUCTS_ARGS,
    EXPECTED_STRUCTS_DATA,
    syn_parser::parser::nodes::StructNode,
    syn_parser::parser::nodes::ExpectedStructNode,
    as_struct,
    LOG_TEST_STRUCT
);

paranoid_test_fields_and_values!(
    test_documented_struct_fields_and_values,
    "crate::structs::DocumentedStruct",
    EXPECTED_STRUCTS_ARGS,
    EXPECTED_STRUCTS_DATA,
    syn_parser::parser::nodes::StructNode,
    syn_parser::parser::nodes::ExpectedStructNode,
    as_struct,
    LOG_TEST_STRUCT
);

// --- Old Test Cases (to be refactored/removed later) ---

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_struct_node_generic_struct_paranoid() -> Result<()> {
    let fixture_name = "fixture_nodes";
    let all_parsed = run_phases_and_collect(fixture_name);

    let struct_name = "GenericStruct";
    let relative_file_path = "src/structs.rs";
    // Module path *within structs.rs* during Phase 2 parse is just ["crate"]
    let module_path = vec!["crate".to_string(), "structs".to_string()];

    let struct_id_args = EXPECTED_STRUCTS_ARGS
        .get("crate::structs::GenericStruct")
        .expect("Struct Data key not found, see EXPECTED_STRUCTS_ARGS for available keys.");

    let test_info = struct_id_args.generate_pid(&all_parsed)?;
    // Basic Node Properties covered by macro paranoid_test_fields_and_values
    let parsed = all_parsed
        .iter()
        .find(|pg| {
            pg.find_any_node_checked(test_info.test_pid().as_any())
                .ok()
                .is_some()
        })
        .unwrap();
    let struct_node = parsed
        .find_any_node_checked(test_info.test_pid().as_any())
        .map(|n| n.as_struct().expect("If this node has been tested by the macro, then there is a problem with the macro."))?;

    // Generics <T>
    assert_eq!(struct_node.generic_params.len(), 1);
    let generic_param = &struct_node.generic_params[0];
    // Check the generic param details (assuming it's a Type parameter named "T")
    match &generic_param.kind {
        GenericParamKind::Type {
            name,
            bounds,
            default,
        } => {
            assert_eq!(name, "T");
            assert!(bounds.is_empty()); // No bounds specified in fixture
            assert!(default.is_none());
        }
        _ => panic!(
            "Expected GenericParamKind::Type, found {:?}",
            generic_param.kind
        ),
    }

    // Fields (pub field: T)
    assert_eq!(struct_node.fields.len(), 1);
    let field_node = &struct_node.fields[0];

    // Field Properties
    assert!(
        matches!(field_node.id, NodeId::Synthetic(_)),
        "Field ID should be Synthetic"
    );
    assert_eq!(field_node.name.as_deref(), Some("field"));
    assert_eq!(field_node.visibility, VisibilityKind::Public); // Field is pub
    assert!(field_node.attributes.is_empty());

    // Field Type (T)
    assert!(
        matches!(field_node.type_id, TypeId::Synthetic(_)),
        "Field TypeId should be Synthetic"
    );
    let field_type_node = find_type_node(graph, field_node.type_id);
    assert!(
        matches!(&field_type_node.kind, TypeKind::Named { path, .. } if path == &["T"]),
        "Expected field type 'T' (generic param), found {:?}",
        field_type_node.kind
    );
    // Ensure the TypeId for the field matches the TypeId associated with the generic parameter 'T'
    // Note: Finding the TypeId associated with a GenericParamKind::Type might require another helper or careful lookup.
    // For now, we check the name match.

    // --- Paranoid Relation Checks ---

    // 1. Module Contains Struct Relation
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(struct_node.id()),
        RelationKind::Contains,
        "Expected ModuleNode to Contain StructNode",
    );

    // 2. Struct Contains Field Relation
    assert_relation_exists(
        graph,
        GraphId::Node(struct_node.id()),
        GraphId::Node(field_node.id), // FieldNode's ID
        RelationKind::StructField,    // Assuming this is the correct kind for struct -> field
        "Expected StructNode to have StructField relation to FieldNode",
    );

    // 3. Field Type Relation (FieldNode -> TypeId)
    // This isn't typically stored as a separate Relation edge, but implicitly via FieldNode.type_id.
    // We already checked the type_id and TypeNode above.
    Ok(())
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_struct_node_sample_struct() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let struct_name = "SampleStruct";
    let relative_file_path = "src/structs.rs";
    let module_path = vec!["crate".to_string(), "structs".to_string()];

    let struct_node = find_struct_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        struct_name,
    );

    // --- Assertions ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Properties
    assert!(matches!(struct_node.id(), NodeId::Synthetic(_)));
    assert!(struct_node.tracking_hash.is_some());
    assert_eq!(struct_node.name(), struct_name);
    assert_eq!(struct_node.visibility(), VisibilityKind::Public);
    assert!(struct_node.attributes.is_empty());
    assert!(struct_node.docstring.is_none());
    assert!(struct_node.generic_params.is_empty());

    // Fields (pub field: String)
    assert_eq!(struct_node.fields.len(), 1);
    let field_node = &struct_node.fields[0];
    assert!(matches!(field_node.id, NodeId::Synthetic(_)));
    assert_eq!(field_node.name.as_deref(), Some("field"));
    assert_eq!(field_node.visibility, VisibilityKind::Public);
    assert!(field_node.attributes.is_empty());
    let field_type_node = find_type_node(graph, field_node.type_id);
    assert!(matches!(&field_type_node.kind, TypeKind::Named { path, .. } if path == &["String"]));

    // Relations
    let module_id = find_inline_module_by_path(graph, &module_path)
        .unwrap()
        .id();
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(struct_node.id()),
        RelationKind::Contains,
        "Module->Struct",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(struct_node.id()),
        GraphId::Node(field_node.id),
        RelationKind::StructField,
        "Struct->Field",
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_struct_node_tuple_struct() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let struct_name = "TupleStruct";
    let relative_file_path = "src/structs.rs";
    let module_path = vec!["crate".to_string(), "structs".to_string()];

    let struct_node = find_struct_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        struct_name,
    );

    // --- Assertions ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Properties
    assert!(matches!(struct_node.id(), NodeId::Synthetic(_)));
    assert!(struct_node.tracking_hash.is_some());
    assert_eq!(struct_node.name(), struct_name);
    assert_eq!(struct_node.visibility(), VisibilityKind::Public);
    assert!(struct_node.attributes.is_empty());
    assert!(struct_node.docstring.is_none());
    assert!(struct_node.generic_params.is_empty());

    // Fields (pub i32, pub i32) - Tuple struct fields have None name
    assert_eq!(struct_node.fields.len(), 2);
    let field0 = &struct_node.fields[0];
    let field1 = &struct_node.fields[1];

    assert!(matches!(field0.id, NodeId::Synthetic(_)));
    assert!(field0.name.is_none()); // Tuple fields are unnamed
    assert_eq!(field0.visibility, VisibilityKind::Public); // Inherits struct pub
    assert!(field0.attributes.is_empty());
    let field0_type_node = find_type_node(graph, field0.type_id);
    assert!(matches!(&field0_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]));

    assert!(matches!(field1.id, NodeId::Synthetic(_)));
    assert!(field1.name.is_none()); // Tuple fields are unnamed
    assert_eq!(field1.visibility, VisibilityKind::Public); // Inherits struct pub
    assert!(field1.attributes.is_empty());
    let field1_type_node = find_type_node(graph, field1.type_id);
    assert!(matches!(&field1_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]));

    // Relations
    let module_id = find_inline_module_by_path(graph, &module_path)
        .unwrap()
        .id();
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(struct_node.id()),
        RelationKind::Contains,
        "Module->Struct",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(struct_node.id()),
        GraphId::Node(field0.id),
        RelationKind::StructField,
        "Struct->Field0",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(struct_node.id()),
        GraphId::Node(field1.id),
        RelationKind::StructField,
        "Struct->Field1",
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_struct_node_unit_struct() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let struct_name = "UnitStruct";
    let relative_file_path = "src/structs.rs";
    let module_path = vec!["crate".to_string(), "structs".to_string()];

    let struct_node = find_struct_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        struct_name,
    );

    // --- Assertions ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Properties
    assert!(matches!(struct_node.id(), NodeId::Synthetic(_)));
    assert!(struct_node.tracking_hash.is_some());
    assert_eq!(struct_node.name(), struct_name);
    assert_eq!(struct_node.visibility(), VisibilityKind::Public);
    assert!(struct_node.attributes.is_empty());
    assert!(struct_node.docstring.is_none());
    assert!(struct_node.generic_params.is_empty());

    // Fields - None for unit struct
    assert!(struct_node.fields.is_empty());

    // Relations
    let module_id = find_inline_module_by_path(graph, &module_path)
        .unwrap()
        .id();
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(struct_node.id()),
        RelationKind::Contains,
        "Module->Struct",
    );
    // No StructField relations expected
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_struct_node_attributed_struct() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let struct_name = "AttributedStruct";
    let relative_file_path = "src/structs.rs";
    let module_path = vec!["crate".to_string(), "structs".to_string()];

    let struct_node = find_struct_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        struct_name,
    );

    // --- Assertions ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Properties
    assert!(matches!(struct_node.id(), NodeId::Synthetic(_)));
    assert!(struct_node.tracking_hash.is_some());
    assert_eq!(struct_node.name(), struct_name);
    assert_eq!(struct_node.visibility(), VisibilityKind::Public);
    assert!(struct_node.docstring.is_none());
    assert!(struct_node.generic_params.is_empty());

    // Attributes (#[derive(Debug)])
    assert_eq!(struct_node.attributes.len(), 1);
    let attribute = &struct_node.attributes[0];
    assert_eq!(attribute.name, "derive");
    assert_eq!(attribute.args, vec!["Debug"]); // Check derive argument

    // Pre-parse_attribute refactor now fails (correctly?)
    // assert_eq!(
    //     Some("# [derive (Debug)]".to_string()),
    //     attribute.value,
    //     "Expected attribute \"# [derive (Debug)]\", found: {:?}",
    //     attribute.value
    // );
    assert!(
        attribute.value.is_none(),
        "attribute.value to be None, found: {:?}",
        attribute.value
    );

    // Fields (pub field: String)
    assert_eq!(struct_node.fields.len(), 1);
    let field_node = &struct_node.fields[0];
    assert!(matches!(field_node.id, NodeId::Synthetic(_)));
    assert_eq!(field_node.name.as_deref(), Some("field"));
    assert_eq!(field_node.visibility, VisibilityKind::Public);
    assert!(field_node.attributes.is_empty());
    let field_type_node = find_type_node(graph, field_node.type_id);
    assert!(matches!(&field_type_node.kind, TypeKind::Named { path, .. } if path == &["String"]));

    // Relations
    let module_id = find_inline_module_by_path(graph, &module_path)
        .unwrap()
        .id();
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(struct_node.id()),
        RelationKind::Contains,
        "Module->Struct",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(struct_node.id()),
        GraphId::Node(field_node.id),
        RelationKind::StructField,
        "Struct->Field",
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_struct_node_documented_struct() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let struct_name = "DocumentedStruct";
    let relative_file_path = "src/structs.rs";
    let module_path = vec!["crate".to_string(), "structs".to_string()];

    let struct_node = find_struct_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        struct_name,
    );

    // --- Assertions ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Properties
    assert!(matches!(struct_node.id(), NodeId::Synthetic(_)));
    assert!(struct_node.tracking_hash.is_some());
    assert_eq!(struct_node.name(), struct_name);
    assert_eq!(struct_node.visibility(), VisibilityKind::Public);
    assert!(struct_node.attributes.is_empty());
    assert!(struct_node.generic_params.is_empty());

    // Docstring
    assert!(struct_node.docstring.is_some());
    assert_eq!(
        struct_node.docstring.as_deref(),
        Some("This is a documented struct") // Note no leading space from ///
    );

    // Fields (pub field: String)
    assert_eq!(struct_node.fields.len(), 1);
    let field_node = &struct_node.fields[0];
    assert!(matches!(field_node.id, NodeId::Synthetic(_)));
    assert_eq!(field_node.name.as_deref(), Some("field"));
    assert_eq!(field_node.visibility, VisibilityKind::Public);
    assert!(field_node.attributes.is_empty());
    let field_type_node = find_type_node(graph, field_node.type_id);
    assert!(matches!(&field_type_node.kind, TypeKind::Named { path, .. } if path == &["String"]));

    // Relations
    let module_id = find_inline_module_by_path(graph, &module_path)
        .unwrap()
        .id();
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(struct_node.id()),
        RelationKind::Contains,
        "Module->Struct",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(struct_node.id()),
        GraphId::Node(field_node.id),
        RelationKind::StructField,
        "Struct->Field",
    );
}
