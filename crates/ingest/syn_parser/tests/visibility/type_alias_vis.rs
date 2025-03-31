//! **Type Alias Visibility**:                                           
//!    - Tests for `pub type StringVec` and other type aliases           
//!    - Visibility tests focus on structs/enums but don't explicitly te
//! type alias visibility                                                    
//!    TODO: Add more test documentation and edge cases                  
#![cfg(feature = "visibility_resolution")]

use crate::common::{
    find_function_by_name, find_struct_by_name, get_visibility_info, parse_fixture,
};
use syn_parser::{
    parser::nodes::{NodeId, TypeDefNode, VisibilityResult},
    CodeGraph,
};

#[test]
fn test_type_alias_visibility() {
    let code_graph = parse_fixture("sample.rs")
        .expect("Failed to parse sample.rs - file missing or invalid syntax");

    // Helper to find type alias by name
    fn find_type_alias<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a TypeDefNode> {
        graph.defined_types.iter().find(|t| match t {
            TypeDefNode::TypeAlias(a) => a.name == name,
            _ => false,
        })
    }

    // Test public type alias
    let string_vec =
        find_type_alias(&code_graph, "StringVec").expect("StringVec type alias not found");

    let (string_vec_id, _) = get_visibility_info(string_vec, &code_graph);
    assert!(
        matches!(
            code_graph.resolve_visibility(string_vec_id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "StringVec should be publicly visible"
    );

    // Test private type alias
    let private_alias =
        find_type_alias(&code_graph, "PrivateTypeAlias").expect("PrivateTypeAlias not found");

    let (private_alias_id, _) = get_visibility_info(private_alias, &code_graph);
    assert!(
        !matches!(
            code_graph.resolve_visibility(private_alias_id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "PrivateTypeAlias should not be publicly visible"
    );

    // Test type alias in module
    let module_alias =
        find_type_alias(&code_graph, "ModuleTypeAlias").expect("ModuleTypeAlias not found");

    let (module_alias_id, _) = get_visibility_info(module_alias, &code_graph);
    let module_context = &["crate".to_string(), "public_module".to_string()];
    assert!(
        matches!(
            code_graph.resolve_visibility(module_alias_id, module_context),
            VisibilityResult::Direct
        ),
        "ModuleTypeAlias should be visible within its module"
    );
}
