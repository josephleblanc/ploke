#![cfg(feature = "uuid_ids")] // Gate the whole module
use crate::common::uuid_ids_utils::*;
use ploke_common::{fixtures_crates_dir, workspace_root};
use ploke_core::{NodeId, TypeId};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use syn_parser::parser::nodes::ImplNode; // Import ImplNode specifically
use syn_parser::parser::nodes::TraitNode; // Import TraitNode specifically
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
            FieldNode, FunctionNode, ImportNode, ModuleNode, StructNode, TypeDefNode, ValueNode,
            Visible,
        },
        relations::{GraphId, Relation, RelationKind},
        types::{GenericParamKind, TypeNode},
        visitor::ParsedCodeGraph,
    },
};
use uuid::Uuid;

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
    let module_path = vec!["crate".to_string()]; // Defined at top level of file

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
    let module_path = vec!["crate".to_string()];

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

// TODO: Add combined test for remaining impl blocks
