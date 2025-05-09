use crate::common::find_type_node;
use crate::common::run_phase1_phase2;
use crate::common::ParanoidArgs;
use anyhow::Ok;
use anyhow::Result;
use lazy_static::lazy_static;
use ploke_core::ItemKind;
use ploke_core::TypeKind;
use std::collections::HashMap;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::GraphNode;
use syn_parser::parser::nodes::ModDisc;
use syn_parser::parser::nodes::PrimaryNodeIdTrait;
use syn_parser::parser::nodes::UnionNode;
use syn_parser::parser::nodes::UnionNodeId;
use syn_parser::parser::types::VisibilityKind;

// This wil be useful once we actually use some better macro testing.
// pub const LOG_TEST_UNION: &str = "log_test_union";

lazy_static! {
    static ref EXPECTED_UNION_ARGS: HashMap<&'static str, ParanoidArgs<'static>> = {
        let mut m = HashMap::new();
        let fixture_name = "fixture_nodes";
        let rel_path = "src/unions.rs";

        m.insert(
            "crate::unions::IntOrFloat",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                expected_path: &["crate", "unions"],
                ident: "IntOrFloat",
                item_kind: ItemKind::Union,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::unions::SecretData",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                expected_path: &["crate", "unions"],
                ident: "SecretData",
                item_kind: ItemKind::Union,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::unions::CrateUnion",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                expected_path: &["crate", "unions"],
                ident: "CrateUnion",
                item_kind: ItemKind::Union,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::unions::DocumentedUnion",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                expected_path: &["crate", "unions"],
                ident: "DocumentedUnion",
                item_kind: ItemKind::Union,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::unions::GenericUnion",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                expected_path: &["crate", "unions"],
                ident: "GenericUnion",
                item_kind: ItemKind::Union,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::unions::ReprCUnion",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                expected_path: &["crate", "unions"],
                ident: "ReprCUnion",
                item_kind: ItemKind::Union,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::unions::UnionWithFieldAttr",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                expected_path: &["crate", "unions"],
                ident: "UnionWithFieldAttr",
                item_kind: ItemKind::Union,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::unions::InnerSecrect",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                expected_path: &["crate", "unions", "inner"],
                ident: "InnerSecrect",
                item_kind: ItemKind::Union,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::unions::InnerPublic",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                expected_path: &["crate", "unions", "inner"],
                ident: "InnerPublic",
                item_kind: ItemKind::Union,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::unions::UseInnerUnion",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                expected_path: &["crate", "unions"],
                ident: "UseInnerUnion",
                item_kind: ItemKind::Union,
                expected_cfg: None,
            },
        );
        m
    };
}

// --- Test Cases ---

#[test]
fn test_union_node_int_or_float_paranoid() -> Result<()> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None)
        .try_init();
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let union_name = "IntOrFloat";
    let relative_file_path = "src/unions.rs";
    let module_path = vec!["crate".to_string(), "unions".to_string()]; // Defined at top level of file

    let parsed_args = EXPECTED_UNION_ARGS
        .get("crate::unions::IntOrFloat")
        .expect("keyed name not found in EXPECTED_UNION_ARGS");

    // --- Assertions ---
    let graph = &results // Need graph for type lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    let test_info = parsed_args.generate_pid(&results)?;
    let union_node_id: UnionNodeId = test_info.test_pid().try_into()?;
    let union_node = graph.get_union_checked(union_node_id)?;

    // Basic Node Properties
    assert!(
        union_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(union_node.name(), union_name);
    assert_eq!(*union_node.visibility(), VisibilityKind::Public);
    assert!(union_node.attributes.is_empty());
    assert!(union_node.docstring.is_none());
    assert!(union_node.generic_params.is_empty());

    // Fields (i: i32, f: f32)
    assert_eq!(union_node.fields.len(), 2);

    println!("{:#?}", union_node);
    let mut field_count = 0_u8;
    let parsed_field_name_i = str_to_field_name(union_node, "i", field_count);
    // for (field, i) in item_union.fields.named.iter().zip(u8::MIN..u8::MAX) {
    //     let mut field_name = field.ident.as_ref().map(|ident| ident.to_string());
    //     let field_ref = field_name.get_or_insert_default();
    //     field_ref.extend("_field_".chars().chain(union_name.as_str().chars()));
    //     field_ref.push(i.into());
    // Field i
    let field_i = union_node
        .fields
        .iter()
        .find(|i| i.name.as_deref() == Some(&parsed_field_name_i))
        .expect("Field 'i' not found");
    assert_eq!(field_i.visibility, VisibilityKind::Inherited); // Fields inherit union visibility by default
    assert!(field_i.attributes.is_empty());
    let type_i = find_type_node(graph, field_i.type_id);
    assert!(matches!(&type_i.kind, TypeKind::Named { path, .. } if path == &["i32"]));
    field_count += 1;

    // Field f
    let parsed_field_name_f = str_to_field_name(union_node, "f", field_count);
    let field_f = union_node
        .fields
        .iter()
        .find(|f| f.name.as_deref() == Some(&parsed_field_name_f))
        .expect("Field 'f' not found");
    assert_eq!(field_f.visibility, VisibilityKind::Inherited);
    assert!(field_f.attributes.is_empty());
    let type_f = find_type_node(graph, field_f.type_id);
    assert!(matches!(&type_f.kind, TypeKind::Named { path, .. } if path == &["f32"]));

    // --- Paranoid Relation Checks ---
    let module_id = graph
        .find_mods_by_kind_path_checked(ModDisc::FileBased, &module_path)
        .unwrap_or_else(|e| {
            union_node.log_node_error();
            panic!(
                "Error: {}, Failed to find containing inline module for node\nInfo Dump\n{:#?}",
                e, union_node,
            )
        })
        .module_id();

    // 1. Module Contains Union
    let container_id = graph
        .relations()
        .iter()
        .filter_map(|r| r.source_contains(union_node_id.to_pid()))
        .next()
        .expect("Expected ModuleNode to Contain UnionNode");
    assert_eq!(
        container_id, module_id,
        "Expected ModuleNode to Contain UnionNode"
    );
    // assert_relation_exists(
    //     graph,
    //     GraphId::Node(module_id),
    //     GraphId::Node(union_node.id()),
    //     RelationKind::Contains,
    //     "Expected ModuleNode to Contain UnionNode",
    // );

    // 2. Union Contains Fields
    // let found_field = graph.relations().iter().find(|r|)
    let union_node_id = graph
        .relations()
        .iter()
        .find_map(|r| r.field_of_union(field_i.id))
        .expect("Did not find field i for node");
    assert_eq!(
        union_node_id,
        union_node.union_id(),
        "Expected UnionNode to have relation with field"
    );
    // assert_relation_exists(
    //     graph,
    //     GraphId::Node(union_node.id()),
    //     GraphId::Node(field_i.id),
    //     RelationKind::StructField, // Re-use StructField for union fields
    //     "Expected UnionNode to have StructField relation to FieldNode 'i'",
    // );
    let union_with_field = graph
        .relations()
        .iter()
        .find_map(|r| r.field_of_union(field_f.id))
        .expect("Did not find field i for node");
    assert_eq!(
        union_with_field, union_node_id,
        "Expected ModuleNode to have relation with field"
    );
    // assert_relation_exists(
    //     graph,
    //     GraphId::Node(union_node.id()),
    //     GraphId::Node(field_f.id),
    //     RelationKind::StructField, // Re-use StructField for union fields
    //     "Expected UnionNode to have StructField relation to FieldNode 'f'",
    // );
    Ok(())
}

fn str_to_field_name(
    union_node: &UnionNode,
    simple_field_name: &str,
    field_count: u8,
) -> std::string::String {
    let mut field_i_name = String::from(simple_field_name);
    field_i_name.extend("_field_".chars().chain(union_node.name.as_str().chars()));
    field_i_name.push(field_count.into());
    field_i_name
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_union_node_generic_union_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let union_name = "GenericUnion";
    let relative_file_path = "src/unions.rs";
    let module_path = vec!["crate".to_string(), "unions".to_string()];

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
#[cfg(not(feature = "type_bearing_ids"))]
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

    let module_id_crate =
        find_inline_module_by_path(graph, &["crate".to_string(), "unions".to_string()])
            .expect("Failed to find top-level module node")
            .id();
    let module_id_inner = find_inline_module_by_path(
        graph,
        &[
            "crate".to_string(),
            "unions".to_string(),
            "inner".to_string(),
        ],
    )
    .expect("Failed to find inner module node")
    .id();

    // --- Test Individual Unions ---

    // SecretData (private)
    let union_name = "SecretData";
    let module_path = vec!["crate".to_string(), "unions".to_string()];
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
    assert!(
        field_always.attributes.is_empty(),
        "Field 'always_present' should have no non-cfg attributes"
    );
    assert!(
        field_always.cfgs.is_empty(),
        "Field 'always_present' should have no cfgs"
    );

    // Check that at least one of the other fields has a non-empty cfgs list
    let has_cfg_string = node
        .fields
        .iter()
        .any(|f| f.name.as_deref() != Some("always_present") && !f.cfgs.is_empty());
    assert!(
        has_cfg_string,
        "Expected at least one cfg'd field ('big_endian_data' or 'little_endian_data') to have a non-empty cfgs list"
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        union_name,
    );

    // --- Unions inside `inner` module ---
    let module_path_inner = vec![
        "crate".to_string(),
        "unions".to_string(),
        "inner".to_string(),
    ];

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
