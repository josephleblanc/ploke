//! **Union Type Visibility**:
//!    - Tests for `pub union IntOrFloat`
//!    - Unions aren't covered in the visibility test files
//!    TODO: Add more test documentation and edge cases
#![cfg(feature = "visibility_resolution")]
// Key additions for union testing:
//
// 1. Specific union visibility tests:
//    - Public union (`IntOrFloat`)
//    - Private union (`PrivateUnion`)
//    - Field-level visibility checks
//
// 2. New helper function `find_union` to locate unions in the graph
//
// 3. Tests both type-level and field-level visibility
//
// 4. Verifies the Rust requirement that union fields must all be public

use crate::common::{
    find_function_by_name, find_struct_by_name, get_visibility_info, parse_fixture,
};
use syn_parser::{
    parser::{
        nodes::{NodeId, TypeDefNode, VisibilityResult},
        types::VisibilityKind,
    },
    CodeGraph,
};

#[test]
fn test_union_visibility() {
    let code_graph = parse_fixture("sample.rs")
        .expect("Failed to parse sample.rs - file missing or invalid syntax");

    // Helper to find union by name
    fn find_union<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a TypeDefNode> {
        graph.defined_types.iter().find(|t| match t {
            TypeDefNode::Union(u) => u.name == name,
            _ => false,
        })
    }

    // Test public union
    let int_or_float = find_union(&code_graph, "IntOrFloat").expect("IntOrFloat union not found");

    let (union_id, _) = get_visibility_info(int_or_float, &code_graph);
    assert!(
        matches!(
            code_graph.resolve_visibility(union_id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "IntOrFloat should be publicly visible"
    );

    // Test private union
    let private_union = find_union(&code_graph, "PrivateUnion").expect("PrivateUnion not found");

    let (private_union_id, _) = get_visibility_info(private_union, &code_graph);
    assert!(
        !matches!(
            code_graph.resolve_visibility(private_union_id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "PrivateUnion should not be publicly visible"
    );

    // Test union field visibility
    if let TypeDefNode::Union(u) = int_or_float {
        assert_eq!(
            u.visibility,
            VisibilityKind::Public,
            "Union itself should be public"
        );
        assert!(
            u.fields
                .iter()
                .all(|f| f.visibility == VisibilityKind::Public),
            "All union fields should be public"
        );
    }
}
