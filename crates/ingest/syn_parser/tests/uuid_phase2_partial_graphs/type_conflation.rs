//! Tests specifically targeting TypeId conflation issues, particularly
//! with generics (T) and Self types across different scopes.

use crate::common::{
    uuid_ids_utils::{
        find_field_type_id, find_function_node_paranoid, find_param_type_id,
        find_return_type_id, find_struct_node_paranoid, run_phases_and_collect,
    },
    FixtureError, // Import FixtureError if needed for helper results
};
use ploke_common::fixtures_crates_dir; // For constructing paths
use std::path::PathBuf;

const FIXTURE_NAME: &str = "fixture_conflation";

// Helper to find the specific graph for lib.rs
fn find_lib_rs_graph(
    graphs: &[crate::common::uuid_ids_utils::ParsedCodeGraph],
) -> &crate::common::uuid_ids_utils::ParsedCodeGraph {
    let fixture_root = fixtures_crates_dir().join(FIXTURE_NAME);
    let lib_rs_path = fixture_root.join("src/lib.rs");
    graphs
        .iter()
        .find(|g| g.file_path == lib_rs_path)
        .expect("ParsedCodeGraph for src/lib.rs not found")
}

/// Test that the TypeId for a generic parameter 'T' is currently the same
/// when defined in different function scopes within the same file.
/// This test is EXPECTED TO PASS with the current implementation and
/// SHOULD FAIL after TypeId generation incorporates parent scope.
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

    // Assert that the TypeIds are currently the same (conflated)
    assert_eq!(
        top_level_param_t_type_id, inner_param_t_type_id,
        "TypeId for generic 'T' parameter should currently be conflated between top_level_func ({}) and inner_func ({}). This test should fail after fix.",
        top_level_param_t_type_id,
        inner_param_t_type_id
    );
}

/// Test that the TypeId for the 'Self' return type is currently the same
/// when used in methods of different impl blocks within the same file.
/// This test is EXPECTED TO PASS with the current implementation and
/// SHOULD FAIL after TypeId generation incorporates parent scope.
#[test]
fn test_self_return_type_conflation_in_impls() {
    let graphs = run_phases_and_collect(FIXTURE_NAME);
    let graph_data = find_lib_rs_graph(&graphs); // Find the specific graph for lib.rs
    let graph = &graph_data.graph;

    // Find the method `method` in `impl TopLevelStruct` in lib.rs
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
        "TypeId for 'Self' return type should currently be conflated between impls for TopLevelStruct ({}) and InnerStruct ({}). This test should fail after fix.",
        top_level_return_self_type_id,
        inner_return_self_type_id
    );
}

/// Test that the TypeId for a generic parameter 'T' used as a field type
/// is currently the same when defined in different struct/newtype scopes
/// within the same file.
/// This test is EXPECTED TO PASS with the current implementation and
/// SHOULD FAIL after TypeId generation incorporates parent scope.
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


    // Assert that the TypeIds for the field 'T' are currently the same (conflated)
    assert_eq!(
        top_level_field_t_type_id, inner_field_t_type_id,
        "TypeId for generic 'T' field should currently be conflated between TopLevelNewtype ({}) and InnerNewtype ({}). This test should fail after fix.",
        top_level_field_t_type_id,
        inner_field_t_type_id
    );
}
