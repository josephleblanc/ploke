//! Helpers for Phase 2 tests asserting on the typed structural `TypeNode` graph.

use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::AnyTypeId;
use syn_parser::parser::type_slots::{OrdinaryTypeUseId, TraitTypeUseId};
use syn_parser::parser::types::TypeNode;

pub fn find_type_node<'a, G: GraphAccess>(
    graph: &'a G,
    type_id: impl Into<AnyTypeId>,
) -> &'a TypeNode {
    let any = type_id.into();
    graph
        .type_graph()
        .iter()
        .find(|node| node.id() == any)
        .unwrap_or_else(|| panic!("TypeNode not found for {any:?}"))
}

pub fn find_ordinary_type_node<'a, G: GraphAccess>(
    graph: &'a G,
    type_id: OrdinaryTypeUseId,
) -> &'a TypeNode {
    find_type_node(graph, AnyTypeId::from(type_id))
}

pub fn find_trait_type_node<'a, G: GraphAccess>(
    graph: &'a G,
    type_id: TraitTypeUseId,
) -> &'a TypeNode {
    find_type_node(graph, AnyTypeId::from(type_id))
}

pub fn assert_named_path(node: &TypeNode, expected: &[&str]) {
    match node {
        TypeNode::Named(named) => {
            let actual: Vec<&str> = named.path.iter().map(String::as_str).collect();
            assert_eq!(
                actual, expected,
                "Expected named path {expected:?}, found {actual:?}"
            );
        }
        other => panic!("Expected TypeNode::Named, found {other:?}"),
    }
}

pub fn assert_named_path_suffix(node: &TypeNode, suffix: &[&str]) {
    match node {
        TypeNode::Named(named) => {
            let actual: Vec<&str> = named.path.iter().map(String::as_str).collect();
            assert!(
                actual.ends_with(suffix),
                "Expected named path ending with {suffix:?}, found {actual:?}"
            );
        }
        other => panic!("Expected TypeNode::Named, found {other:?}"),
    }
}

pub fn assert_named_path_contains_segment(node: &TypeNode, segment: &str) {
    match node {
        TypeNode::Named(named) => assert!(
            named.path.iter().any(|part| part == segment),
            "Expected named path to contain `{segment}`, found {:?}",
            named.path
        ),
        other => panic!("Expected TypeNode::Named, found {other:?}"),
    }
}

pub fn assert_trait_bound_path_contains(node: &TypeNode, segment: &str) {
    match node {
        TypeNode::TraitBound(bound) => assert!(
            bound.path.iter().any(|part| part == segment),
            "Expected trait bound path to contain `{segment}`, found {:?}",
            bound.path
        ),
        other => panic!("Expected TypeNode::TraitBound, found {other:?}"),
    }
}

pub fn assert_ordinary_argument_count(node: &TypeNode, expected: usize) {
    match node {
        TypeNode::Named(named) => assert_eq!(
            named.arguments.len(),
            expected,
            "Expected {expected} named type arguments for path {:?}",
            named.path
        ),
        TypeNode::Tuple(tuple) => assert_eq!(
            tuple.elements.len(),
            expected,
            "Expected tuple to have {expected} elements"
        ),
        other => panic!("Expected named or tuple type node, found {other:?}"),
    }
}

pub fn ordinary_argument(node: &TypeNode, index: usize) -> OrdinaryTypeUseId {
    match node {
        TypeNode::Named(named) => *named
            .arguments
            .get(index)
            .unwrap_or_else(|| panic!("Named type argument index {index} out of bounds")),
        TypeNode::Tuple(tuple) => *tuple
            .elements
            .get(index)
            .unwrap_or_else(|| panic!("Tuple element index {index} out of bounds")),
        other => panic!("Expected named or tuple type node, found {other:?}"),
    }
}

pub fn assert_function_signature(
    node: &TypeNode,
    expected_params: usize,
    has_return: bool,
    is_unsafe: bool,
    is_extern: bool,
) {
    match node {
        TypeNode::Function(function) => {
            assert_eq!(function.parameters.len(), expected_params);
            assert_eq!(function.return_type.is_some(), has_return);
            assert_eq!(function.is_unsafe, is_unsafe);
            assert_eq!(function.is_extern, is_extern);
            assert!(function.abi.is_none());
        }
        other => panic!("Expected TypeNode::Function, found {other:?}"),
    }
}
