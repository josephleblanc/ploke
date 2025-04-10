#![cfg(feature = "uuid_ids")] // Gate the whole module
use crate::common::uuid_ids_utils::*;
use ploke_core::{NodeId, TypeId};
use syn_parser::parser::nodes::ImplNode; // Import UnionNode specifically
use syn_parser::parser::types::TypeKind; // Import EnumNode specifically
use syn_parser::parser::types::VisibilityKind;
use syn_parser::parser::{
    nodes::{FunctionNode, Visible},
    relations::{GraphId, RelationKind},
    types::GenericParamKind,
};

// --- Test Cases ---

#[test]
fn test_impl_node_simple_struct_inherent_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let self_type_str = "SimpleStruct"; // Match the type name string
    let trait_type_str = None;
    let relative_file_path = "src/impls.rs";
    let module_path = vec!["crate".to_string(), "impls".to_string()]; // Defined at top level of file

    let impl_node = find_impl_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        self_type_str,
        trait_type_str,
    );

    // --- Assertions ---
    let graph = &results // Need graph for type/relation lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(impl_node.id(), NodeId::Synthetic(_)));
    // assert!(impl_node.tracking_hash.is_some()); // ImplNode doesn't have tracking hash currently
    assert!(impl_node.generic_params.is_empty()); // Not a generic impl block
    assert!(impl_node.trait_type.is_none()); // Inherent impl

    // Self Type (SimpleStruct)
    assert!(matches!(impl_node.self_type, TypeId::Synthetic(_)));
    let self_type_node = find_type_node(graph, impl_node.self_type);
    assert!(
        matches!(&self_type_node.kind, TypeKind::Named { path, .. } if path == &[self_type_str])
    );

    // Methods (new, private_method, public_method)
    assert_eq!(impl_node.methods.len(), 3);

    // Check 'new' method briefly
    let method_new = impl_node
        .methods
        .iter()
        .find(|m| m.name() == "new")
        .expect("Method 'new' not found");
    assert!(matches!(method_new.id(), NodeId::Synthetic(_)));
    assert!(method_new.tracking_hash.is_some());
    assert_eq!(method_new.visibility(), VisibilityKind::Public);
    assert_eq!(method_new.parameters.len(), 1); // data: i32
    assert!(method_new.return_type.is_some()); // -> Self
    let ret_type_id = method_new.return_type.unwrap();
    let ret_type_node = find_type_node(graph, ret_type_id);
    // Expect the TypeId for "Self" here, which is generic in Phase 2
    assert!(matches!(&ret_type_node.kind, TypeKind::Named { path, .. } if path == &["Self"]));

    // Check 'private_method' briefly
    let method_private = impl_node
        .methods
        .iter()
        .find(|m| m.name() == "private_method")
        .expect("Method 'private_method' not found");
    assert!(matches!(method_private.id(), NodeId::Synthetic(_)));
    assert!(method_private.tracking_hash.is_some());
    assert_eq!(method_private.visibility(), VisibilityKind::Inherited); // Private method
    assert_eq!(method_private.parameters.len(), 1); // &self
    assert!(method_private.parameters[0].is_self);
    assert!(method_private.return_type.is_some()); // -> i32

    // Check 'public_method' briefly
    let method_public = impl_node
        .methods
        .iter()
        .find(|m| m.name() == "public_method")
        .expect("Method 'public_method' not found");
    assert!(matches!(method_public.id(), NodeId::Synthetic(_)));
    assert!(method_public.tracking_hash.is_some());
    assert_eq!(method_public.visibility(), VisibilityKind::Public);
    assert_eq!(method_public.parameters.len(), 1); // &self
    assert!(method_public.parameters[0].is_self);
    assert!(method_public.return_type.is_some()); // -> i32

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains Impl
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(impl_node.id()),
        RelationKind::Contains,
        "Expected ModuleNode to Contain ImplNode",
    );

    // 2. Impl Implements For Self Type
    assert_relation_exists(
        graph,
        GraphId::Node(impl_node.id()),
        GraphId::Type(impl_node.self_type),
        RelationKind::ImplementsFor, // Correct kind for inherent impl
        "Expected ImplNode to have ImplementsFor relation to Self Type",
    );

    // 3. Impl does NOT Implement Trait
    assert_relation_does_not_exist(
        graph,
        GraphId::Node(impl_node.id()),
        GraphId::Type(impl_node.self_type), // Target doesn't matter as much here
        RelationKind::ImplementsTrait,
        "Expected ImplNode NOT to have ImplementsTrait relation (inherent impl)",
    );

    // 4. Method Relations (Param/Return - checked implicitly by finding types)
    // No explicit Impl->Method relation is created in Phase 2
}

#[test]
fn test_impl_node_generic_trait_for_generic_struct_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    // Need the string representations as generated by to_string()
    // GenericStruct<T> -> "GenericStruct < T >"
    // GenericTrait<T> -> "GenericTrait < T >"
    let self_type_str = "GenericStruct < T >";
    let trait_type_str = Some("GenericTrait < T >");
    let relative_file_path = "src/impls.rs";
    let module_path = vec!["crate".to_string(), "impls".to_string()];

    let impl_node = find_impl_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        self_type_str,
        trait_type_str,
    );

    // --- Assertions ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(impl_node.id(), NodeId::Synthetic(_)));
    // assert!(impl_node.tracking_hash.is_some()); // ImplNode doesn't have tracking hash
    assert!(impl_node.trait_type.is_some()); // Trait impl

    // Generics (on impl block: <T: Clone>)
    assert_eq!(impl_node.generic_params.len(), 1);
    let generic_param = &impl_node.generic_params[0];
    match &generic_param.kind {
        GenericParamKind::Type {
            name,
            bounds,
            default,
        } => {
            assert_eq!(name, "T");
            assert_eq!(bounds.len(), 1, "Expected one bound (Clone)");
            let bound_type = find_type_node(graph, bounds[0]);
            // Check bound is Clone
            assert!(
                matches!(&bound_type.kind, TypeKind::Named { path, .. } if path.ends_with(&["Clone".to_string()]))
            );
            assert!(default.is_none());
        }
        _ => panic!(
            "Expected GenericParamKind::Type, found {:?}",
            generic_param.kind
        ),
    }

    // Self Type (GenericStruct<T>) - Check presence and that it's Synthetic
    assert!(
        matches!(impl_node.self_type, TypeId::Synthetic(_)),
        "Expected self_type to be a Synthetic TypeId"
    );
    // NOTE: Skipping detailed check of the self_type's TypeNode kind and related_types
    // due to potential brittleness of TypeId lookup via to_string() for generics in Phase 2.
    let self_type_node = find_type_node(graph, impl_node.self_type);
    assert!(
        matches!(&self_type_node.kind, TypeKind::Named { path, .. } if path == &["GenericStruct"])
    );
    assert_eq!(self_type_node.related_types.len(), 1); // T
    let related_self_t = find_type_node(graph, self_type_node.related_types[0]);
    assert!(matches!(&related_self_t.kind, TypeKind::Named { path, .. } if path == &["T"]));

    // Trait Type (GenericTrait<T>) - Check presence and that it's Synthetic
    let trait_type_id = impl_node
        .trait_type
        .expect("Expected a trait_type for this impl");
    assert!(
        matches!(trait_type_id, TypeId::Synthetic(_)),
        "Expected trait_type to be a Synthetic TypeId"
    );
    // NOTE: Skipping detailed check of the trait_type's TypeNode kind and related_types
    // due to potential brittleness of TypeId lookup via to_string() for generics in Phase 2.
    let trait_type_node = find_type_node(graph, trait_type_id);
    assert!(
        matches!(&trait_type_node.kind, TypeKind::Named { path, .. } if path == &["GenericTrait"])
    );
    assert_eq!(trait_type_node.related_types.len(), 1); // T
    let related_trait_t = find_type_node(graph, trait_type_node.related_types[0]);
    assert!(matches!(&related_trait_t.kind, TypeKind::Named { path, .. } if path == &["T"]));
    assert_eq!(
        self_type_node.related_types[0], trait_type_node.related_types[0],
        "TypeId for 'T' should be consistent"
    );

    // Methods (generic_trait_method)
    assert_eq!(impl_node.methods.len(), 1);
    let method_node = &impl_node.methods[0];
    assert_eq!(method_node.name(), "generic_trait_method");
    assert!(matches!(method_node.id(), NodeId::Synthetic(_)));
    assert!(method_node.tracking_hash.is_some());
    assert_eq!(method_node.visibility(), VisibilityKind::Inherited); // Trait methods are implicitly pub unless specified otherwise? Check this.
    assert_eq!(method_node.parameters.len(), 2); // &self, value: T
    assert!(method_node.parameters[0].is_self);
    let param_value_type = find_type_node(graph, method_node.parameters[1].type_id);
    assert!(matches!(&param_value_type.kind, TypeKind::Named { path, .. } if path == &["T"]));
    assert!(method_node.return_type.is_none()); // Implicit unit

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains Impl
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(impl_node.id()),
        RelationKind::Contains,
        "Module->Impl",
    );

    // 2. Impl Implements Trait for Self Type
    assert_relation_exists(
        graph,
        GraphId::Node(impl_node.id()),
        GraphId::Type(impl_node.self_type),
        RelationKind::ImplementsTrait, // Correct kind for trait impl
        "Expected ImplNode to have ImplementsTrait relation to Self Type",
    );
    assert_relation_exists(
        graph,
        GraphId::Node(impl_node.id()),
        GraphId::Type(trait_type_id), // Use the unwrapped trait_type_id
        RelationKind::ImplementsTrait,
        "Expected ImplNode to have ImplementsTrait relation to Trait Type",
    );

    // 3. Impl does NOT Implement For
    assert_relation_does_not_exist(
        graph,
        GraphId::Node(impl_node.id()),
        GraphId::Type(impl_node.self_type),
        RelationKind::ImplementsFor,
        "Expected ImplNode NOT to have ImplementsFor relation (trait impl)",
    );
}

#[test]
#[should_panic(
    expected = "TypeId for Self in SimpleStruct::new should be different from TypeId for Self in GenericStruct::print_value parameter"
)] // Expecting this to fail currently
fn test_impl_node_self_type_conflation_phase2() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let relative_file_path = "src/impls.rs";
    let module_path = vec!["crate".to_string(), "impls".to_string()];

    // --- Get Data for impl SimpleStruct ---
    let self_type_str_ss = "SimpleStruct";
    let impl_node_ss = find_impl_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        self_type_str_ss,
        None, // Inherent impl
    );
    let type_id_simple_struct = impl_node_ss.self_type; // TypeId for SimpleStruct

    // Find the 'new' method and its return TypeId (which is Self)
    let method_new = impl_node_ss
        .methods
        .iter()
        .find(|m| m.name() == "new")
        .expect("Method 'new' not found");
    let type_id_self_return_ss = method_new
        .return_type
        .expect("Expected return type for new method");

    // --- Get Data for impl<T: Debug> GenericStruct<T> ---
    // Note: Finding this impl is tricky with the current helper because the self_type string is complex.
    // We might need to iterate manually or enhance the helper later.
    // For now, let's find the method directly and assume it belongs to the correct impl.
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Find the print_value function node
    // We need a way to reliably find this specific function node. Let's assume find_function_node_paranoid works.
    // We need its parent impl context to be sure, but let's try finding the function first.
    let method_print_value_candidates: Vec<&FunctionNode> = graph
        .functions
        .iter()
        .filter(|f| f.name() == "print_value")
        .collect();
    // We expect this method to be associated with an ImplNode, not directly in the module.
    // Let's find the ImplNode that contains this method.
    let mut parent_impl_node_gs: Option<&ImplNode> = None;
    let mut method_print_value_node: Option<&FunctionNode> = None;
    for impl_node in graph.impls.iter() {
        if let Some(method) = impl_node.methods.iter().find(|m| m.name() == "print_value") {
            // PARANOID CHECK: Ensure only one impl contains this method name
            assert!(
                parent_impl_node_gs.is_none(),
                "Found multiple impls containing 'print_value'"
            );
            parent_impl_node_gs = Some(impl_node);
            method_print_value_node = Some(method);
        }
    }
    let method_print_value =
        method_print_value_node.expect("Method 'print_value' not found in any impl block");

    // Get the TypeId for the `&self` parameter's underlying `Self` type
    assert!(
        method_print_value.parameters[0].is_self,
        "'print_value' first param should be self"
    );
    let self_param_type_id_gs = method_print_value.parameters[0].type_id; // This is TypeId for `& Self`
    let self_param_type_node_gs = find_type_node(graph, self_param_type_id_gs); // This is TypeNode for `& Self`
    assert!(
        matches!(self_param_type_node_gs.kind, TypeKind::Reference { .. }),
        "Expected &Self param type to be a Reference"
    );
    let type_id_self_param_gs = self_param_type_node_gs.related_types[0]; // This should be the TypeId for `Self`

    // --- Assertions ---

    // 1. (Should Pass) Assert that the TypeId for `Self` return type in SimpleStruct::new
    //    is NOT the same as the TypeId for the concrete type `SimpleStruct`.
    assert_ne!(
        type_id_self_return_ss, type_id_simple_struct,
        "TypeId for 'Self' return type ({:?}) should not match TypeId for 'SimpleStruct' ({:?}) in Phase 2",
        type_id_self_return_ss, type_id_simple_struct
    );

    // 2. (Should Fail - Demonstrating Conflation) Assert that the TypeId for `Self` return type
    //    in SimpleStruct::new IS THE SAME as the TypeId for the underlying `Self` type from
    //    the `&self` parameter in GenericStruct<T>::print_value.
    assert_ne!( // Use assert_ne! here because the test should panic if they ARE equal
        type_id_self_return_ss, type_id_self_param_gs,
        "TypeId for Self in SimpleStruct::new should be different from TypeId for Self in GenericStruct::print_value parameter" // This message is for the #[should_panic]
    );

    // If the above assert_ne! passes, it means the TypeIds were different, which is unexpected. Panic manually.
    // Note: This manual panic might not be hit if the assert_ne! itself panics due to equality.
    // The #[should_panic] attribute is the primary mechanism here.
    // panic!("TEST FAILED: TypeId for Self was unexpectedly different across impl blocks: {:?} vs {:?}", type_id_self_return_ss, type_id_self_param_gs);

    // WARNING: If the previous test passed then this test MUST fail (assert_eq! vs assert_ne!)
    // This is only here as a sanity check and debug temporarily. Delete one of the two assertions
    // after debugging is over and tests are stable
    //     assert_eq!(
    //         // Use assert_ne! here because the test should panic if they ARE equal
    //         type_id_self_return_ss,
    //         type_id_self_param_gs,
    //         "typid of self_id_self_return_ss: {type_id_self_return_ss}
    // typeid of type_id_self_param_gs: {type_id_self_param_gs}
    // CONFIRMED_NE type_id_simple_struct: {type_id_simple_struct}
    //
    // parent_impl_node_gs: {:#?}
    // method_print_value: {:#?}",
    //         parent_impl_node_gs,
    //         method_print_value
    //     );
}

// TODO: Add combined test for remaining impl blocks
