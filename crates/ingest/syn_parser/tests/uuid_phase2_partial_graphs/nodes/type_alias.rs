#![allow(non_snake_case)]

use crate::common::find_type_node;
use crate::common::ParanoidArgs;
use crate::paranoid_test_fields_and_values;
use crate::paranoid_test_setup;
use crate::run_paranoid_test;
use lazy_static::lazy_static;
use ploke_core::ItemKind;
use ploke_core::TypeId;
use ploke_core::TypeKind;
use std::collections::HashMap;
use syn_parser::error::SynParserError;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::ExpectedTypeAliasNode;
use syn_parser::parser::nodes::PrimaryNodeIdTrait;
use syn_parser::parser::nodes::TypeAliasNode;
use syn_parser::parser::types::GenericParamKind;
use syn_parser::parser::ParsedCodeGraph;

// Keep old imports for existing tests
// Import TypeKind from ploke_core
// Import TypeAliasNode specifically
use syn_parser::parser::types::VisibilityKind;

pub const LOG_TEST_TYPE_ALIAS: &str = "log_test_type_alias";

lazy_static! {
    static ref EXPECTED_TYPE_ALIASES_ARGS: HashMap<&'static str, ParanoidArgs<'static>> = {
        let mut m = HashMap::new();
        let fixture_name = "fixture_nodes";
        let rel_path = "src/type_alias.rs";

        m.insert("crate::type_alias::SimpleId", ParanoidArgs {
            fixture: fixture_name, relative_file_path: rel_path, ident: "SimpleId",
            expected_path: &["crate", "type_alias"], item_kind: ItemKind::TypeAlias, expected_cfg: None,
        });
        m.insert("crate::type_alias::GenericContainer", ParanoidArgs {
            fixture: fixture_name, relative_file_path: rel_path, ident: "GenericContainer",
            expected_path: &["crate", "type_alias"], item_kind: ItemKind::TypeAlias, expected_cfg: None,
        });
        m.insert("crate::type_alias::DisplayableContainer", ParanoidArgs {
            fixture: fixture_name, relative_file_path: rel_path, ident: "DisplayableContainer",
            expected_path: &["crate", "type_alias"], item_kind: ItemKind::TypeAlias, expected_cfg: None,
        });
        m.insert("crate::type_alias::Mapping", ParanoidArgs {
            fixture: fixture_name,
            relative_file_path: rel_path,
            expected_path: &["crate", "type_alias"],
            ident: "Mapping",
            item_kind: ItemKind::TypeAlias,
            expected_cfg: None,
        });
        m
    };

    static ref EXPECTED_TYPE_ALIASES_DATA: HashMap<&'static str, ExpectedTypeAliasNode> = {
        let mut m = HashMap::new();

        // For use in ExpectedTypeAliasNode below

        m.insert("crate::type_alias::SimpleId", ExpectedTypeAliasNode {
            name:"SimpleId",visibility:VisibilityKind::Public,
            generic_params_count:0,attributes:vec![],
            docstring:None,
            tracking_hash_check:true,
            cfgs:vec![],
            type_id_check: true,
        });
        m.insert("crate::type_alias::GenericContainer", ExpectedTypeAliasNode {
            name:"GenericContainer",visibility:VisibilityKind::Public,
            generic_params_count:1,
            attributes:vec![],
            docstring:None,
            tracking_hash_check:true,
            cfgs:vec![],
            type_id_check: true
        });
        m.insert("crate::type_alias::DisplayableContainer", ExpectedTypeAliasNode {
            name:"DisplayableContainer",
            visibility:VisibilityKind::Public,
            generic_params_count:1,
            attributes:vec![],
            docstring:None,
            tracking_hash_check:true,cfgs:vec![],
            type_id_check: true
        });
        m.insert("crate::type_alias::Mapping", ExpectedTypeAliasNode {
            name: "Mapping",                       // same as key path ending
            visibility: VisibilityKind::Public, // usually Public
            type_id_check: true,            // t/f, hanlde more specific elsewhere
            generic_params_count: 2,       // basic check, more detailed elsehwere
            attributes: vec![],           // usually empty, otherwise vec of &'static str
            docstring: None,                  // usually empty (None)
            tracking_hash_check: true,      // usually true, handle more specific checks in other tests
            cfgs: vec![]                  // usually empty
        });
        m
    };
}

paranoid_test_fields_and_values!(
    test_type_alias_simple_id_macro,
    "crate::type_alias::SimpleId",
    EXPECTED_TYPE_ALIASES_ARGS,
    EXPECTED_TYPE_ALIASES_DATA,
    syn_parser::parser::nodes::TypeAliasNode,
    syn_parser::parser::nodes::ExpectedTypeAliasNode,
    as_type_alias,
    LOG_TEST_TYPE_ALIAS
);

paranoid_test_fields_and_values!(
    test_type_alias_generic_container_macro,
    "crate::type_alias::GenericContainer",
    EXPECTED_TYPE_ALIASES_ARGS,
    EXPECTED_TYPE_ALIASES_DATA,
    syn_parser::parser::nodes::TypeAliasNode,
    syn_parser::parser::nodes::ExpectedTypeAliasNode,
    as_type_alias,
    LOG_TEST_TYPE_ALIAS
);

// Covers:
//  - Multiple generic parameters
//  - Type lookup of generic parameters, confirming
//      - name
//      - bounds
//      - defalut (some/none)
//  - aliased type id, checking
//      - presence/variant of `TypeId`
//      - path of typed id, here "std", "collections", "HashMap"
//      - presence + name of associated types (here K, V)
paranoid_test_setup!(
    setup_mapping,
    "crate::type_alias::Mapping",
    EXPECTED_TYPE_ALIASES_ARGS,
    EXPECTED_TYPE_ALIASES_DATA,
    syn_parser::parser::nodes::TypeAliasNode,
    syn_parser::parser::nodes::ExpectedTypeAliasNode,
    as_type_alias,
    LOG_TEST_TYPE_ALIAS
);

run_paranoid_test! { setup_mapping, test_display_mapping, check_generic_params_mapping }

fn check_generic_params_mapping(
    setup_data: (TypeAliasNode, &ParsedCodeGraph),
) -> Result<(), SynParserError> {
    let (type_alias_node, graph_data) = setup_data;

    let generic_param_k = &type_alias_node.generic_params[0];
    match &generic_param_k.kind {
        GenericParamKind::Type {
            name,
            bounds,
            default,
        } => {
            assert_eq!(name, "K"); // should be T
            assert_eq!(bounds.len(), 0, "Expected no trait bounds on K");
            assert!(default.is_none());
        }
        _ => panic!(
            "Expected GenericParamKind::Type for K, found {:?}",
            generic_param_k.kind
        ),
    }

    let generic_param_v = &type_alias_node.generic_params[1];
    match &generic_param_v.kind {
        GenericParamKind::Type {
            name,
            bounds,
            default,
        } => {
            assert_eq!(name, "V"); // should be T
            assert_eq!(bounds.len(), 0, "Expected no trait bounds on K");
            assert!(default.is_none());
        }
        _ => panic!(
            "Expected GenericParamKind::Type for V, found {:?}",
            generic_param_v
        ),
    }
    // 2. Check the aliased TypeId: resolves to Vec<T>
    let aliased_type_id = type_alias_node.type_id;
    assert!(
        matches!(aliased_type_id, TypeId::Synthetic(_)),
        "Aliased TypeId should be Synthetic"
    );
    let aliased_type_node = find_type_node(&graph_data.graph, aliased_type_id);

    // Check the related types K and V for std::collections::HashMap<K, V>
    assert_eq!(
        aliased_type_node.related_types.len(),
        2,
        "std::collections::HashMap<K, V> should have two related types (K, V)"
    );
    let related_type_id_for_vec_k = aliased_type_node.related_types[0];
    let related_type_node_for_vec_k = find_type_node(&graph_data.graph, related_type_id_for_vec_k);
    assert!(
        matches!(&related_type_node_for_vec_k.kind, TypeKind::Named { path, .. } if path == &["K".to_string()]),
        "Expected related type 'K' for HashMap, found {:?}",
        related_type_node_for_vec_k.kind
    );
    let related_type_id_for_vec_v = aliased_type_node.related_types[1];
    let related_type_node_for_vec_v = find_type_node(&graph_data.graph, related_type_id_for_vec_v);
    assert!(
        matches!(&related_type_node_for_vec_v.kind, TypeKind::Named { path, .. } if path == &["V".to_string()]),
        "Expected related type 'T' for Vec, found {:?}",
        related_type_node_for_vec_v.kind
    );

    Ok(())
}

// --- New Manual Detailed Test ---

paranoid_test_setup!(
    setup_display_container,
    "crate::type_alias::DisplayableContainer",
    EXPECTED_TYPE_ALIASES_ARGS,
    EXPECTED_TYPE_ALIASES_DATA,
    syn_parser::parser::nodes::TypeAliasNode,
    syn_parser::parser::nodes::ExpectedTypeAliasNode,
    as_type_alias,
    LOG_TEST_TYPE_ALIAS
);

run_paranoid_test! { setup_display_container, test_display_container, check_generic_params }

fn check_generic_params(
    setup_data: (TypeAliasNode, &ParsedCodeGraph),
) -> Result<(), SynParserError> {
    let (type_alias_node, graph_data) = setup_data;
    // 1. Check Generic Parameters of the TypeAliasNode: <T: std::fmt::Display>
    assert_eq!(type_alias_node.generic_params.len(), 1);
    let generic_param_t = &type_alias_node.generic_params[0];
    match &generic_param_t.kind {
        GenericParamKind::Type {
            name,
            bounds,
            default,
        } => {
            assert_eq!(name, "T"); // should be T
            assert_eq!(bounds.len(), 1, "Expected one trait bound (Display)");
            assert!(default.is_none());

            // Check the bound TypeId corresponds to Display
            let bound_type_id = bounds[0];
            let bound_type_node = find_type_node(&graph_data.graph, bound_type_id);
            // Path might be fully qualified or just "Display" depending on resolution context
            // For Phase 2, it's often the simple name or a partially resolved path.
            // A more robust check might involve checking ends_with or specific segments.
            assert!(
                matches!(&bound_type_node.kind, TypeKind::Named { path, .. } if path.iter().any(|seg| seg == "Display")),
                "Expected bound type 'Display', found {:?}",
                bound_type_node.kind
            );
        }
        _ => panic!(
            "Expected GenericParamKind::Type for T, found {:?}",
            generic_param_t.kind
        ),
    }

    // 2. Check the aliased TypeId: resolves to Vec<T>
    let aliased_type_id = type_alias_node.type_id;
    assert!(
        matches!(aliased_type_id, TypeId::Synthetic(_)),
        "Aliased TypeId should be Synthetic"
    );
    let aliased_type_node = find_type_node(&graph_data.graph, aliased_type_id);

    // Check the Vec part
    assert!(
        matches!(&aliased_type_node.kind, TypeKind::Named { path, .. } if path == &["Vec".to_string()]),
        "Expected aliased type 'Vec<T>', found outer type {:?}",
        aliased_type_node.kind
    );

    // Check the related type (T) for Vec<T>
    assert_eq!(
        aliased_type_node.related_types.len(),
        1,
        "Vec<T> should have one related type (T)"
    );

    // Ensure the 'T' from Vec<T> is the same TypeId as the 'T' from the generic_params
    // This requires TypeId to be comparable and correctly generated.
    // The TypeId for the generic parameter `T` itself is not directly stored on GenericParamNode.
    // Instead, the `TypeId` for `T` as used in `Vec<T>` (related_type_id_for_vec_t)
    // should correspond to a `TypeNode` whose name is "T".
    // The `GenericParamNode` for `T` also has `name: "T"`.
    // A deeper check would involve ensuring that within the scope of this type alias,
    // these two "T"s refer to the same conceptual type parameter.
    // For now, matching by name is a good indicator.

    Ok(())
}

// --- Old Test Cases (Kept as per instruction) ---

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_type_alias_node_simple_id_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let alias_name = "SimpleId";
    let relative_file_path = "src/type_alias.rs";
    let module_path = vec!["crate".to_string(), "type_alias".to_string()]; // Defined at top level of file

    let type_alias_node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );

    // --- Assertions ---
    let graph = &results // Need graph for type lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(type_alias_node.id(), NodeId::Synthetic(_)));
    assert!(
        type_alias_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(type_alias_node.name(), alias_name);
    assert_eq!(type_alias_node.visibility(), VisibilityKind::Public);
    assert!(type_alias_node.attributes.is_empty());
    assert!(type_alias_node.docstring.is_none());
    assert!(type_alias_node.generic_params.is_empty());

    // Aliased Type (type_id -> u64)
    assert!(
        matches!(type_alias_node.type_id, TypeId::Synthetic(_)),
        "Aliased TypeId should be Synthetic"
    );
    let aliased_type_node = find_type_node(graph, type_alias_node.type_id);
    assert!(
        matches!(&aliased_type_node.kind, TypeKind::Named { path, .. } if path == &["u64"]),
        "Expected aliased type 'u64', found {:?}",
        aliased_type_node.kind
    );
    assert!(aliased_type_node.related_types.is_empty()); // u64 has no related types

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains TypeAlias
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(type_alias_node.id()),
        RelationKind::Contains,
        "Expected ModuleNode to Contain TypeAliasNode",
    );

    // 2. TypeAlias Aliases Type (Implicit via TypeAliasNode.type_id)
    // No separate relation edge for this currently.
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_type_alias_node_displayable_container_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let alias_name = "DisplayableContainer";
    let relative_file_path = "src/type_alias.rs";
    let module_path = vec!["crate".to_string(), "type_alias".to_string()];

    let type_alias_node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );

    // --- Assertions ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(type_alias_node.id(), NodeId::Synthetic(_)));
    assert!(type_alias_node.tracking_hash.is_some());
    assert_eq!(type_alias_node.name(), alias_name);
    assert_eq!(type_alias_node.visibility(), VisibilityKind::Public);
    assert!(type_alias_node.attributes.is_empty());
    assert!(type_alias_node.docstring.is_none());

    // Generics <T: std::fmt::Display>
    assert_eq!(type_alias_node.generic_params.len(), 1);
    let generic_param = &type_alias_node.generic_params[0];
    match &generic_param.kind {
        GenericParamKind::Type {
            name,
            bounds,
            default,
        } => {
            assert_eq!(name, "T");
            assert_eq!(bounds.len(), 1, "Expected one trait bound (Display)");
            // Check the bound TypeId corresponds to Display
            let bound_type_id = bounds[0];
            let bound_type_node = find_type_node(graph, bound_type_id);
            // Expecting path like ["std", "fmt", "Display"] - may need adjustment based on how paths are stored
            assert!(
                matches!(&bound_type_node.kind, TypeKind::Named { path, .. } if path.ends_with(&["Display".to_string()])), // Check suffix for now
                "Expected bound type 'Display', found {:?}",
                bound_type_node.kind
            );
            assert!(default.is_none());
        }
        _ => panic!(
            "Expected GenericParamKind::Type, found {:?}",
            generic_param.kind
        ),
    }

    // Aliased Type (type_id -> Vec<T>)
    assert!(matches!(type_alias_node.type_id, TypeId::Synthetic(_)));
    let aliased_type_node = find_type_node(graph, type_alias_node.type_id);
    // Check the Vec part
    assert!(
        matches!(&aliased_type_node.kind, TypeKind::Named { path, .. } if path == &["Vec"]),
        "Expected aliased type 'Vec<T>', found {:?}",
        aliased_type_node.kind
    );
    // Check the related type (T)
    assert_eq!(
        aliased_type_node.related_types.len(),
        1,
        "Vec<T> should have one related type (T)"
    );
    let related_type_id = aliased_type_node.related_types[0];
    let related_type_node = find_type_node(graph, related_type_id);
    assert!(
        matches!(&related_type_node.kind, TypeKind::Named { path, .. } if path == &["T"]),
        "Expected related type 'T', found {:?}",
        related_type_node.kind
    );

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains TypeAlias
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(type_alias_node.id()),
        RelationKind::Contains,
        "Expected ModuleNode to Contain TypeAliasNode",
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_other_type_alias_nodes() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let relative_file_path = "src/type_alias.rs";

    // --- Find the relevant graph ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .expect("ParsedCodeGraph for type_alias.rs not found")
        .graph;

    let module_id_crate =
        find_inline_module_by_path(graph, &["crate".to_string(), "type_alias".to_string()])
            .expect("Failed to find top-level module node")
            .id();
    let module_id_inner = find_inline_module_by_path(
        graph,
        &[
            "crate".to_string(),
            "type_alias".to_string(),
            "inner".to_string(),
        ],
    )
    .expect("Failed to find inner module node")
    .id();

    // --- Test Individual Aliases ---

    // InternalCounter (private)
    let alias_name = "InternalCounter";
    let module_path = vec!["crate".to_string(), "type_alias".to_string()];
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path == &["i32"]));
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // CrateBuffer (crate visible)
    let alias_name = "CrateBuffer";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(
        node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()])
    ); // pub(crate)
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path == &["Vec"])); // Vec<u8>
    assert_eq!(aliased_type.related_types.len(), 1); // u8
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // Point (documented)
    let alias_name = "Point";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert!(node.docstring.is_some());
    assert_eq!(
        node.docstring.as_deref(),
        Some("Documented public alias for a tuple type")
    );
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(
        matches!(&aliased_type.kind, TypeKind::Unknown { type_str } if type_str == "(i32 , i32)")
    ); // Tuple not implemented
       // #[ignore = "TypeKind::Tuple not yet handled"]
    {}
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // GenericContainer<T>
    let alias_name = "GenericContainer";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.generic_params.len(), 1);
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path == &["Vec"])); // Vec<T>
    assert_eq!(aliased_type.related_types.len(), 1); // T
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // Mapping<K, V>
    let alias_name = "Mapping";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.generic_params.len(), 2);
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(
        matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path.ends_with(&["HashMap".to_string()]))
    ); // std::collections::HashMap<K, V>
    assert_eq!(aliased_type.related_types.len(), 2); // K, V
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // MathOperation (private fn pointer)
    let alias_name = "MathOperation";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(
        matches!(&aliased_type.kind, TypeKind::Unknown { type_str } if type_str == "fn (i32 , i32) -> i32")
    ); // Fn Ptr not implemented
       // #[ignore = "TypeKind::Function not yet handled"]
    {}
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // OldId (attribute)
    let alias_name = "OldId";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.attributes.len(), 1);
    assert_eq!(node.attributes[0].name, "deprecated");
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path == &["String"]));
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // IdAlias (alias of alias)
    let alias_name = "IdAlias";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path == &["SimpleId"])); // Points to the other alias name
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // ComplexGeneric<T> (where clause)
    let alias_name = "ComplexGeneric";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.generic_params.len(), 1); // T
                                              // TODO: Add check for where clause bounds once generics parsing is more detailed
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path == &["Option"])); // Option<T>
    assert_eq!(aliased_type.related_types.len(), 1); // T
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // --- Aliases inside `inner` module ---
    let module_path_inner = vec![
        "crate".to_string(),
        "type_alias".to_string(),
        "inner".to_string(),
    ];

    // InnerSecret (private in private mod)
    let alias_name = "InnerSecret";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        alias_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path == &["bool"]));
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // InnerPublic (pub in private mod)
    let alias_name = "InnerPublic";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        alias_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Public); // Public within its module
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path == &["f64"]));
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // OuterPoint (pub(super))
    let alias_name = "OuterPoint";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        alias_name,
    );
    assert_eq!(
        node.visibility(),
        VisibilityKind::Restricted(vec!["super".to_string()])
    ); // pub(super)
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(
        matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path.ends_with(&["Point".to_string()]))
    ); // super::Point
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // --- Aliases using inner module types ---
    let module_path = vec!["crate".to_string(), "type_alias".to_string()]; // Back to top level

    // UseInner (private, uses pub type from private mod)
    let alias_name = "UseInner";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(
        matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path.ends_with(&["InnerPublic".to_string()]))
    ); // inner::InnerPublic
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // UseOuterPoint (private, uses pub(super) type from private mod)
    let alias_name = "UseOuterPoint";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(
        matches!(&aliased_type.kind, TypeKind::Named { path, .. } if path.ends_with(&["OuterPoint".to_string()]))
    ); // inner::OuterPoint
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // --- Reference/Pointer Aliases ---

    // StrSlice<'a>
    let alias_name = "StrSlice";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.generic_params.len(), 1); // Lifetime 'a
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(matches!(
        &aliased_type.kind,
        TypeKind::Reference {
            is_mutable: false,
            ..
        }
    )); // &'a str
    assert_eq!(aliased_type.related_types.len(), 1); // str
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // MutStrSlice<'a>
    let alias_name = "MutStrSlice";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.generic_params.len(), 1); // Lifetime 'a
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(matches!(
        &aliased_type.kind,
        TypeKind::Reference {
            is_mutable: true,
            ..
        }
    )); // &'a mut str
    assert_eq!(aliased_type.related_types.len(), 1); // str
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // ConstRawPtr
    let alias_name = "ConstRawPtr";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(
        matches!(&aliased_type.kind, TypeKind::Unknown { type_str } if type_str == "* const u8")
    ); // Ptr not implemented
       // #[ignore = "TypeKind::Ptr not yet handled"]
    {}
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // MutRawPtr
    let alias_name = "MutRawPtr";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    let expected_type_str = "* mut u8";
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(
        matches!(&aliased_type.kind, TypeKind::Unknown { type_str } if type_str == expected_type_str),
        "Expected: \"{}\", found: {:?}",
        expected_type_str,
        &aliased_type.kind
    ); // Ptr not implemented
       // #[ignore = "TypeKind::Ptr not yet handled"]
    {}
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // --- Array/Slice/Trait Object Aliases ---

    // ByteArray
    let alias_name = "ByteArray";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(
        matches!(&aliased_type.kind, TypeKind::Unknown { type_str } if type_str == "[u8 ; 256]")
    ); // Array not implemented
       // #[ignore = "TypeKind::Array not yet handled"]
    {}
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );

    // DynDrawable
    let alias_name = "DynDrawable";
    let node = find_type_alias_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        alias_name,
    );
    let expected_type_str = "dyn std :: fmt :: Debug"; // shadowing above
    let aliased_type = find_type_node(graph, node.type_id);
    assert!(
        matches!(&aliased_type.kind, TypeKind::Unknown { type_str } if type_str == expected_type_str),
        "Expected: \"{}\", found: {:?}",
        expected_type_str,
        &aliased_type.kind
    ); // TraitObject not implemented
       // #[ignore = "TypeKind::TraitObject not yet handled"]
    {}
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        alias_name,
    );
}
