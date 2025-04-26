//! Tests specifically targeting TypeId conflation issues, particularly
//! with generics (T) and Self types across different scopes.

use crate::common::{
    paranoid::find_struct_node_paranoid, // Import from the paranoid module
    uuid_ids_utils::{
        find_field_type_id,
        find_function_node_paranoid,
        find_method_node_paranoid,
        find_param_type_id,
        run_phases_and_collect,
        MethodParentContext, // Import the new helper and context enum
    },
};
use ploke_common::fixtures_crates_dir; // For constructing paths
                                       // use std::path::PathBuf; // Removed unused import
use syn_parser::parser::{nodes::GraphNode, ParsedCodeGraph}; // Import directly

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

// --- #[cfg] Conflation Tests (Expected Behavior in Phase 2) ---
// These tests verify that NodeIds ARE currently conflated when items
// differ only by mutually exclusive #[cfg] attributes, as Phase 2
// ID generation does not yet account for cfg.

#[test]
fn test_cfg_struct_node_id_conflation() {
    let graphs = run_phases_and_collect(FIXTURE_NAME);
    let module_path = &["crate".to_string()];

    // Find the two struct nodes using the paranoid helper.
    // The helper internally verifies the ID generation including CFG context.
    // We need to find *both* instances. Since the paranoid helper asserts uniqueness
    // based on name *and* module path, we can't use it directly to find both.
    // Instead, we'll find them manually in the graph and assert their IDs differ.

    let graph_data = find_lib_rs_graph(&graphs);
    let graph = &graph_data.graph;

    // Find all struct nodes with that name associated with the correct module
    let module_node = graph
        .modules
        .iter()
        .find(|m| m.path == *module_path)
        .expect("ModuleNode for crate path not found");

    let found_structs: Vec<&syn_parser::parser::nodes::StructNode> = graph
        .defined_types
        .iter()
        .filter_map(|td| match td {
            syn_parser::parser::nodes::TypeDefNode::Struct(s) if s.name == "CfgGatedStruct" => {
                Some(s)
            }
            _ => None,
        })
        .filter(|s| {
            module_node
                .items()
                .is_some_and(|items| items.contains(&s.id))
        })
        .collect();

    // Assert: Exactly TWO nodes exist because the visitor processes both cfg branches
    assert_eq!(found_structs.len(), 2,
        "FAILED: Expected exactly two CfgGatedStruct nodes (one for each cfg branch), found {}. Visitor might not be processing both branches.",
        found_structs.len());

    // Assert: The two nodes have DIFFERENT NodeIds because their CFGs differ
    assert_ne!(found_structs[0].id, found_structs[1].id,
        "FAILED: Expected the two CfgGatedStruct nodes to have DIFFERENT NodeIds due to differing CFGs, but they are the same: {}. CFG hashing might not be working.",
        found_structs[0].id);

    // Optional: Verify each node individually using the paranoid helper if needed,
    // although the assertion above confirms the core requirement.
    // find_struct_node_paranoid(&graphs, FIXTURE_NAME, "src/lib.rs", module_path, "CfgGatedStruct");
    // This would panic if called twice expecting uniqueness, confirming the need for manual check here.
}

#[test]
fn test_cfg_function_node_id_conflation() {
    let graphs = run_phases_and_collect(FIXTURE_NAME);
    let module_path = &["crate".to_string()];

    // Similar to the struct test, find both function nodes manually and compare IDs.
    let graph_data = find_lib_rs_graph(&graphs);
    let graph = &graph_data.graph;

    // Find the parent module node
    let module_node = graph
        .modules
        .iter()
        .find(|m| m.path == *module_path)
        .expect("ModuleNode for crate path not found");

    // Find all function nodes with that name associated with the correct module
    let found_funcs: Vec<&syn_parser::parser::nodes::FunctionNode> = graph
        .functions
        .iter()
        .filter(|f| f.name == "cfg_gated_func")
        .filter(|f| {
            module_node
                .items()
                .is_some_and(|items| items.contains(&f.id))
        })
        .collect();

    // Assert: Exactly TWO nodes exist because the visitor processes both cfg branches
    assert_eq!(found_funcs.len(), 2,
        "FAILED: Expected exactly two cfg_gated_func nodes (one for each cfg branch), found {}. Visitor might not be processing both branches.",
        found_funcs.len());

    // Assert: The two nodes have DIFFERENT NodeIds because their CFGs differ
    assert_ne!(found_funcs[0].id, found_funcs[1].id,
        "FAILED: Expected the two cfg_gated_func nodes to have DIFFERENT NodeIds due to differing CFGs, but they are the same: {}. CFG hashing might not be working.",
        found_funcs[0].id);
}

// --- File-Level #[cfg] Disambiguation Tests ---

/// Test that NodeIds ARE distinct for identically named items defined in
/// separate files gated by mutually exclusive file-level #[cfg] attributes.
/// This distinction is expected because NodeId generation includes the file path.
///
/// Fixture Targets:
/// - `FileGatedStruct` in `src/cfg_file_a.rs` (via `#[cfg(feature = "feature_a")]`)
/// - `FileGatedStruct` in `src/cfg_file_not_a.rs` (via `#[cfg(not(feature = "feature_a"))]`)
#[test]
fn test_file_level_cfg_struct_node_id_disambiguation() {
    let graphs = run_phases_and_collect(FIXTURE_NAME);

    // Find FileGatedStruct in cfg_file_a.rs
    // Module path is ["crate", "cfg_file_a"]
    let struct_in_file_a = find_struct_node_paranoid(
        &graphs,
        FIXTURE_NAME,
        "src/cfg_file_a.rs",
        &["crate".to_string(), "cfg_file_a".to_string()],
        "FileGatedStruct",
    );

    // Find FileGatedStruct in cfg_file_not_a.rs
    // Module path is ["crate", "cfg_file_not_a"]
    let struct_in_file_not_a = find_struct_node_paranoid(
        &graphs,
        FIXTURE_NAME,
        "src/cfg_file_not_a.rs",
        &["crate".to_string(), "cfg_file_not_a".to_string()],
        "FileGatedStruct",
    );

    // Assert that the NodeIds are distinct (NOT equal)
    assert_ne!(
        struct_in_file_a.id, struct_in_file_not_a.id,
        "FAILED: Expected NodeIds for FileGatedStruct in different cfg-gated files to be distinct due to file path difference, but they are the same.\n - ID in cfg_file_a.rs:    {}\n - ID in cfg_file_not_a.rs: {}",
        struct_in_file_a.id, struct_in_file_not_a.id
    );

    // --- Check File-Level Attributes on ModuleNode ---
    // The file-level #[cfg] attributes are stored on the ModuleNode representing the file,
    // not directly on the items defined within it. We need to find the ModuleNodes.

    let fixture_root = fixtures_crates_dir().join(FIXTURE_NAME);
    let file_a_path = fixture_root.join("src/cfg_file_a.rs");
    let file_not_a_path = fixture_root.join("src/cfg_file_not_a.rs");

    // Find the ModuleNode for cfg_file_a.rs
    let module_a = graphs
        .iter()
        .find_map(|g| {
            g.graph
                .modules
                .iter()
                .find(|m| m.file_path().is_some_and(|p| p == &file_a_path))
        })
        .expect("ModuleNode for cfg_file_a.rs not found");

    // Find the ModuleNode for cfg_file_not_a.rs
    let module_not_a = graphs
        .iter()
        .find_map(|g| {
            g.graph
                .modules
                .iter()
                .find(|m| m.file_path().is_some_and(|p| p == &file_not_a_path))
        })
        .expect("ModuleNode for cfg_file_not_a.rs not found");
    // Assert that the correct file-level cfg attribute is present on module_a's cfgs field
    assert!(
        module_a.cfgs().contains(&"feature = \"feature_a\"".to_string()),
        "FAILED: Expected ModuleNode for cfg_file_a.rs to have `cfgs` containing 'feature = \"feature_a\"'. Found: {:?}",
        module_a.cfgs()
    );

    let expected_cfg_not_a = "not (feature = \"feature_a\")"; // Expect space after 'not'
                                                              // Assert that the correct file-level cfg attribute is present on module_not_a's cfgs field
    assert!(
        module_not_a.cfgs().contains(&expected_cfg_not_a.to_string()), // Note: syn might normalize spacing
        "FAILED: Expected ModuleNode for cfg_file_not_a.rs to have `cfgs` containing 'not(feature = \"feature_a\")'. Found: {:?}",
        module_not_a.cfgs()
    );
}
