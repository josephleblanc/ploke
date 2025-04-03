//! **Module System Edge Cases**:
//!    - Nested module visibility (`mod outer { pub mod inner {} }`)
//!    - More complex module hierarchies than tested in visibility files
//!    TODO: Add more test documentation and edge cases
#![cfg(feature = "visibility_resolution")]

use crate::common::{find_function_by_name, find_module_by_path, parse_fixture};
use syn_parser::{
    parser::{
        nodes::{NodeId, OutOfScopeReason, VisibilityResult},
        relations::RelationKind,
        types::VisibilityKind,
    },
    CodeGraph,
};
// Key aspects of this implementation:
//
// 1. **Module Edge Cases Tested**:
//    - Deeply nested module hierarchies
//    - Cross-module visibility boundaries
//    - `pub(in path)` restricted visibility
//    - Re-export chains and their visibility effects
//
// 2. **Helper Functions**:
//    - `get_visibility_info`: Provides consistent (id, name) access for TypeDefNodes
//    - Uses existing `find_module_by_path` and `find_function_by_name`
//
// 3. **Test Patterns**:
//    ```rust
//    // Positive case - visible through module chain
//    assert!(visible_in_hierarchy());
//
//    // Negative case - blocked across modules
//    assert!(matches!(result, OutOfScope {..}));
//
//    // Re-export case
//    assert!(visible_through_reexport());
//    ```
//
// 4. **Edge Cases Covered**:
//    - Multiple levels of nesting
//    - Mixed pub/private modules
//    - Visibility through re-exports
//    - Path-restricted visibility
//
// The tests expect a `modules.rs` fixture with:
// ```rust
// mod outer {
//     pub mod middle {
//         pub mod inner {
//             pub fn deep_function() {}
//         }
//     }
// }
//
// mod unrelated {
//     // Should not see outer::middle::inner items
// }
//
// mod allowed_parent {
//     pub(in crate::allowed_parent) fn restricted_fn() {}
// }
//
// mod intermediate {
//     pub use super::deeply::nested::nested_export_fn;
// }
// ```

#[test]
fn test_nested_module_visibility() {
    let code_graph = parse_fixture("sample.rs")
        .expect("Failed to parse modules.rs - file missing or invalid syntax");

    // Test deeply nested public item
    let deep_function =
        find_function_by_name(&code_graph, "deep_function").expect("deep_function not found");

    let allowed_context = &[
        "crate".to_string(),
        "outer".to_string(),
        "middle".to_string(),
        "inner".to_string(),
    ];
    assert!(
        matches!(
            code_graph.resolve_visibility(deep_function.id, allowed_context),
            VisibilityResult::Direct
        ),
        "Function should be visible in same module hierarchy"
    );

    // Test cross-module access
    let outer_module_path = ["crate".to_string(), "outer".to_string()];
    let outer_module =
        find_module_by_path(&code_graph, &outer_module_path).expect("outer module not found");

    let unrelated_path = &["crate".to_string(), "unrelated".to_string()];

    let actual_result = code_graph.resolve_visibility(deep_function.id, unrelated_path);

    let container_module = code_graph
        .modules
        .iter()
        .find(|m| m.items.contains(&deep_function.id));
    let relation_contains_id = code_graph
        .relations
        .iter()
        .find(|r| r.target == deep_function.id && r.kind == RelationKind::Contains)
        .unwrap()
        .source;
    let detected_container_by_relation = code_graph
        .modules
        .iter()
        .find(|m| m.id == relation_contains_id);

    let expected_result = VisibilityResult::NeedsUse(vec![
        "crate".to_string(),
        "outer".to_string(),
        "middle".to_string(),
        "inner".to_string(),
    ]);
    assert!(
        expected_result == actual_result,
        "\nNested function should be blocked outside module chain. Instead:
---context_module: {:?}
---deep_function:
{:#?}
---deep_function container module detected by module items:
    let container_module = code_graph
        .modules
        .iter()
        .find(|m| m.items.contains(&deep_function.id));
{:#?}
---deep_function detected by relation:
    let relation_contains_id = code_graph
        .relations
        .iter()
        .find(|r| r.target == deep_function.id && r.kind == RelationKind::Contains)
        .unwrap()
        .source;
    let detected_container_by_relation = code_graph
        .modules
        .iter()
        .find(|m| m.id == relation_contains_id);
{:#?}
---detection of deep_function by module items == detection by relation:
{}
-----
Expected VisibilityResult: {:#?}
Actual VisibilityResult: {:#?}
-----",
        unrelated_path,
        deep_function,
        container_module,
        detected_container_by_relation,
        container_module.unwrap().id == detected_container_by_relation.unwrap().id,
        expected_result,
        actual_result
    );

    // Test restricted pub(in path)
    let restricted_fn =
        find_function_by_name(&code_graph, "restricted_fn").expect("restricted_fn not found");

    let allowed_restricted = &["crate".to_string(), "allowed_parent".to_string()];
    assert!(
        matches!(
            code_graph.resolve_visibility(restricted_fn.id, allowed_restricted),
            VisibilityResult::Direct
        ),
        "pub(in path) item should be visible in specified parent"
    );
}

#[test]
fn test_module_re_exports() {
    let code_graph = parse_fixture("sample.rs").expect("Failed to parse modules.rs");

    // Test re-exported item visibility
    let re_exported_fn =
        find_function_by_name(&code_graph, "re_exported_fn").expect("re_exported_fn not found");

    assert!(
        matches!(
            code_graph.resolve_visibility(re_exported_fn.id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "Re-exported function {} (id: {}) should be visible at crate root {:?}. 
Instead found module path {:?} for function (id: {})",
        re_exported_fn.name,
        re_exported_fn.id,
        ["crate".to_string()],
        code_graph.get_item_module_path(re_exported_fn.id),
        re_exported_fn.id,
    );

    // Test nested re-export
    let nested_export_fn =
        find_function_by_name(&code_graph, "nested_export_fn").expect("nested_export_fn not found");

    let intermediate_context = &["crate".to_string(), "intermediate".to_string()];
    assert!(
        matches!(
            code_graph.resolve_visibility(nested_export_fn.id, intermediate_context),
            VisibilityResult::Direct
        ),
        "Nested re-export should be visible through export chain"
    );
}
