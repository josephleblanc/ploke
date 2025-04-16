//! Tests specifically targeting TypeId conflation issues, particularly
//! with generics (T) and Self types across different scopes.

use crate::common::uuid_ids_utils::{
    find_function_node_paranoid, find_param_type_id, find_return_type_id, run_phases_and_collect,
};
use ploke_core::NodeId; // Assuming NodeId is needed, adjust if not

const FIXTURE_NAME: &str = "fixture_conflation";

/// Test that the TypeId for a generic parameter 'T' is currently the same
/// when defined in different function scopes within the same file.
/// This test is EXPECTED TO PASS with the current implementation and
/// SHOULD FAIL after TypeId generation incorporates parent scope.
#[test]
fn test_generic_param_conflation_in_functions() {
    let graphs = run_phases_and_collect(FIXTURE_NAME);
    // Assuming fixture_conflation/src/lib.rs is the only file parsed for this fixture
    assert_eq!(graphs.len(), 1, "Expected only one graph for this fixture");
    let graph_data = &graphs[0];
    let graph = &graph_data.graph;

    // Find the top-level function `top_level_func<T>(param: T)`
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

    // Assert that the TypeIds are currently the same (conflated)
    assert_eq!(
        top_level_param_t_type_id, inner_param_t_type_id,
        "TypeId for generic 'T' parameter should currently be conflated between top_level_func and inner_func. This test should fail after fix."
    );
}

/// Test that the TypeId for the 'Self' return type is currently the same
/// when used in methods of different impl blocks within the same file.
/// This test is EXPECTED TO PASS with the current implementation and
/// SHOULD FAIL after TypeId generation incorporates parent scope.
#[test]
fn test_self_return_type_conflation_in_impls() {
    let graphs = run_phases_and_collect(FIXTURE_NAME);
    assert_eq!(graphs.len(), 1, "Expected only one graph for this fixture");
    let graph_data = &graphs[0];
    let graph = &graph_data.graph;

    // Find the method `method` in `impl TopLevelStruct`
    // Note: The parent scope for a method is the ImplNode ID. We need to find that first,
    // or adjust find_function_node_paranoid if it can handle methods directly.
    // Let's assume find_function_node_paranoid works by finding the function and checking
    // its parent 'Contains' relation points to the correct Impl block scope.
    // We need the module path containing the *impl block*.
    let top_level_method_node = find_function_node_paranoid(
        &graphs,
        FIXTURE_NAME,
        "src/lib.rs",
        &["crate".to_string()], // Impl block is at the top level
        "method",               // Method name
    );

    // Find the method `inner_method` in `impl InnerStruct`
    let inner_method_node = find_function_node_paranoid(
        &graphs,
        FIXTURE_NAME,
        "src/lib.rs",
        &["crate".to_string(), "inner_mod".to_string()], // Impl block is in inner_mod
        "inner_method",                                 // Method name
    );

    // Get the TypeId for the return type (Self) in both methods
    let top_level_return_self_type_id = find_return_type_id(graph, top_level_method_node.id)
        .expect("Failed to find return TypeId for Self in TopLevelStruct::method");
    let inner_return_self_type_id = find_return_type_id(graph, inner_method_node.id)
        .expect("Failed to find return TypeId for Self in InnerStruct::inner_method");

    // Assert that the TypeIds are currently the same (conflated)
    assert_eq!(
        top_level_return_self_type_id, inner_return_self_type_id,
        "TypeId for 'Self' return type should currently be conflated between impls for TopLevelStruct and InnerStruct. This test should fail after fix."
    );
}
