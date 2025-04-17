//! Tests specifically targeting TypeId conflation issues, particularly
//! with generics (T) and Self types across different scopes.

use crate::common::{
    paranoid::find_struct_node_paranoid, // Import from the paranoid module
    uuid_ids_utils::{
        find_field_type_id,
        find_function_node_paranoid,
        find_method_node_paranoid,
        find_param_type_id,
        find_return_type_id,
        run_phases_and_collect,
        MethodParentContext, // Import the new helper and context enum
    },
};
use ploke_common::fixtures_crates_dir; // For constructing paths
                                       // use std::path::PathBuf; // Removed unused import
use syn_parser::parser::visitor::ParsedCodeGraph; // Import directly

const FIXTURE_NAME: &str = "fixture_conflation";

// Helper to find the specific graph for lib.rs
fn find_lib_rs_graph(graphs: &[ParsedCodeGraph]) -> &ParsedCodeGraph {
    // Use direct import
    let fixture_root = fixtures_crates_dir().join(FIXTURE_NAME);
    let lib_rs_path = fixture_root.join("src/lib.rs");
    graphs
        .iter()
        .find(|g| g.file_path == lib_rs_path)
        .expect("ParsedCodeGraph for src/lib.rs not found")
}

/// Test that the TypeId for a generic parameter 'T' is distinct
/// when defined in different function scopes within the same file.
///
/// Fixture Targets:
/// - `top_level_func<T>(param: T)` in `src/lib.rs`
/// - `inner_mod::inner_func<T>(param: T)` in `src/lib.rs`
///
/// Expected Behavior (Post-Fix):
/// The `TypeId` for `T` in `top_level_func` should be different from the
/// `TypeId` for `T` in `inner_func` because they are defined within
/// different parent scopes (the functions themselves).
///
/// Current Behavior (Pre-Fix):
/// The `TypeId`s are expected to be the *same* because `TypeId::generate_synthetic`
/// currently ignores the parent scope. This test SHOULD FAIL until the fix is applied.
#[test]
fn test_generic_param_conflation_in_functions() {
    let graphs = run_phases_and_collect(FIXTURE_NAME);
    let graph_data = find_lib_rs_graph(&graphs); // Find the specific graph for lib.rs
    let graph = &graph_data.graph;

    // Find the top-level function `top_level_func<T>(param: T)` in lib.rs
    let top_level_func_node = find_function_node_paranoid(
        &graphs,
        FIXTURE_NAME,
        "src/lib.rs",
        &["crate".to_string()], // Top-level module path
        "top_level_func",
    );

    // Find the inner function `inner_func<T>(param: T)`
    let inner_func_node = find_function_node_paranoid(
        &graphs,
        FIXTURE_NAME,
        "src/lib.rs",
        &["crate".to_string(), "inner_mod".to_string()], // Inner module path
        "inner_func",
    );

    // Get the TypeId for the first parameter (T) in both functions
    let top_level_param_t_type_id = find_param_type_id(graph, top_level_func_node.id, 0)
        .expect("Failed to find TypeId for T in top_level_func parameter");
    let inner_param_t_type_id = find_param_type_id(graph, inner_func_node.id, 0)
        .expect("Failed to find TypeId for T in inner_func parameter");

    // Assert that the TypeIds are distinct (NOT equal).
    // This WILL FAIL until TypeId::generate_synthetic incorporates parent_scope_id.
    assert_ne!(
        top_level_param_t_type_id, inner_param_t_type_id,
        "FAILED: TypeId for generic 'T' parameter is conflated between different function scopes.\n - Top Level Func 'T': {}\n - Inner Func 'T':    {}\nThis indicates TypeId generation needs parent scope context.",
        top_level_param_t_type_id,
        inner_param_t_type_id
    );
}

/// Test that the TypeId for the 'Self' return type is distinct
/// when used in methods of different impl blocks within the same file.
///
/// Fixture Targets:
/// - `impl TopLevelStruct { fn method(&self, ...) -> Self }` in `src/lib.rs`
/// - `impl inner_mod::InnerStruct { fn inner_method(&self, ...) -> Self }` in `src/lib.rs`
///
/// Expected Behavior (Post-Fix):
/// The `TypeId` for the `Self` return type in `TopLevelStruct::method` should
/// be different from the `TypeId` for `Self` in `InnerStruct::inner_method`
/// because `Self` refers to different types (`TopLevelStruct` vs `InnerStruct`)
/// defined in different scopes. The parent scope (the impl block) should differentiate them.
///
/// Current Behavior (Pre-Fix):
/// The `TypeId`s are expected to be the *same* because `TypeId::generate_synthetic`
/// currently ignores the parent scope and only sees `TypeKind::Named { path: ["Self"], .. }`.
/// This test SHOULD FAIL until the fix is applied.
#[test]
fn test_self_return_type_conflation_in_impls() {
    let graphs = run_phases_and_collect(FIXTURE_NAME);
    let graph_data = find_lib_rs_graph(&graphs); // Find the specific graph for lib.rs
    let graph = &graph_data.graph;

    // Find the method `method` in `impl TopLevelStruct` in lib.rs
    // Note: The parent scope for a method is the ImplNode ID. We need to find that first,
    // or adjust find_function_node_paranoid if it can handle methods directly.
    // Let's assume find_function_node_paranoid works by finding the function and checking
    // its parent 'Contains' relation points to the correct Impl block scope. <-- This assumption was wrong for find_function_node_paranoid
    // We need the module path containing the *impl block*.
    // Use the new find_method_node_paranoid helper
    let top_level_method_node = find_method_node_paranoid(
        &graphs,
        FIXTURE_NAME,
        "src/lib.rs",
        &["crate".to_string()], // Module path containing the impl block
        MethodParentContext::Impl {
            self_type_str: "TopLevelStruct < T >", // The struct being implemented
            trait_type_str: None,                  // It's an inherent impl
        },
        "method", // Method name
    );

    // Find the method `inner_method` in `impl InnerStruct` using the new helper
    let inner_method_node = find_method_node_paranoid(
        &graphs,
        FIXTURE_NAME,
        "src/lib.rs",
        &["crate".to_string(), "inner_mod".to_string()], // Module path containing the impl block
        MethodParentContext::Impl {
            self_type_str: "InnerStruct < T >", // The struct being implemented
            trait_type_str: None,               // It's an inherent impl
        },
        "inner_method", // Method name
    );

    // Get the TypeId for the return type (Self) in both methods
    let top_level_return_self_type = graph
        .impls
        .iter()
        .flat_map(|imp| &imp.methods)
        .find(|f| f.id == top_level_method_node.id)
        .expect("Failed to find return TypeId for Self in TopLevelStruct::method");
    let inner_return_self_type = graph
        .impls
        .iter()
        .flat_map(|imp| &imp.methods)
        .find(|f| f.id == inner_method_node.id)
        .expect("Failed to find return TypeId for Self in InnerStruct::inner_method");

    // Assert that the TypeIds are distinct (NOT equal).
    // This WILL FAIL until TypeId::generate_synthetic incorporates parent_scope_id.
    assert_ne!(
        top_level_return_self_type.id,
        inner_return_self_type.id,
        "FAILED: TypeId for 'Self' return type is conflated between different impl blocks.
- TopLevelStruct impl 'Self': {}
- InnerStruct impl 'Self':    {}
This indicates TypeId generation needs parent scope context.
Full node info:
top_level_return_self_type: {:#?}
inner_return_self_type: {:#?}",
        top_level_return_self_type.id,
        inner_return_self_type.id,
        top_level_return_self_type,
        inner_return_self_type
    );
}

/// Test that the TypeId for a generic parameter 'T' used as a field type
/// is distinct when defined in different struct/newtype scopes within the same file.
///
/// Fixture Targets:
/// - `TopLevelNewtype<T>(pub T)` in `src/lib.rs` (field `0` of type `T`)
/// - `inner_mod::InnerNewtype<T>(pub T)` in `src/lib.rs` (field `0` of type `T`)
///
/// Expected Behavior (Post-Fix):
/// The `TypeId` for the field type `T` in `TopLevelNewtype` should be different
/// from the `TypeId` for the field type `T` in `InnerNewtype` because the `T`
/// generic parameter is defined within different parent scopes (the structs themselves).
///
/// Current Behavior (Pre-Fix):
/// The `TypeId`s are expected to be the *same* because `TypeId::generate_synthetic`
/// currently ignores the parent scope and only sees `TypeKind::Named { path: ["T"], .. }`.
/// This test SHOULD FAIL until the fix is applied.
#[test]
fn test_generic_field_conflation_in_structs() {
    let graphs = run_phases_and_collect(FIXTURE_NAME);
    let graph_data = find_lib_rs_graph(&graphs); // Find the specific graph for lib.rs
    let graph = &graph_data.graph;

    // Find TopLevelNewtype<T>(pub T)
    let top_level_newtype_node = find_struct_node_paranoid(
        &graphs,
        FIXTURE_NAME,
        "src/lib.rs",
        &["crate".to_string()],
        "TopLevelNewtype",
    );
    // The field in a tuple struct doesn't have a name, find by index (0) or ID if possible.
    // Assuming the FieldNode ID can be found via relation or index.
    // Let's assume the first field is the one we want.
    let top_level_field_node_id = top_level_newtype_node.fields[0].id;
    let top_level_field_t_type_id = find_field_type_id(graph, top_level_field_node_id)
        .expect("Failed to find TypeId for T field in TopLevelNewtype");

    // Find inner_mod::InnerNewtype<T>(pub T)
    let inner_newtype_node = find_struct_node_paranoid(
        &graphs,
        FIXTURE_NAME,
        "src/lib.rs",
        &["crate".to_string(), "inner_mod".to_string()],
        "InnerNewtype",
    );
    let inner_field_node_id = inner_newtype_node.fields[0].id;
    let inner_field_t_type_id = find_field_type_id(graph, inner_field_node_id)
        .expect("Failed to find TypeId for T field in InnerNewtype");

    // Assert that the TypeIds for the field 'T' are distinct (NOT equal).
    // This WILL FAIL until TypeId::generate_synthetic incorporates parent_scope_id.
    assert_ne!(
        top_level_field_t_type_id, inner_field_t_type_id,
        "FAILED: TypeId for generic 'T' field type is conflated between different struct scopes.\n - TopLevelNewtype 'T' field: {}\n - InnerNewtype 'T' field:    {}\nThis indicates TypeId generation needs parent scope context.",
        top_level_field_t_type_id,
        inner_field_t_type_id
    );
}
