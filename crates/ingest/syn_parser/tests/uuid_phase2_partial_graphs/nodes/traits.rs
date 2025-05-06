use crate::common::paranoid::find_trait_node_paranoid;
// Gate the whole module
use crate::common::uuid_ids_utils::*;
use ploke_core::{TypeKind};
use syn_parser::parser::nodes::GraphId;
// Import TypeKind from ploke_core
// Import UnionNode specifically
use syn_parser::parser::types::VisibilityKind;
use syn_parser::parser::{nodes::GraphNode, relations::RelationKind};

// --- Test Cases ---

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_trait_node_simple_trait_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let trait_name = "SimpleTrait";
    let relative_file_path = "src/traits.rs";
    let module_path = vec!["crate".to_string(), "traits".to_string()]; // Defined at top level of file

    let trait_node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );

    // --- Assertions ---
    let graph = &results // Need graph for type/relation lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(trait_node.id(), NodeId::Synthetic(_)));
    assert!(
        trait_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(trait_node.name(), trait_name);
    assert_eq!(trait_node.visibility(), VisibilityKind::Public);
    assert!(trait_node.attributes.is_empty());
    assert!(trait_node.docstring.is_none());
    assert!(trait_node.generic_params.is_empty());
    assert!(trait_node.super_traits.is_empty());

    // Methods (fn required_method(&self) -> i32;)
    assert_eq!(trait_node.methods.len(), 1);
    let method_node = &trait_node.methods[0];
    assert_eq!(method_node.name(), "required_method");
    assert!(matches!(method_node.id(), NodeId::Synthetic(_)));
    assert!(method_node.tracking_hash.is_some()); // Methods within traits should have hashes
    assert_eq!(method_node.parameters.len(), 1); // &self
    assert!(method_node.parameters[0].is_self);
    assert!(method_node.return_type.is_some());
    let return_type = find_type_node(graph, method_node.return_type.unwrap());
    assert!(matches!(&return_type.kind, TypeKind::Named { path, .. } if path == &["i32"]));

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains Trait
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(trait_node.id()),
        RelationKind::Contains,
        "Expected ModuleNode to Contain TraitNode",
    );

    // 2. Trait Contains Method (Assuming RelationKind::TraitMethod exists)
    // NOTE: Note yet implemented
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_trait_node_complex_generic_trait_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let trait_name = "ComplexGenericTrait";
    let relative_file_path = "src/traits.rs";
    let module_path = vec!["crate".to_string(), "traits".to_string()];

    let trait_node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );

    // --- Assertions ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(trait_node.id(), NodeId::Synthetic(_)));
    assert!(trait_node.tracking_hash.is_some());
    assert_eq!(trait_node.name(), trait_name);
    assert_eq!(trait_node.visibility(), VisibilityKind::Public);
    assert!(trait_node.attributes.is_empty());
    assert!(trait_node.docstring.is_none());
    assert!(trait_node.super_traits.is_empty());

    // Generics <'a, T: Debug + Clone, S: Send + Sync> where T: 'a
    assert_eq!(trait_node.generic_params.len(), 3);
    // TODO: Add detailed checks for generic param kinds, bounds, and where clauses

    // Methods (fn complex_process(&'a self, item: T, other: S) -> &'a T;)
    assert_eq!(trait_node.methods.len(), 1);
    let method_node = &trait_node.methods[0];
    assert_eq!(method_node.name(), "complex_process");
    assert!(matches!(method_node.id(), NodeId::Synthetic(_)));
    assert!(method_node.tracking_hash.is_some());
    assert_eq!(method_node.parameters.len(), 3); // &'a self, item: T, other: S
    assert!(method_node.parameters[0].is_self);
    let param_t_type = find_type_node(graph, method_node.parameters[1].type_id);
    let param_s_type = find_type_node(graph, method_node.parameters[2].type_id);
    assert!(matches!(&param_t_type.kind, TypeKind::Named { path, .. } if path == &["T"]));
    assert!(matches!(&param_s_type.kind, TypeKind::Named { path, .. } if path == &["S"]));

    assert!(method_node.return_type.is_some());
    let return_type_node = find_type_node(graph, method_node.return_type.unwrap());
    // Check return type &'a T
    assert!(matches!(&return_type_node.kind, TypeKind::Reference { .. }));
    assert_eq!(return_type_node.related_types.len(), 1);
    let referenced_return_type = find_type_node(graph, return_type_node.related_types[0]);
    assert!(matches!(&referenced_return_type.kind, TypeKind::Named { path, .. } if path == &["T"]));

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains Trait
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(trait_node.id()),
        RelationKind::Contains,
        "Module->Trait",
    );

    // 2. Trait Contains Method
    // NOTE: Not yet implemented
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_other_trait_nodes() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let relative_file_path = "src/traits.rs";

    // --- Find the relevant graph ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .expect("ParsedCodeGraph for traits.rs not found")
        .graph;

    let module_id_crate =
        find_inline_module_by_path(graph, &["crate".to_string(), "traits".to_string()])
            .expect("Failed to find top-level module node")
            .id();
    let module_id_inner = find_inline_module_by_path(
        graph,
        &[
            "crate".to_string(),
            "traits".to_string(),
            "inner".to_string(),
        ],
    )
    .expect("Failed to find inner module node")
    .id();

    // --- Test Individual Traits ---

    // InternalTrait (private)
    let trait_name = "InternalTrait";
    let module_path = vec!["crate".to_string(), "traits".to_string()];
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    assert_eq!(node.methods.len(), 1); // default_method
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );
    // TODO: Add assertion for trait method if/when implemented here for
    // "InternalTrait->default_method"

    // CrateTrait (crate visible)
    let trait_name = "CrateTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(
        node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()])
    ); // pub(crate)
    assert_eq!(node.methods.len(), 1); // crate_method
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // DocumentedTrait (documented)
    let trait_name = "DocumentedTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert!(node.docstring.is_some());
    assert_eq!(node.docstring.as_deref(), Some("Documented public trait"));
    assert_eq!(node.methods.len(), 1); // documented_method
    assert!(node.methods[0].docstring.is_some()); // Check method docstring too
    assert_eq!(
        node.methods[0].docstring.as_deref(),
        Some("Required method documentation") // Note leading whitespace already stripped
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // GenericTrait<T>
    let trait_name = "GenericTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.generic_params.len(), 1);
    assert_eq!(node.methods.len(), 1); // process
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // LifetimeTrait<'a>
    let trait_name = "LifetimeTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.generic_params.len(), 1);
    // TODO: Check generic param is lifetime 'a'
    assert_eq!(node.methods.len(), 1); // get_ref
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // AssocTypeTrait
    let trait_name = "AssocTypeTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    // NOTE: Associated types are not stored directly on TraitNode yet
    assert_eq!(node.methods.len(), 1); // generate
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // AssocTypeWithBounds
    let trait_name = "AssocTypeWithBounds";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    // NOTE: Associated types are not stored directly on TraitNode yet
    assert_eq!(node.methods.len(), 1); // generate_bounded
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // AssocConstTrait
    let trait_name = "AssocConstTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    // NOTE: Associated consts are not stored directly on TraitNode yet
    assert_eq!(node.methods.len(), 1); // get_id
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // SuperTrait: SimpleTrait
    let trait_name = "SuperTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.super_traits.len(), 1);
    // Check the supertrait TypeId corresponds to SimpleTrait
    let super_trait_id = node.super_traits[0];
    let super_trait_type = find_type_node(graph, super_trait_id);
    assert!(
        matches!(&super_trait_type.kind, TypeKind::Named { path, .. } if path == &["SimpleTrait"]),
        "\nExpected path: '&[\"SimpleTrait\"]' for TypeKind::Named in TypeNode, found: 
    TypeKind::Named path:{:?}
    Complete super_trait TypeNode: 
{:#?}",
        &super_trait_type.kind,
        &super_trait_type
    );
    assert_eq!(node.methods.len(), 1); // super_method
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // MultiSuperTrait: SimpleTrait + InternalTrait + Debug
    let trait_name = "MultiSuperTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.super_traits.len(), 3);
    // TODO: Check all 3 supertrait TypeIds (SimpleTrait, InternalTrait, Debug)
    assert_eq!(node.methods.len(), 1); // multi_super_method
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // GenericSuperTrait<T>: GenericTrait<T>
    let trait_name = "GenericSuperTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.generic_params.len(), 1); // <T>
    assert_eq!(node.super_traits.len(), 1);
    // Check supertrait TypeId corresponds to GenericTrait<T>
    let super_trait_id = node.super_traits[0];
    let super_trait_type = find_type_node(graph, super_trait_id);
    assert!(
        matches!(&super_trait_type.kind, TypeKind::Named { path, .. } if path == &["GenericTrait"])
    );
    assert_eq!(super_trait_type.related_types.len(), 1); // <T>
    assert_eq!(node.methods.len(), 1); // generic_super_method
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // AttributedTrait
    let trait_name = "AttributedTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.attributes.len(), 1);
    assert_eq!(node.attributes[0].name, "must_use");
    assert_eq!(
        node.attributes[0].value.as_deref(),
        Some("Trait results should be used")
    );
    assert_eq!(node.methods.len(), 1); // calculate
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // UnsafeTrait
    let trait_name = "UnsafeTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    // TODO: Check if TraitNode has an `is_unsafe` flag - currently it doesn't seem to.
    assert_eq!(node.methods.len(), 1); // unsafe_method
                                       // TODO: Check if method_node has an `is_unsafe` flag.
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // --- Traits inside `inner` module ---
    let module_path_inner = vec![
        "crate".to_string(),
        "traits".to_string(),
        "inner".to_string(),
    ];

    // InnerSecretTrait (private in private mod)
    let trait_name = "InnerSecretTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        trait_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    assert_eq!(node.methods.len(), 1); // secret_op
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // InnerPublicTrait (pub in private mod)
    let trait_name = "InnerPublicTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        trait_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Public); // Public within its module
    assert_eq!(node.methods.len(), 1); // public_inner_op
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // SuperGraphNodeTrait (pub(super))
    let trait_name = "SuperGraphNodeTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        trait_name,
    );
    assert_eq!(
        node.visibility(),
        VisibilityKind::Restricted(vec!["super".to_string()])
    ); // pub(super)
    assert_eq!(node.super_traits.len(), 1); // super::SimpleTrait
    assert_eq!(node.methods.len(), 1); // super_visible_op
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // --- Traits with Self usage ---
    let module_path = vec!["crate".to_string(), "traits".to_string()]; // Back to top level

    // SelfUsageTrait
    let trait_name = "SelfUsageTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.methods.len(), 2);
    // TODO: Check method signatures involving Self
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // SelfInAssocBound
    let trait_name = "SelfInAssocBound";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    // NOTE: Associated types not stored on TraitNode yet
    assert_eq!(node.methods.len(), 1); // get_related
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );
}
