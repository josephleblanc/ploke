#![cfg(feature = "uuid_ids")] // Gate the whole module
use crate::common::uuid_ids_utils::*;
use ploke_common::{fixtures_crates_dir, workspace_root};
use ploke_core::{NodeId, TypeId};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use syn_parser::parser::nodes::TypeAliasNode; // Import TypeAliasNode specifically
use syn_parser::parser::nodes::UnionNode; // Import UnionNode specifically
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
fn test_union_node_int_or_float_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let union_name = "IntOrFloat";
    let relative_file_path = "src/unions.rs";
    let module_path = vec!["crate".to_string()]; // Defined at top level of file

    let union_node = find_union_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        union_name,
    );

    // --- Assertions ---
    let graph = &results // Need graph for type lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(union_node.id(), NodeId::Synthetic(_)));
    assert!(
        union_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(union_node.name(), union_name);
    assert_eq!(union_node.visibility(), VisibilityKind::Public);
    assert!(union_node.attributes.is_empty());
    assert!(union_node.docstring.is_none());
    assert!(union_node.generic_params.is_empty());

    // Fields (i: i32, f: f32)
    assert_eq!(union_node.fields.len(), 2);

    // Field i
    let field_i = union_node
        .fields
        .iter()
        .find(|f| f.name.as_deref() == Some("i"))
        .expect("Field 'i' not found");
    assert!(matches!(field_i.id, NodeId::Synthetic(_)));
    assert_eq!(field_i.visibility, VisibilityKind::Inherited); // Fields inherit union visibility by default
    assert!(field_i.attributes.is_empty());
    let type_i = find_type_node(graph, field_i.type_id);
    assert!(matches!(&type_i.kind, TypeKind::Named { path, .. } if path == &["i32"]));

    // Field f
    let field_f = union_node
        .fields
        .iter()
        .find(|f| f.name.as_deref() == Some("f"))
        .expect("Field 'f' not found");
    assert!(matches!(field_f.id, NodeId::Synthetic(_)));
    assert_eq!(field_f.visibility, VisibilityKind::Inherited);
    assert!(field_f.attributes.is_empty());
    let type_f = find_type_node(graph, field_f.type_id);
    assert!(matches!(&type_f.kind, TypeKind::Named { path, .. } if path == &["f32"]));

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains Union
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(union_node.id()),
        RelationKind::Contains,
        "Expected ModuleNode to Contain UnionNode",
    );

    // 2. Union Contains Fields
    assert_relation_exists(
        graph,
        GraphId::Node(union_node.id()),
        GraphId::Node(field_i.id),
        RelationKind::StructField, // Re-use StructField for union fields
        "Expected UnionNode to have StructField relation to FieldNode 'i'",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(union_node.id()),
        GraphId::Node(field_f.id),
        RelationKind::StructField, // Re-use StructField for union fields
        "Expected UnionNode to have StructField relation to FieldNode 'f'",
    );
}

#[test]
fn test_union_node_generic_union_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let union_name = "GenericUnion";
    let relative_file_path = "src/unions.rs";
    let module_path = vec!["crate".to_string()];

    let union_node = find_union_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        union_name,
    );

    // --- Assertions ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(union_node.id(), NodeId::Synthetic(_)));
    assert!(union_node.tracking_hash.is_some());
    assert_eq!(union_node.name(), union_name);
    assert_eq!(union_node.visibility(), VisibilityKind::Public);
    assert!(union_node.attributes.is_empty());
    assert!(union_node.docstring.is_none());

    // Generics <T>
    assert_eq!(union_node.generic_params.len(), 1);
    let generic_param = &union_node.generic_params[0];
    match &generic_param.kind {
        GenericParamKind::Type {
            name,
            bounds,
            default,
        } => {
            assert_eq!(name, "T");
            assert!(bounds.is_empty());
            assert!(default.is_none());
        }
        _ => panic!(
            "Expected GenericParamKind::Type, found {:?}",
            generic_param.kind
        ),
    }

    // Fields (value: ManuallyDrop<T>, raw: usize)
    assert_eq!(union_node.fields.len(), 2);

    // Field value
    let field_value = union_node
        .fields
        .iter()
        .find(|f| f.name.as_deref() == Some("value"))
        .expect("Field 'value' not found");
    assert!(matches!(field_value.id, NodeId::Synthetic(_)));
    assert_eq!(field_value.visibility, VisibilityKind::Inherited);
    let type_value = find_type_node(graph, field_value.type_id);
    // Check ManuallyDrop<T>
    assert!(
        matches!(&type_value.kind, TypeKind::Named { path, .. } if path.ends_with(&["ManuallyDrop".to_string()]))
    );
    assert_eq!(type_value.related_types.len(), 1); // Should relate to T
    let related_t = find_type_node(graph, type_value.related_types[0]);
    assert!(matches!(&related_t.kind, TypeKind::Named { path, .. } if path == &["T"]));

    // Field raw
    let field_raw = union_node
        .fields
        .iter()
        .find(|f| f.name.as_deref() == Some("raw"))
        .expect("Field 'raw' not found");
    assert!(matches!(field_raw.id, NodeId::Synthetic(_)));
    assert_eq!(field_raw.visibility, VisibilityKind::Inherited);
    let type_raw = find_type_node(graph, field_raw.type_id);
    assert!(matches!(&type_raw.kind, TypeKind::Named { path, .. } if path == &["usize"]));

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains Union
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(union_node.id()),
        RelationKind::Contains,
        "Module->Union",
    );

    // 2. Union Contains Fields
    assert_relation_exists(
        graph,
        GraphId::Node(union_node.id()),
        GraphId::Node(field_value.id),
        RelationKind::StructField,
        "Union->Field value",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(union_node.id()),
        GraphId::Node(field_raw.id),
        RelationKind::StructField,
        "Union->Field raw",
    );
}

#[test]
fn test_other_union_nodes() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let relative_file_path = "src/unions.rs";

    // --- Find the relevant graph ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .expect("ParsedCodeGraph for unions.rs not found")
        .graph;

    let module_id_crate = find_inline_module_by_path(graph, &["crate".to_string()])
        .expect("Failed to find top-level module node")
        .id();
    let module_id_inner =
        find_inline_module_by_path(graph, &["crate".to_string(), "inner".to_string()])
            .expect("Failed to find inner module node")
            .id();

    // --- Test Individual Unions ---

    // SecretData (private)
    let union_name = "SecretData";
    let module_path = vec!["crate".to_string()];
    let node = find_union_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        union_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    assert_eq!(node.fields.len(), 2);
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        union_name,
    );

    // CrateUnion (crate visible)
    let union_name = "CrateUnion";
    let node = find_union_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        union_name,
    );
    assert_eq!(
        node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()])
    ); // pub(crate)
    assert_eq!(node.fields.len(), 2);
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        union_name,
    );

    // DocumentedUnion (documented)
    let union_name = "DocumentedUnion";
    let node = find_union_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        union_name,
    );
    assert!(node.docstring.is_some());
    assert_eq!(node.docstring.as_deref(), Some("Documented public union"));
    assert_eq!(node.fields.len(), 2);
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        union_name,
    );

    // ReprCUnion (attribute)
    let union_name = "ReprCUnion";
    let node = find_union_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        union_name,
    );
    assert_eq!(node.attributes.len(), 1);
    assert_eq!(node.attributes[0].name, "repr");
    assert_eq!(node.attributes[0].args, vec!["C"]);
    assert_eq!(node.fields.len(), 2);
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        union_name,
    );

    // UnionWithFieldAttr (fields have attributes)
    let union_name = "UnionWithFieldAttr";
    let node = find_union_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        union_name,
    );
    assert_eq!(node.fields.len(), 3); // Note: Both cfg'd fields are parsed
    let field_always = node
        .fields
        .iter()
        .find(|f| f.name.as_deref() == Some("always_present"))
        .expect("Field 'always_present' not found");
    assert!(field_always.attributes.is_empty());
    // Check that at least one of the cfg'd fields has attributes (exact check depends on target)
    let has_cfg_attr = node
        .fields
        .iter()
        .any(|f| f.name.as_deref() != Some("always_present") && !f.attributes.is_empty());
    assert!(
        has_cfg_attr,
        "Expected at least one cfg'd field to have attributes"
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        union_name,
    );

    // --- Unions inside `inner` module ---
    let module_path_inner = vec!["crate".to_string(), "inner".to_string()];

    // InnerSecret (private in private mod)
    let union_name = "InnerSecret";
    let node = find_union_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        union_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    assert_eq!(node.fields.len(), 2);
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        union_name,
    );

    // InnerPublic (pub in private mod)
    let union_name = "InnerPublic";
    let node = find_union_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        union_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Public); // Public within its module
    assert_eq!(node.fields.len(), 2);
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        union_name,
    );
}
