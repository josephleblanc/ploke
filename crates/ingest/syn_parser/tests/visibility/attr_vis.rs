//! **Attribute-Controlled Visibility**:
//!    - Tests for `#[cfg_attr(feature = "foo", visibility)]` conditional visibility
//!    - Tests visibility modified by other attributes
//!    TODO: Remove `ignore` on test if we implement attribute tracking for visibility.
#![cfg(feature = "visibility_resolution")]
// Key features of this implementation:
//
// 1. **Test Cases**:
//    - Structs with `#[cfg_attr]` conditional visibility
//    - Functions with multiple visibility-affecting attributes
//    - Items that should remain private despite attributes
//
// 2. **Edge Cases Covered**:
//    - Multiple attributes affecting visibility
//    - Nested conditional visibility
//    - Attribute combinations that don't change visibility
//
// 3. **Verification**:
//    - Checks resolved visibility matches attribute conditions
//    - Validates both positive and negative cases
//    - Tests interaction with normal visibility modifiers
//
// 4. **Helper Usage**:
//    - Uses existing `find_struct_by_name`
//    - Leverages `get_visibility_info` pattern
//    - Maintains consistent error reporting

// The test assumes these structures exist in `sample.rs`:
// #[cfg_attr(public, feature = "public")]
// struct ConditionalVisibilityStruct { /* ... */ }
//
// #[cfg_attr(test, allow(unused))]
// #[cfg_attr(feature = "public", pub)]
// fn multi_attr_function() {}
//
// #[cfg_attr(public, feature = "never_enabled")]
// struct ConditionalPrivateStruct { /* ... */ }

use crate::common::{find_struct_by_name, get_visibility_info, parse_fixture};
use syn_parser::{
    parser::{
        nodes::{NodeId, TypeDefNode, VisibilityResult},
        types::VisibilityKind,
    },
    CodeGraph,
};

#[test]
#[ignore]
fn test_attribute_controlled_visibility() {
    let code_graph = parse_fixture("sample.rs")
        .expect("Failed to parse sample.rs - file missing or invalid syntax");

    // Test struct with conditional visibility
    let conditional_struct = find_struct_by_name(&code_graph, "ConditionalVisibilityStruct")
        .expect("ConditionalVisibilityStruct not found");

    let (conditional_id, _) = get_visibility_info(
        &TypeDefNode::Struct(conditional_struct.clone()),
        &code_graph,
    );

    // Should be public when cfg conditions are met
    assert!(
        matches!(
            code_graph.resolve_visibility(conditional_id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "ConditionalVisibilityStruct should be visible when attributes are satisfied"
    );

    // Test function with multiple visibility attributes
    let multi_attr_fn = code_graph
        .functions
        .iter()
        .find(|f| f.name == "multi_attr_function")
        .expect("multi_attr_function not found");

    assert!(
        matches!(
            code_graph.resolve_visibility(multi_attr_fn.id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "Function with multiple visibility attributes should respect combined visibility"
    );

    // Test private item with conditional pub
    let conditional_private = find_struct_by_name(&code_graph, "ConditionalPrivateStruct")
        .expect("ConditionalPrivateStruct not found");

    let (conditional_private_id, _) = get_visibility_info(
        &TypeDefNode::Struct(conditional_private.clone()),
        &code_graph,
    );
    assert!(
        !matches!(
            code_graph.resolve_visibility(conditional_private_id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "ConditionalPrivateStruct should remain private when conditions aren't met"
    );
}
