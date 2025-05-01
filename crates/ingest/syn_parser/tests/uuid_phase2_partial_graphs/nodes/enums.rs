use crate::common::paranoid::find_enum_node_paranoid;
use crate::common::uuid_ids_utils::*;
use ploke_core::{TypeId, TypeKind};
use syn_parser::parser::nodes::GraphId;
// Import TypeKind from ploke_core
use syn_parser::parser::types::VisibilityKind;
use syn_parser::parser::{nodes::GraphNode, relations::RelationKind};

// --- Test Cases ---

#[test]
fn test_enum_node_sample_enum_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let enum_name = "SampleEnum";
    let relative_file_path = "src/enums.rs";
    // Module path *within enums.rs* during Phase 2 parse is just ["crate"]
    let module_path = vec!["crate".to_string(), "enums".to_string()];

    let enum_node = find_enum_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        enum_name,
    );

    // --- Assertions ---
    let graph = &results // Need graph for type/relation lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic EnumNode Properties
    assert!(matches!(enum_node.id(), NodeId::Synthetic(_)));
    assert!(
        enum_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(enum_node.name(), enum_name);
    assert_eq!(enum_node.visibility(), VisibilityKind::Public);
    assert!(enum_node.attributes.is_empty());
    assert!(enum_node.docstring.is_none());
    assert!(enum_node.generic_params.is_empty());

    // Variants
    assert_eq!(enum_node.variants.len(), 3);

    // Variant 1: Variant1
    let variant1 = enum_node
        .variants
        .iter()
        .find(|v| v.name == "Variant1")
        .expect("Variant1 not found");
    assert!(matches!(variant1.id, NodeId::Synthetic(_)));
    assert!(variant1.fields.is_empty());
    assert!(variant1.discriminant.is_none());
    assert!(variant1.attributes.is_empty());

    // Variant 2: Variant2 { value: i32 }
    let variant2 = enum_node
        .variants
        .iter()
        .find(|v| v.name == "Variant2")
        .expect("Variant2 not found");
    assert!(matches!(variant2.id, NodeId::Synthetic(_)));
    assert!(variant2.discriminant.is_none());
    assert!(variant2.attributes.is_empty());
    assert_eq!(variant2.fields.len(), 1);

    // Field within Variant2: value: i32
    let field_value = &variant2.fields[0];
    assert!(matches!(field_value.id, NodeId::Synthetic(_)));
    assert_eq!(field_value.name.as_deref(), Some("value"));
    // Visibility of fields in struct-like variants defaults to Inherited
    assert_eq!(field_value.visibility, VisibilityKind::Inherited);
    assert!(field_value.attributes.is_empty());
    assert!(matches!(field_value.type_id, TypeId::Synthetic(_)));
    let field_type_node = find_type_node(graph, field_value.type_id);
    assert!(matches!(&field_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]));

    // Variant 3: Variant3
    let variant3 = enum_node
        .variants
        .iter()
        .find(|v| v.name == "Variant3")
        .expect("Variant3 not found");
    assert!(matches!(variant3.id, NodeId::Synthetic(_)));
    assert!(variant3.fields.is_empty());
    assert!(variant3.discriminant.is_none());
    assert!(variant3.attributes.is_empty());

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains Enum
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(enum_node.id()),
        RelationKind::Contains,
        "Expected ModuleNode to Contain EnumNode",
    );

    // 2. Enum Contains Variants
    assert_relation_exists(
        graph,
        GraphId::Node(enum_node.id()),
        GraphId::Node(variant1.id),
        RelationKind::EnumVariant,
        "Expected EnumNode to have EnumVariant relation to Variant1",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(enum_node.id()),
        GraphId::Node(variant2.id),
        RelationKind::EnumVariant,
        "Expected EnumNode to have EnumVariant relation to Variant2",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(enum_node.id()),
        GraphId::Node(variant3.id),
        RelationKind::EnumVariant,
        "Expected EnumNode to have EnumVariant relation to Variant3",
    );

    // 3. Variant Contains Field (for struct-like variants)
    assert_relation_exists(
        graph,
        GraphId::Node(variant2.id),
        GraphId::Node(field_value.id),
        RelationKind::VariantField, // Use the correct RelationKind
        "Expected Variant2 to have VariantField relation to its field 'value'",
    );

    // 4. Field Type Relation (Implicit via FieldNode.type_id)
    // Already checked via find_type_node above.
}

#[test]
fn test_other_enum_nodes() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let relative_file_path = "src/enums.rs";
    let module_path = vec!["crate".to_string(), "enums".to_string()];

    // --- Find the relevant graph ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .expect("ParsedCodeGraph for enums.rs not found")
        .graph;

    // --- Test SampleEnum1 ---
    let enum1_name = "SampleEnum1";
    let enum1_node = find_enum_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        enum1_name,
    );

    // Assertions for SampleEnum1
    assert!(matches!(enum1_node.id(), NodeId::Synthetic(_)));
    assert!(enum1_node.tracking_hash.is_some());
    assert_eq!(enum1_node.name(), enum1_name);
    assert_eq!(enum1_node.visibility(), VisibilityKind::Public);
    assert!(enum1_node.attributes.is_empty());
    assert!(enum1_node.docstring.is_none());
    assert!(enum1_node.generic_params.is_empty());
    assert_eq!(enum1_node.variants.len(), 2);

    let variant1_1 = enum1_node
        .variants
        .iter()
        .find(|v| v.name == "Variant1")
        .expect("Variant1 not found in SampleEnum1");
    let variant1_2 = enum1_node
        .variants
        .iter()
        .find(|v| v.name == "Variant2")
        .expect("Variant2 not found in SampleEnum1");
    assert!(variant1_1.fields.is_empty());
    assert!(variant1_2.fields.is_empty());

    // Relations for SampleEnum1
    let module_id = find_inline_module_by_path(graph, &module_path)
        .unwrap()
        .id();
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(enum1_node.id()),
        RelationKind::Contains,
        "Module->SampleEnum1",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(enum1_node.id()),
        GraphId::Node(variant1_1.id),
        RelationKind::EnumVariant,
        "SampleEnum1->Variant1",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(enum1_node.id()),
        GraphId::Node(variant1_2.id),
        RelationKind::EnumVariant,
        "SampleEnum1->Variant2",
    );

    // --- Test EnumWithData ---
    let enum_data_name = "EnumWithData";
    let enum_data_node = find_enum_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        enum_data_name,
    );

    // Assertions for EnumWithData
    assert!(matches!(enum_data_node.id(), NodeId::Synthetic(_)));
    assert!(enum_data_node.tracking_hash.is_some());
    assert_eq!(enum_data_node.name(), enum_data_name);
    assert_eq!(enum_data_node.visibility(), VisibilityKind::Public);
    assert!(enum_data_node.attributes.is_empty());
    assert!(enum_data_node.docstring.is_none());
    assert!(enum_data_node.generic_params.is_empty());
    assert_eq!(enum_data_node.variants.len(), 2);

    // Variant1(i32)
    let variant_data_1 = enum_data_node
        .variants
        .iter()
        .find(|v| v.name == "Variant1")
        .expect("Variant1 not found in EnumWithData");
    assert_eq!(variant_data_1.fields.len(), 1);
    let field_data_1 = &variant_data_1.fields[0];
    assert!(field_data_1.name.is_none()); // Tuple variant field
    assert_eq!(field_data_1.visibility, VisibilityKind::Inherited);
    let field_type_node_1 = find_type_node(graph, field_data_1.type_id);
    assert!(matches!(&field_type_node_1.kind, TypeKind::Named { path, .. } if path == &["i32"]));

    // Variant2(String)
    let variant_data_2 = enum_data_node
        .variants
        .iter()
        .find(|v| v.name == "Variant2")
        .expect("Variant2 not found in EnumWithData");
    assert_eq!(variant_data_2.fields.len(), 1);
    let field_data_2 = &variant_data_2.fields[0];
    assert!(field_data_2.name.is_none()); // Tuple variant field
    assert_eq!(field_data_2.visibility, VisibilityKind::Inherited);
    let field_type_node_2 = find_type_node(graph, field_data_2.type_id);
    assert!(matches!(&field_type_node_2.kind, TypeKind::Named { path, .. } if path == &["String"]));

    // Relations for EnumWithData
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(enum_data_node.id()),
        RelationKind::Contains,
        "Module->EnumWithData",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(enum_data_node.id()),
        GraphId::Node(variant_data_1.id),
        RelationKind::EnumVariant,
        "EnumWithData->Variant1",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(enum_data_node.id()),
        GraphId::Node(variant_data_2.id),
        RelationKind::EnumVariant,
        "EnumWithData->Variant2",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(variant_data_1.id),
        GraphId::Node(field_data_1.id),
        RelationKind::VariantField,
        "EnumWithData::Variant1->Field0",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(variant_data_2.id),
        GraphId::Node(field_data_2.id),
        RelationKind::VariantField,
        "EnumWithData::Variant2->Field0",
    );

    // --- Test DocumentedEnum ---
    let enum_doc_name = "DocumentedEnum";
    let enum_doc_node = find_enum_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        enum_doc_name,
    );

    // Assertions for DocumentedEnum
    assert!(matches!(enum_doc_node.id(), NodeId::Synthetic(_)));
    assert!(enum_doc_node.tracking_hash.is_some());
    assert_eq!(enum_doc_node.name(), enum_doc_name);
    assert_eq!(enum_doc_node.visibility(), VisibilityKind::Public);
    assert!(enum_doc_node.attributes.is_empty());
    assert!(enum_doc_node.generic_params.is_empty());
    assert_eq!(enum_doc_node.variants.len(), 2);

    // Docstring
    assert!(enum_doc_node.docstring.is_some());
    assert_eq!(
        enum_doc_node.docstring.as_deref(),
        Some("This is a documented enum") // Note leading space
    );

    let variant_doc_1 = enum_doc_node
        .variants
        .iter()
        .find(|v| v.name == "Variant1")
        .expect("Variant1 not found in DocumentedEnum");
    let variant_doc_2 = enum_doc_node
        .variants
        .iter()
        .find(|v| v.name == "Variant2")
        .expect("Variant2 not found in DocumentedEnum");
    assert!(variant_doc_1.fields.is_empty());
    assert!(variant_doc_2.fields.is_empty());

    // Relations for DocumentedEnum
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(enum_doc_node.id()),
        RelationKind::Contains,
        "Module->DocumentedEnum",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(enum_doc_node.id()),
        GraphId::Node(variant_doc_1.id),
        RelationKind::EnumVariant,
        "DocumentedEnum->Variant1",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(enum_doc_node.id()),
        GraphId::Node(variant_doc_2.id),
        RelationKind::EnumVariant,
        "DocumentedEnum->Variant2",
    );
}
