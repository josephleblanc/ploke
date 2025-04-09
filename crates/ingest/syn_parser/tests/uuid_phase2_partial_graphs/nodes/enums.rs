#![cfg(feature = "uuid_ids")] // Gate the whole module
use crate::common::uuid_ids_utils::*;
use ploke_common::{fixtures_crates_dir, workspace_root};
use ploke_core::{NodeId, TypeId};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
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
use uuid::Uuid;

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
    let module_path = vec!["crate".to_string()];

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
        RelationKind::StructField, // Re-use StructField for struct-like variants
        "Expected Variant2 to have StructField relation to its field 'value'",
    );

    // 4. Field Type Relation (Implicit via FieldNode.type_id)
    // Already checked via find_type_node above.
}

// TODO: Add less paranoid tests for SampleEnum1, EnumWithData, DocumentedEnum
