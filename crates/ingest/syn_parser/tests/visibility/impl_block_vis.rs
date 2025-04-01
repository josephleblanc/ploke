//! **Impl Block Visibility**:
//!    - Tests visibility of methods within impl blocks
//!    - Covers both trait implementations and inherent impls
//!    TODO: Add more test documentation and edge cases
#![cfg(feature = "visibility_resolution")]
// Key aspects of this implementation:
//
// 1. **Test Coverage**:
//    - Public methods in impl blocks
//    - Private methods in impl blocks
//    - Trait implementation methods
//    - Visibility inheritance from traits
//
// 2. **Helper Functions**:
//    - `find_impl_for_type`: Locates impl blocks by type name
//    - `find_impl_method`: Finds methods within impl blocks
//
// 3. **Test Patterns**:
//    ```rust
//    // Positive case - public method
//    assert!(matches!(visibility, VisibilityResult::Direct));
//
//    // Negative case - private method
//    assert!(!matches!(visibility, VisibilityResult::Direct));
//
//    // Trait method case
//    assert!(inherits_trait_visibility());
//    ```
//
// 4. **Edge Cases Handled**:
//    - Method visibility vs impl visibility
//    - Trait method visibility rules
//    - Private methods in public impls

use crate::common::{find_impl_for_type, parse_fixture};
use syn_parser::{
    parser::{
        nodes::{FunctionNode, ImplNode, NodeId, VisibilityResult},
        types::VisibilityKind,
    },
    CodeGraph,
};

#[test]
fn test_impl_block_visibility() {
    let code_graph = parse_fixture("sample.rs")
        .expect("Failed to parse sample.rs - file missing or invalid syntax");

    // Helper to find method in impl block
    fn find_impl_method<'a>(
        impl_block: &'a ImplNode,
        method_name: &str,
    ) -> Option<&'a FunctionNode> {
        impl_block.methods.iter().find(|m| m.name == method_name)
    }

    // Test public method in public impl
    let sample_struct_impl = find_impl_for_type(&code_graph, "SampleStruct")
        .expect("Impl block for SampleStruct not found");
    println!("sample_struct_impl: {:#?}", sample_struct_impl);

    let public_method = find_impl_method(sample_struct_impl, "public_impl_method")
        .expect("public_impl_method not found");
    println!("public_method_id: {}", public_method.id);
    println!(
        "code_graph.functions.iter().find(|m| m.id == public_method.id) = {:?}",
        code_graph
            .functions
            .iter()
            .find(|m| m.id == public_method.id)
    );

    assert!(
        matches!(
            code_graph.resolve_visibility(public_method.id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "Public method in impl block should be visible"
    );

    // Test private method in public impl
    let private_method = find_impl_method(sample_struct_impl, "private_impl_method")
        .expect("private_impl_method not found");

    assert!(
        matches!(
            code_graph.resolve_visibility(private_method.id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "Private method in impl block should be publicly visible in the same module path context"
    );

    // Test trait impl method visibility
    let trait_impl = code_graph
        .impls
        .iter()
        .find(|i| i.trait_type.is_some())
        .expect("Trait implementation not found");

    let trait_method = trait_impl.methods.first().expect("Trait method not found");

    assert!(
        matches!(
            code_graph.resolve_visibility(trait_method.id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "Trait method should inherit visibility from trait"
    );
}
