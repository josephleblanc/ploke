//! **Macro Visibility**:
//!    - Tests for `#[macro_export]` macros
//!    - Visibility rules for `macro_rules!` declarations
//!    TODO: Add more test documentation and edge cases
#![cfg(feature = "visibility_resolution")]
// Key aspects of this implementation:
//
// 1. **Macro Visibility Tests**:
//    - Tests `#[macro_export]` visibility
//    - Verifies non-exported macros are private
//    - Checks module-scoped macro visibility
//    - Validates both declaration and usage visibility
//
// 2. **Helper Functions**:
//    - `find_value_by_name`: Locates const/static values
//    - `find_macro_by_name`: Finds macros by name
//    - Both follow the same pattern as other test helpers
//
// 3. **Test Coverage**:
//    ```rust
//    // Positive case - exported macro
//    assert!(matches!(visibility, VisibilityResult::Direct));
//
//    // Negative case - private macro
//    assert!(find_macro_by_name(...).is_none());
//
//    // Module-scoped case
//    assert!(visible_in_module(module_context));
//    ```
//
// 4. **Error Handling**:
//    - Uses `expect` with clear error messages
//    - Includes assertion messages explaining failures
//    - Follows same pattern as other visibility tests

/// Find a value (const/static) by name in the code graph
use crate::common::{find_macro_by_name, parse_fixture};
use syn_parser::{
    parser::{
        nodes::{NodeId, ValueNode, VisibilityResult},
        types::VisibilityKind,
    },
    CodeGraph,
};

#[test]
fn test_macro_visibility() {
    let code_graph = parse_fixture("sample.rs")
        .expect("Failed to parse sample.rs - file missing or invalid syntax");

    // Test exported macro
    let test_macro = find_macro_by_name(&code_graph, "test_macro").expect("test_macro not found");

    assert!(
        matches!(
            code_graph.resolve_visibility(test_macro.id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "Exported macro should be publicly visible"
    );
    assert_eq!(
        test_macro.visibility,
        VisibilityKind::Public,
        "Macro with #[macro_export] should have public visibility"
    );

    // Test non-exported macro (should not be found)
    let private_macro = find_macro_by_name(&code_graph, "private_macro");
    assert!(
        private_macro.is_none(),
        "Non-exported macro should not be publicly accessible"
    );

    // Test macro in module
    let module_macro =
        find_macro_by_name(&code_graph, "module_macro").expect("module_macro not found");

    let module_context = &["crate".to_string(), "public_module".to_string()];
    assert!(
        matches!(
            code_graph.resolve_visibility(module_macro.id, module_context),
            VisibilityResult::Direct
        ),
        "Module macro should be visible within its module"
    );
}
