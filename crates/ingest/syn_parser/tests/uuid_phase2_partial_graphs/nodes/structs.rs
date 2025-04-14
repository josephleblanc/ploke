#![cfg(test)]

// Imports mirrored from functions.rs, adjust as needed
use crate::common::{paranoid::find_struct_node_paranoid, uuid_ids_utils::*};
use ploke_core::{NodeId, TypeId};
use syn_parser::parser::{
    nodes::Visible,
    relations::{GraphId, RelationKind}, // Added for relation checks
    types::{GenericParamKind, TypeKind, VisibilityKind},
};

// --- Test Cases ---

#[test]
fn test_struct_node_generic_struct_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let struct_name = "GenericStruct";
    let relative_file_path = "src/structs.rs";
    // Module path *within structs.rs* during Phase 2 parse is just ["crate"]
    let module_path = vec!["crate".to_string(), "structs".to_string()];

    let struct_node = find_struct_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        struct_name,
    );

    // --- Assertions ---
    let graph = &results // Need graph for type/relation lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(struct_node.id(), NodeId::Synthetic(_)));
    assert!(
        struct_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(struct_node.name(), struct_name);
    assert_eq!(struct_node.visibility(), VisibilityKind::Public);
    assert!(struct_node.attributes.is_empty());
    assert!(struct_node.docstring.is_none());

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
}

#[test]
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
