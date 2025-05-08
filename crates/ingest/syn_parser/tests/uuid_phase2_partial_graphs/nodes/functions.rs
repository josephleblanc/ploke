#![cfg(test)]

use crate::common::run_phases_and_collect;
use crate::common::ParanoidArgs;
use crate::paranoid_test_fields_and_values; // For EXPECTED_FUNCTIONS_ARGS
use lazy_static::lazy_static;
use ploke_core::{ItemKind, TypeId, TypeKind};
use std::collections::HashMap;
use syn_parser::error::SynParserError; // Import ItemKind and TypeKind from ploke_core
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::PrimaryNodeIdTrait;
use syn_parser::parser::nodes::{Attribute, ExpectedFunctionNode, GraphNode}; // For ExpectedFunctionNode and Attribute
use syn_parser::parser::types::VisibilityKind; // Import VisibilityKind from its correct location
                                               // Remove TypeKind from here, already imported from ploke_core

pub const LOG_TEST_FUNCTION: &str = "log_test_function";

lazy_static! {
    static ref EXPECTED_FUNCTIONS_DATA: HashMap<&'static str, ExpectedFunctionNode> = {
        let mut m = HashMap::new();

        m.insert("crate::process_tuple", ExpectedFunctionNode {
            name: "process_tuple",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        });
        m.insert("crate::process_slice", ExpectedFunctionNode {
            name: "process_slice",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        });
        m.insert("crate::process_array", ExpectedFunctionNode {
            name: "process_array",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        });
        m.insert("crate::process_ref", ExpectedFunctionNode {
            name: "process_ref",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        });
        m.insert("crate::process_mut_ref", ExpectedFunctionNode {
            name: "process_mut_ref",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false, // Returns implicit unit
            body_is_some: true,
        });
        m.insert("crate::apply_op", ExpectedFunctionNode {
            name: "apply_op",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 3,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        });
        m.insert("crate::process_const_ptr", ExpectedFunctionNode {
            name: "process_const_ptr",
            visibility: VisibilityKind::Inherited, // private
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        });
        m.insert("crate::process_mut_ptr", ExpectedFunctionNode {
            name: "process_mut_ptr",
            visibility: VisibilityKind::Inherited, // private
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false, // Returns implicit unit
            body_is_some: true,
        });
        m.insert("crate::draw_object", ExpectedFunctionNode {
            name: "draw_object",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false, // Returns implicit unit
            body_is_some: true,
        });
        m.insert("crate::process_impl_trait_arg", ExpectedFunctionNode {
            name: "process_impl_trait_arg",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: false, // Returns implicit unit
            body_is_some: true,
        });
        m.insert("crate::create_impl_trait_return", ExpectedFunctionNode {
            name: "create_impl_trait_return",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 0,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        });
        m.insert("crate::inferred_type_example", ExpectedFunctionNode {
            name: "inferred_type_example",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 0,
            generic_param_count: 0,
            return_type_is_some: false, // Returns implicit unit
            body_is_some: true,
        });
        // consumes_point in src/func/return_types.rs
        m.insert("crate::func::return_types::consumes_point", ExpectedFunctionNode { // Renamed key to be unique
            name: "consumes_point",
            visibility: VisibilityKind::Restricted(vec!["crate".to_string()]), // pub(crate)
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true, // returns bool
            body_is_some: true,
        });
        // generic_func in src/func/return_types.rs
        m.insert("crate::func::return_types::generic_func", ExpectedFunctionNode { // Renamed key
            name: "generic_func",
            visibility: VisibilityKind::Restricted(vec!["crate".to_string()]), // pub(crate)
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 2,
            generic_param_count: 2,
            return_type_is_some: true, // returns T
            body_is_some: true,
        });
        // math_operation_consumer in src/func/return_types.rs
        m.insert("crate::func::return_types::math_operation_consumer", ExpectedFunctionNode {
            name: "math_operation_consumer",
            visibility: VisibilityKind::Inherited, // private
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 3,
            generic_param_count: 0,
            return_type_is_some: true, // returns i32
            body_is_some: true,
        });
        // math_operation_producer in src/func/return_types.rs
        m.insert("crate::func::return_types::math_operation_producer", ExpectedFunctionNode {
            name: "math_operation_producer",
            visibility: VisibilityKind::Inherited, // private
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 0,
            generic_param_count: 0,
            return_type_is_some: true, // returns MathOperation
            body_is_some: true,
        });
        // consumes_point in src/func/return_types.rs/restricted_duplicate
        // Note: The existing test `test_function_node_consumes_point_in_restricted_duplicate`
        // actually tests the one in `src/func/return_types.rs` due to its module_path.
        // I will create a distinct entry for the one truly in `restricted_duplicate`.
        m.insert("crate::func::return_types::restricted_duplicate::consumes_point", ExpectedFunctionNode {
            name: "consumes_point",
            visibility: VisibilityKind::Restricted(vec!["crate".to_string()]), // pub(crate)
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true, // returns bool
            body_is_some: true,
        });
        // generic_func in src/func/return_types.rs/restricted_duplicate
        m.insert("crate::func::return_types::restricted_duplicate::generic_func", ExpectedFunctionNode {
            name: "generic_func",
            visibility: VisibilityKind::Restricted(vec!["crate".to_string()]), // pub(crate)
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 2,
            generic_param_count: 2,
            return_type_is_some: true, // returns T
            body_is_some: true,
        });
        // process_tuple in src/lib.rs/duplicate_names
        m.insert("crate::duplicate_names::process_tuple", ExpectedFunctionNode { // Renamed key
            name: "process_tuple",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        });
        // process_slice in src/lib.rs/duplicate_names
        m.insert("crate::duplicate_names::process_slice", ExpectedFunctionNode { // Renamed key
            name: "process_slice",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        });
        // process_array in src/lib.rs/duplicate_names
        m.insert("crate::duplicate_names::process_array", ExpectedFunctionNode { // Renamed key
            name: "process_array",
            visibility: VisibilityKind::Public,
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_param_count: 0,
            return_type_is_some: true,
            body_is_some: true,
        });
        m
    };

    static ref EXPECTED_FUNCTIONS_ARGS: HashMap<&'static str, ParanoidArgs<'static>> = {
        let mut m = HashMap::new();
        m.insert("crate::process_tuple", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_tuple",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::process_slice", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_slice",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::process_array", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_array",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::process_ref", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_ref",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::process_mut_ref", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_mut_ref",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::apply_op", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "apply_op",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::process_const_ptr", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_const_ptr",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::process_mut_ptr", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_mut_ptr",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::draw_object", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "draw_object",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::process_impl_trait_arg", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_impl_trait_arg",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::create_impl_trait_return", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "create_impl_trait_return",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::inferred_type_example", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "inferred_type_example",
            expected_cfg: None,
            expected_path: &["crate"],
            item_kind: ItemKind::Function,
        });
        // Functions in src/func/return_types.rs
        m.insert("crate::func::return_types::consumes_point", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/func/return_types.rs",
            ident: "consumes_point", // Corresponds to DATA key
            expected_cfg: None,
            expected_path: &["crate", "func", "return_types"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::func::return_types::generic_func", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/func/return_types.rs",
            ident: "generic_func", // Corresponds to DATA key
            expected_cfg: None,
            expected_path: &["crate", "func", "return_types"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::func::return_types::math_operation_consumer", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/func/return_types.rs",
            ident: "math_operation_consumer",
            expected_cfg: None,
            expected_path: &["crate", "func", "return_types"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::func::return_types::math_operation_producer", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/func/return_types.rs",
            ident: "math_operation_producer",
            expected_cfg: None,
            expected_path: &["crate", "func", "return_types"],
            item_kind: ItemKind::Function,
        });
        // Functions in src/func/return_types.rs/restricted_duplicate
        m.insert("crate::func::return_types::restricted_duplicate::consumes_point", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/func/return_types.rs", // File is still return_types.rs
            ident: "consumes_point", // Corresponds to DATA key
            expected_cfg: None,
            expected_path: &["crate", "func", "return_types", "restricted_duplicate"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::func::return_types::restricted_duplicate::generic_func", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/func/return_types.rs", // File is still return_types.rs
            ident: "generic_func", // Corresponds to DATA key
            expected_cfg: None,
            expected_path: &["crate", "func", "return_types", "restricted_duplicate"],
            item_kind: ItemKind::Function,
        });
        // Functions in src/lib.rs/duplicate_names
        m.insert("crate::duplicate_names::process_tuple", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_tuple", // Corresponds to DATA key
            expected_cfg: None,
            expected_path: &["crate", "duplicate_names"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::duplicate_names::process_slice", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_slice", // Corresponds to DATA key
            expected_cfg: None,
            expected_path: &["crate", "duplicate_names"],
            item_kind: ItemKind::Function,
        });
        m.insert("crate::duplicate_names::process_array", ParanoidArgs {
            fixture: "fixture_types",
            relative_file_path: "src/lib.rs",
            ident: "process_array", // Corresponds to DATA key
            expected_cfg: None,
            expected_path: &["crate", "duplicate_names"],
            item_kind: ItemKind::Function,
        });
        m
    };
}
// -- new test for functions
// basic sanity check that the macro logic is working correctly.
// DO NOT REMOVE
#[test]
fn test_function_node_standard() -> Result<(), SynParserError> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .try_init();
    // NOTE: Our current macro doesn't do a very good job of distinguishing between two identical
    // Original was Result<()> which is FixtureError
    // Collect successful graphs
    let successful_graphs = run_phases_and_collect("fixture_types");

    // Use ParanoidArgs to find the node
    let args_key = "crate::duplicate_names::process_tuple";
    let args = EXPECTED_FUNCTIONS_ARGS.get(args_key).unwrap_or_else(|| {
        panic!("ParanoidArgs not found for key: {}", args_key);
    });
    let exp_func = EXPECTED_FUNCTIONS_DATA.get(args.ident).unwrap();

    // Generate the expected PrimaryNodeId using the method on ParanoidArgs
    let test_info = args.generate_pid(&successful_graphs).inspect_err(|e| {
        log::warn!(target: LOG_TEST_FUNCTION, "PID generation failed for '{}' (Error: {:?}). Running direct value checks:", args.ident, e);
        let target_graph = successful_graphs
            .iter()
            .find(|pg| pg.file_path.ends_with(args.relative_file_path))
            .unwrap_or_else(|| panic!("Target graph '{}' not found for value checks after PID generation failure for '{}'.", args.relative_file_path, args.ident));

        let _found = exp_func.find_node_by_values(target_graph).count();
        let _ = args.check_graph(target_graph);
    })?;

    // Find the node using the generated ID within the correct graph
    let node = test_info
        .target_data() // This is &ParsedCodeGraph
        .find_node_unique(test_info.test_pid().into()) // Uses the generated PID
        .inspect_err(|e| {
            let target_graph = test_info.target_data();
            let _ = args.check_graph(target_graph);
            let count = exp_func.find_node_by_values(target_graph).count();
            log::warn!(target: LOG_TEST_FUNCTION, "Node lookup by PID '{}' failed for '{}', found {} matching values with find_node_by_values (Error: {:?}). Running direct value checks:", test_info.test_pid(), args.ident, count, e);
        })?;

    assert_eq!(
        node.name(), // Use the GraphNode trait method
        args.ident,
        "Mismatch for name field. Expected: '{}', Actual: '{}'",
        args.ident,
        node.name()
    );

    let node = node.as_function().unwrap();
    assert!({
        ![
            exp_func.is_name_match_debug(node),
            exp_func.is_visibility_match_debug(node),
            exp_func.is_attributes_match_debug(node),
            exp_func.is_body_match_debug(node),
            exp_func.is_docstring_match_debug(node),
            exp_func.is_tracking_hash_match_debug(node),
            exp_func.is_cfgs_match_debug(node),
        ]
        .contains(&false)
    });
    let expected_func_node = EXPECTED_FUNCTIONS_DATA
        .get("process_tuple")
        .expect("The specified node was not found in they map of expected function nodes.");

    let mut node_matches_iter = expected_func_node
        .find_node_by_values(test_info.target_data())
        .filter(|func| func.id.to_pid() == node.id.to_pid());
    let macro_found_node = node_matches_iter.next().unwrap();
    println!(
        "FucntionNode found using new macro: {:#?}",
        macro_found_node
    );
    println!("FunctionNode found using old methods: {:#?}", node);
    assert!(macro_found_node.id.to_pid() == node.id.to_pid());
    assert!(node_matches_iter.next().is_none());
    // assert!(expected_const_node.check_all_fields(node));
    Ok(())
}

paranoid_test_fields_and_values!(
    test_function_node_new_macro,
    "crate::process_tuple",                          // args_key
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);

// --- Test Cases ---

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_tuple() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let func_name = "process_tuple";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results // Need graph for type lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.docstring.is_none());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.generic_params.is_empty());
    // assert!(func_node.body_str.is_some()); // FunctionNode doesn't expose body_str

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("p")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (Point -> (i32, i32))
    let param_type_node = find_type_node(graph, param.type_id);
    // Current state: Falls back to Unknown because TypeKind::Path processing doesn't fully resolve aliases yet
    assert!(
        matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Point"]),
        "Expected TypeKind::Named for alias 'Point', found {:?}",
        param_type_node.kind
    );
    // TODO: Once alias resolution is deeper, this should check the underlying tuple type.

    // Return Type
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));

    // Check return TypeNode (i32)
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(
        matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]),
        "Expected TypeKind::Named for 'i32', found {:?}",
        return_type_node.kind
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_slice() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_slice";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.docstring.is_none());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.generic_params.is_empty());
    // assert!(func_node.body_str.is_some()); // FunctionNode doesn't expose body_str

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("s")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (&[u8])
    let param_type_node = find_type_node(graph, param.type_id);
    // Current state: Correctly identifies the reference part.
    assert!(
        matches!(&param_type_node.kind, TypeKind::Reference { is_mutable, .. } if !*is_mutable),
        "Expected TypeKind::Reference for '&[u8]', found {:?}",
        param_type_node.kind
    );
    // Check the referenced type ([u8])
    assert_eq!(
        param_type_node.related_types.len(),
        1,
        "Reference should have one related type ([u8])"
    );
    let slice_type_id = param_type_node.related_types[0];
    let slice_type_node = find_type_node(graph, slice_type_id);
    // The underlying slice type [u8] currently falls back to Unknown because TypeKind::Slice is not implemented
    assert!(
        matches!(&slice_type_node.kind, TypeKind::Unknown { type_str } if type_str == "[u8]"),
        "Expected underlying type '[u8]' to be TypeKind::Unknown currently, found {:?}",
        slice_type_node.kind
    );

    // #[ignore = "TypeKind::Slice not yet handled in type_processing.rs"]
    {
        // Target state assertion for the underlying slice type (will fail until implemented)
        // assert!(matches!(param_type_node.kind, TypeKind::Slice { .. }));
        // assert_eq!(param_type_node.related_types.len(), 1); // Should relate to u8
    }

    // Return Type
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));

    // Check return TypeNode (usize)
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(
        matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["usize"]),
        "Expected TypeKind::Named for 'usize', found {:?}",
        return_type_node.kind
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_array() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_array";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("a")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (Buffer -> [u8; 1024])
    let param_type_node = find_type_node(graph, param.type_id);
    assert!(
        matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Buffer"]),
        "Expected TypeKind::Named for alias 'Buffer', found {:?}",
        param_type_node.kind
    );
    // TODO: Deeper check for underlying array type once alias resolution is better

    // Return Type
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));

    // Check return TypeNode (u8)
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(
        matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["u8"]),
        "Expected TypeKind::Named for 'u8', found {:?}",
        return_type_node.kind
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_ref() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_ref";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("r")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (&String)
    let param_type_node = find_type_node(graph, param.type_id);
    assert!(
        matches!(&param_type_node.kind, TypeKind::Reference { is_mutable, .. } if !*is_mutable),
        "Expected TypeKind::Reference (immutable), found {:?}",
        param_type_node.kind
    );
    assert_eq!(
        param_type_node.related_types.len(),
        1,
        "Reference should have one related type (String)"
    );
    let referenced_type_id = param_type_node.related_types[0];
    let referenced_type_node = find_type_node(graph, referenced_type_id);
    assert!(
        matches!(&referenced_type_node.kind, TypeKind::Named { path, .. } if path == &["String"]),
        "Expected referenced type to be 'String', found {:?}",
        referenced_type_node.kind
    );

    // Return Type (usize)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(
        matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["usize"]),
        "Expected TypeKind::Named for 'usize', found {:?}",
        return_type_node.kind
    );
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_mut_ref() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_mut_ref";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("r")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (&mut String)
    let param_type_node = find_type_node(graph, param.type_id);
    assert!(
        matches!(&param_type_node.kind, TypeKind::Reference { is_mutable, .. } if *is_mutable),
        "Expected TypeKind::Reference (mutable), found {:?}",
        param_type_node.kind
    );
    assert_eq!(
        param_type_node.related_types.len(),
        1,
        "Reference should have one related type (String)"
    );
    let referenced_type_id = param_type_node.related_types[0];
    let referenced_type_node = find_type_node(graph, referenced_type_id);
    assert!(
        matches!(&referenced_type_node.kind, TypeKind::Named { path, .. } if path == &["String"]),
        "Expected referenced type to be 'String', found {:?}",
        referenced_type_node.kind
    );

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none()); // Implicit unit
}

// --- Tests for functions inside `duplicate_names` module ---

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_tuple_in_duplicate_names() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_tuple";
    let relative_file_path = "src/lib.rs"; // Function is defined in lib.rs
                                           // Module path *within lib.rs* where the function is defined
    let module_path = vec!["crate".to_string(), "duplicate_names".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Basic Assertions (should be similar to the top-level one)
    let graph = &results // Need graph for type lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public); // Check visibility within module

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("p")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));
    let param_type_node = find_type_node(graph, param.type_id);
    // Check type kind - should reference the 'Point' defined *within* duplicate_names
    // This requires checking the TypeNode's path or related types carefully.
    // For now, assert it's Named "Point". Phase 3 resolves which "Point" it is.
    assert!(
        matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Point"]),
        "Expected TypeKind::Named for alias 'Point' (in duplicate_names), found {:?}",
        param_type_node.kind
    );

    // Return Type
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(
        matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]),
        "Expected TypeKind::Named for 'i32', found {:?}",
        return_type_node.kind
    );

    // Add more assertions for other functions in duplicate_names...
    // test_function_node_process_slice_in_duplicate_names
    // test_function_node_process_array_in_duplicate_names
    // etc.
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_apply_op() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "apply_op";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);

    // Parameters (a: i32, b: i32, op: MathOperation)
    assert_eq!(func_node.parameters.len(), 3);
    let param_a = &func_node.parameters[0];
    let param_b = &func_node.parameters[1];
    let param_op = &func_node.parameters[2];

    assert_eq!(param_a.name.as_deref(), Some("a"));
    assert_eq!(param_b.name.as_deref(), Some("b"));
    assert_eq!(param_op.name.as_deref(), Some("op"));

    let type_a = find_type_node(graph, param_a.type_id);
    let type_b = find_type_node(graph, param_b.type_id);
    let type_op = find_type_node(graph, param_op.type_id);

    assert!(matches!(&type_a.kind, TypeKind::Named { path, .. } if path == &["i32"]));
    assert!(matches!(&type_b.kind, TypeKind::Named { path, .. } if path == &["i32"]));
    assert!(matches!(&type_op.kind, TypeKind::Named { path, .. } if path == &["MathOperation"]));
    // TODO: Check underlying fn pointer type for MathOperation once alias resolution is better

    // Return Type (i32)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]));
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_const_ptr() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_const_ptr";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()]; // Private function at top level

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    // Private functions default to Inherited visibility in the parser
    assert_eq!(func_node.visibility(), VisibilityKind::Inherited);

    // Parameters (p: *const i32)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("p"));
    let param_type_node = find_type_node(graph, param.type_id);
    // Current state: Falls back to Unknown because TypeKind::Ptr not implemented
    assert!(
        matches!(&param_type_node.kind, TypeKind::Unknown { type_str } if type_str == "* const i32"),
        "Expected TypeKind::Unknown for '*const i32' currently, found {:?}",
        param_type_node.kind
    );
    // #[ignore = "TypeKind::Ptr not yet handled in type_processing.rs"]
    {
        // Target state assertion
        // assert!(matches!(param_type_node.kind, TypeKind::Pointer { is_mutable: false, .. }));
        // assert_eq!(param_type_node.related_types.len(), 1); // Should relate to i32
    }

    // Return Type (i32)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]));
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_consumes_point_in_func_mod() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "consumes_point";
    // Function is defined in src/func/return_types.rs
    let relative_file_path = "src/func/return_types.rs";
    // Module path *within return_types.rs*
    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
    ];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    // pub(crate) is parsed as Restricted(["crate"]) currently
    assert_eq!(
        func_node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()])
    );

    // Parameters (point: Point)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("point"));
    let param_type_node = find_type_node(graph, param.type_id);
    // Should resolve to the top-level Point alias
    assert!(matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Point"]));

    // Return Type (bool)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["bool"]));
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_draw_object() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "draw_object";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters (obj: &dyn Drawable)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("obj"));
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (&dyn Drawable)
    let param_type_node = find_type_node(graph, param.type_id);
    // Should be an immutable reference
    assert!(
        matches!(&param_type_node.kind, TypeKind::Reference { is_mutable, .. } if !*is_mutable),
        "Expected TypeKind::Reference (immutable) for '&dyn Drawable', found {:?}",
        param_type_node.kind
    );
    // Check the referenced type (dyn Drawable)
    assert_eq!(
        param_type_node.related_types.len(),
        1,
        "Reference should have one related type (dyn Drawable)"
    );
    let trait_object_type_id = param_type_node.related_types[0];
    let trait_object_type_node = find_type_node(graph, trait_object_type_id);
    // The underlying trait object type currently falls back to Unknown because TypeKind::TraitObject is not implemented
    assert!(
        matches!(&trait_object_type_node.kind, TypeKind::Unknown { type_str } if type_str == "dyn Drawable"),
        "Expected underlying type 'dyn Drawable' to be TypeKind::Unknown currently, found {:?}",
        trait_object_type_node.kind
    );

    // #[ignore = "TypeKind::TraitObject not yet handled in type_processing.rs"]
    {
        // Target state assertion for the underlying trait object type
        // assert!(matches!(trait_object_type_node.kind, TypeKind::TraitObject { .. }));
        // assert_eq!(trait_object_type_node.related_types.len(), 1); // Should relate to Drawable trait TypeId
    }

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none());
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_impl_trait_arg() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_impl_trait_arg";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.generic_params.is_empty()); // `impl Trait` is not a generic param here
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters (arg: impl Debug)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("arg"));
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (impl Debug)
    let param_type_node = find_type_node(graph, param.type_id);
    // The impl trait type currently falls back to Unknown because TypeKind::ImplTrait is not implemented
    assert!(
        matches!(&param_type_node.kind, TypeKind::Unknown { type_str } if type_str == "impl Debug"),
        "Expected type 'impl Debug' to be TypeKind::Unknown currently, found {:?}",
        param_type_node.kind
    );

    // #[ignore = "TypeKind::ImplTrait not yet handled in type_processing.rs"]
    {
        // Target state assertion for the impl trait type
        // assert!(matches!(param_type_node.kind, TypeKind::ImplTrait { .. }));
        // assert_eq!(param_type_node.related_types.len(), 1); // Should relate to Debug trait TypeId
    }

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none());
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_create_impl_trait_return() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "create_impl_trait_return";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters ()
    assert!(func_node.parameters.is_empty());

    // Return Type (impl Debug)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));

    // Check return TypeNode (impl Debug)
    let return_type_node = find_type_node(graph, return_type_id);
    // The impl trait type currently falls back to Unknown because TypeKind::ImplTrait is not implemented
    assert!(
        matches!(&return_type_node.kind, TypeKind::Unknown { type_str } if type_str == "impl Debug"),
        "Expected type 'impl Debug' to be TypeKind::Unknown currently, found {:?}",
        return_type_node.kind
    );

    // #[ignore = "TypeKind::ImplTrait not yet handled in type_processing.rs"]
    {
        // Target state assertion for the impl trait type
        // assert!(matches!(return_type_node.kind, TypeKind::ImplTrait { .. }));
        // assert_eq!(return_type_node.related_types.len(), 1); // Should relate to Debug trait TypeId
    }
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_mut_ptr() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_mut_ptr";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()]; // Private function at top level

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    // Private functions default to Inherited visibility in the parser
    assert_eq!(func_node.visibility(), VisibilityKind::Inherited);
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters (p: *mut i32)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("p"));
    let param_type_node = find_type_node(graph, param.type_id);
    // Current state: Falls back to Unknown because TypeKind::Ptr not implemented
    assert!(
        matches!(&param_type_node.kind, TypeKind::Unknown { type_str } if type_str == "* mut i32"),
        "Expected TypeKind::Unknown for '*mut i32' currently, found {:?}",
        param_type_node.kind
    );
    // #[ignore = "TypeKind::Ptr not yet handled in type_processing.rs"]
    {
        // Target state assertion
        // assert!(matches!(param_type_node.kind, TypeKind::Pointer { is_mutable: true, .. }));
        // assert_eq!(param_type_node.related_types.len(), 1); // Should relate to i32
    }

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none());
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_inferred_type_example() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "inferred_type_example";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    // let graph = &results // Graph not needed for this test's assertions
    //     .iter()
    //     .find(|data| data.file_path.ends_with(relative_file_path))
    //     .unwrap()
    //     .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters () - None
    assert!(func_node.parameters.is_empty());

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none());

    // Note: We don't currently parse function bodies deeply enough to create
    // nodes or types for inferred types within `let` bindings like `let x = 5;`
    // or `let _y: _ = ...;`. So, there are no specific type assertions to make here
    // regarding the `_` type itself based on the FunctionNode.
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_slice_in_duplicate_names() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_slice";
    let relative_file_path = "src/lib.rs"; // Defined in lib.rs
    let module_path = vec!["crate".to_string(), "duplicate_names".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions (similar to top-level process_slice)
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public); // Public within module

    // Parameters (s: &[u8])
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("s"));
    let param_type_node = find_type_node(graph, param.type_id);
    assert!(
        matches!(&param_type_node.kind, TypeKind::Reference { is_mutable, .. } if !*is_mutable),
        "Expected TypeKind::Reference for '&[u8]', found {:?}",
        param_type_node.kind
    );
    assert_eq!(param_type_node.related_types.len(), 1);
    let slice_type_id = param_type_node.related_types[0];
    let slice_type_node = find_type_node(graph, slice_type_id);
    assert!(
        matches!(&slice_type_node.kind, TypeKind::Unknown { type_str } if type_str == "[u8]"),
        "Expected underlying type '[u8]' to be TypeKind::Unknown currently, found {:?}",
        slice_type_node.kind
    );

    // Return Type (usize)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["usize"]));
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_process_array_in_duplicate_names() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_array";
    let relative_file_path = "src/lib.rs"; // Defined in lib.rs
    let module_path = vec!["crate".to_string(), "duplicate_names".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions (similar to top-level process_array)
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public); // Public within module

    // Parameters (a: Buffer)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("a"));
    let param_type_node = find_type_node(graph, param.type_id);
    // Should resolve to the Buffer alias defined *within* duplicate_names
    assert!(
        matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Buffer"]),
        "Expected TypeKind::Named for alias 'Buffer' (in duplicate_names), found {:?}",
        param_type_node.kind
    );
    // TODO: Deeper check for underlying array type once alias resolution is better

    // Return Type (u8)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["u8"]));
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_generic_func_in_func_mod() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "generic_func";
    let relative_file_path = "src/func/return_types.rs";
    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
    ]; // Defined directly in the file

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(
        func_node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()]) // pub(crate)
    );
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Generics <T: Display + Clone, S: Send + Sync>
    assert_eq!(func_node.generic_params.len(), 2);
    // TODO: Add detailed checks for GenericParamNode kinds and bounds once implemented/needed

    // Parameters (first: T, unused_param: S)
    assert_eq!(func_node.parameters.len(), 2);
    let param_t = &func_node.parameters[0];
    let param_s = &func_node.parameters[1];

    assert_eq!(param_t.name.as_deref(), Some("first"));
    assert_eq!(param_s.name.as_deref(), Some("unused_param"));

    let type_t = find_type_node(graph, param_t.type_id);
    let type_s = find_type_node(graph, param_s.type_id);

    // Check parameter types refer to the generic names
    assert!(matches!(&type_t.kind, TypeKind::Named { path, .. } if path == &["T"]));
    assert!(matches!(&type_s.kind, TypeKind::Named { path, .. } if path == &["S"]));

    // Return Type (T)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    // Should be the same TypeId as the 'T' parameter
    assert_eq!(return_type_id, param_t.type_id);
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["T"]));
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_math_operation_consumer_in_func_mod() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "math_operation_consumer";
    let relative_file_path = "src/func/return_types.rs";
    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
    ]; // private function

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Inherited); // Private function
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters (func_param: MathOperation, x: i32, y: i32)
    assert_eq!(func_node.parameters.len(), 3);
    let param_func = &func_node.parameters[0];
    let param_x = &func_node.parameters[1];
    let param_y = &func_node.parameters[2];

    assert_eq!(param_func.name.as_deref(), Some("func_param"));
    assert_eq!(param_x.name.as_deref(), Some("x"));
    assert_eq!(param_y.name.as_deref(), Some("y"));

    let type_func = find_type_node(graph, param_func.type_id);
    let type_x = find_type_node(graph, param_x.type_id);
    let type_y = find_type_node(graph, param_y.type_id);

    assert!(matches!(&type_func.kind, TypeKind::Named { path, .. } if path == &["MathOperation"]));
    assert!(matches!(&type_x.kind, TypeKind::Named { path, .. } if path == &["i32"]));
    assert!(matches!(&type_y.kind, TypeKind::Named { path, .. } if path == &["i32"]));

    // Return Type (i32)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]));
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_math_operation_producer_in_func_mod() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "math_operation_producer";
    let relative_file_path = "src/func/return_types.rs";
    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
    ];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Inherited); // Private function
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters ()
    assert!(func_node.parameters.is_empty());

    // Return Type (MathOperation)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(
        matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["MathOperation"])
    );
    // TODO: Check underlying fn pointer type once alias resolution is better
}

// --- Tests for functions inside src/func/return_types.rs/restricted_duplicate ---

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_consumes_point_in_restricted_duplicate() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "consumes_point";
    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
    ];
    let relative_file_path = "src/func/return_types.rs";
    // Module path *within return_types.rs* for the nested module

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions (should be identical to consumes_point_in_func_mod)
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    // pub(crate) is parsed as Restricted(["crate"]) currently
    assert_eq!(
        func_node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()])
    );

    // Parameters (point: Point)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("point"));
    let param_type_node = find_type_node(graph, param.type_id);
    // Should resolve to the top-level Point alias (defined outside this module)
    assert!(matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Point"]));

    // Return Type (bool)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["bool"]));
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_function_node_generic_func_in_restricted_duplicate() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "generic_func";
    let relative_file_path = "src/func/return_types.rs";

    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
        "restricted_duplicate".to_string(),
    ];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions (should be identical to generic_func_in_func_mod)
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(
        func_node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()]) // pub(crate)
    );
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Generics <T: Display + Clone, S: Send + Sync>
    assert_eq!(func_node.generic_params.len(), 2);
    // TODO: Add detailed checks for GenericParamNode kinds and bounds once implemented/needed

    // Parameters (first: T, unused_param: S)
    assert_eq!(func_node.parameters.len(), 2);
    let param_t = &func_node.parameters[0];
    let param_s = &func_node.parameters[1];

    assert_eq!(param_t.name.as_deref(), Some("first"));
    assert_eq!(param_s.name.as_deref(), Some("unused_param"));

    let type_t = find_type_node(graph, param_t.type_id);
    let type_s = find_type_node(graph, param_s.type_id);

    // Check parameter types refer to the generic names
    assert!(matches!(&type_t.kind, TypeKind::Named { path, .. } if path == &["T"]));
    assert!(matches!(&type_s.kind, TypeKind::Named { path, .. } if path == &["S"]));

    // Return Type (T)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    // Should be the same TypeId as the 'T' parameter
    assert_eq!(return_type_id, param_t.type_id);
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["T"]));
}
