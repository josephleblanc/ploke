//! **Documentation Visibility**:
//!    - Tests visibility markers in documentation comments
//!    - Tests `#[doc(hidden)]` attribute behavior
//!    TODO: Add more test documentation and edge cases
#![cfg(feature = "visibility_resolution")]

use crate::{common::{find_function_by_name, find_struct_by_name, parse_fixture, get_visibility_info}};
use syn_parser::{
    parser::{
        nodes::{NodeId, TypeDefNode, VisibilityResult},
        types::{TypeKind, VisibilityKind},
    },
    CodeGraph,
};
// Key aspects of this implementation:
//
// 1. **Test Coverage**:
//    - Visibility markers in regular doc comments (`/// [Visibility]`)
//    - `#[doc(hidden)]` attribute behavior
//    - Documentation visibility inheritance
//    - Hidden items in impl blocks
//
// 2. **Test Cases**:
//    ```rust
//    // Positive case - docs shouldn't affect visibility
//    assert!(visible_with_docs());
//
//    // Negative case - #[doc(hidden)]
//    assert!(!visible_when_hidden());
//
//    // Impl block case
//    assert!(hidden_method_invisible());
//    ```
//
// 3. **Edge Cases**:
//    - Documentation on impl blocks
//    - Inherited visibility through docs
//    - Combination of doc attributes and visibility modifiers
//
// 4. **Fixture Requirements**:
// The test expects these in `sample.rs`:
// ```rust
// /// Struct with [Visibility] markers in docs
// pub struct DocumentedStruct;
//
// #[doc(hidden)]
// pub fn hidden_function() {}
//
// /// Inherits visibility from parent
// pub struct DocInheritanceStruct;
//
// impl DocumentedStruct {
//     #[doc(hidden)]
//     pub fn hidden_method() {}
// }

#[test]
fn test_documentation_visibility() {
    let code_graph = parse_fixture("sample.rs")
        .expect("Failed to parse sample.rs - file missing or invalid syntax");

    // Test struct with visibility markers in docs
    let documented_struct = find_struct_by_name(&code_graph, "DocumentedStruct")
        .expect("DocumentedStruct not found");

    let (doc_struct_id, _) = get_visibility_info(&TypeDefNode::Struct(documented_struct.clone()), &code_graph);
    
    assert!(
        matches!(
            code_graph.resolve_visibility(doc_struct_id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "Struct with visibility markers in docs should remain visible"
    );

    // Test #[doc(hidden)] item
    let hidden_function = find_function_by_name(&code_graph, "hidden_function")
        .expect("hidden_function not found");
    
    assert!(
        !matches!(
            code_graph.resolve_visibility(hidden_function.id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "Function marked with #[doc(hidden)] should not be publicly visible"
    );

    // Test documentation visibility inheritance
    let doc_inheritance_struct = find_struct_by_name(&code_graph, "DocInheritanceStruct")
        .expect("DocInheritanceStruct not found");
    
    let (doc_inheritance_id, _) = get_visibility_info(&TypeDefNode::Struct(doc_inheritance_struct.clone()), &code_graph);
    assert!(
        matches!(
            code_graph.resolve_visibility(doc_inheritance_id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "Struct inheriting visibility through docs should be visible"
    );
}

#[test]
fn test_doc_hidden_impl_items() {
    let code_graph = parse_fixture("sample.rs")
        .expect("Failed to parse sample.rs - file missing or invalid syntax");

    // Test #[doc(hidden)] method in impl block
    if let Some(impl_block) = code_graph.impls.iter().find(|i| {
        code_graph.type_graph.iter().any(|t| 
            t.id == i.self_type && 
            matches!(&t.kind, TypeKind::Named { path, .. } if path.last() == Some(&"DocumentedStruct".to_string()))
        )
    }) {
        let hidden_method = impl_block.methods.iter()
            .find(|m| m.name == "hidden_method")
            .expect("hidden_method not found in impl block");
        
        assert!(
            !matches!(
                code_graph.resolve_visibility(hidden_method.id, &["crate".to_string()]),
                VisibilityResult::Direct
            ),
            "Method marked with #[doc(hidden)] should not be visible"
        );
    }
}
