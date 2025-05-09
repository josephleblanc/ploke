//! Tests for `FunctionNode` parsing and field extraction.
//!
//! ## Test Coverage Analysis
//!
//! *   **Fixture:** `tests/fixture_crates/fixture_types/src/lib.rs` and `tests/fixture_crates/fixture_types/src/func/return_types.rs`
//! *   **Tests:** `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/functions.rs` (using `paranoid_test_fields_and_values!`)
//!
//! ### 1. Coverage of Fixture Items:
//!
//! The `EXPECTED_FUNCTIONS_DATA` and `EXPECTED_FUNCTIONS_ARGS` maps cover 21 distinct standalone function items from the `fixture_types` crate. This includes:
//! *   Functions at the crate root (`src/lib.rs`).
//! *   Functions within the `func::return_types` module.
//! *   Functions within the `func::return_types::restricted_duplicate` nested module.
//! *   Functions within the `duplicate_names` module in `src/lib.rs`.
//!
//! The covered functions test a variety of signatures, visibilities, parameter types (including tuples, slices, arrays, references, function pointers, trait objects, impl Trait), and return types.
//!
//! **Conclusion for Fixture Coverage:** Excellent. All standalone functions from the specified fixture files are covered by the `paranoid_test_fields_and_values!` tests.
//!
//! ### 2. Coverage of `FunctionNode` Property Variations:
//!
//! Based on the 21 items covered:
//!
//! *   `id: FunctionNodeId`: Implicitly covered by ID generation and lookup.
//! *   `name: String`: Excellent coverage (various unique names, including duplicates in different modules).
//! *   `span: (usize, usize)`: Not directly asserted by value in the new tests.
//! *   `visibility: VisibilityKind`: Excellent coverage (`Public`, `Inherited` (private), `Crate`).
//! *   `parameters: Vec<ParamData>`: Tested via `parameter_count`. Detailed `ParamData` (name, type_id, mutability, self) checks are not part of `ExpectedFunctionNode` but are implicitly part of `TypeId` generation for function signatures.
//! *   `return_type: Option<TypeId>`: Tested via `return_type_is_some`. The actual `TypeId` is not directly compared, but its presence/absence is.
//! *   `generic_params: Vec<GenericParamNode>`: Tested via `generic_param_count`. Detailed `GenericParamNode` checks are not part of `ExpectedFunctionNode`.
//! *   `attributes: Vec<Attribute>`: Good coverage (all tested functions currently have no attributes, so `vec![]` is consistently checked).
//! *   `docstring: Option<String>`: Good coverage (all tested functions currently have no docstrings, so `None` is consistently checked).
//! *   `body: Option<String>`: Tested via `body_is_some`. All tested functions have bodies.
//! *   `tracking_hash: Option<TrackingHash>`: Tested via `tracking_hash_check: true`, ensuring presence.
//! *   `cfgs: Vec<String>`: Poor coverage (all tested functions currently have no `cfg` attributes, so `vec![]` is consistently checked).
//!
//! **Conclusion for Property Variation Coverage:**
//! *   **Excellent:** `name`, `visibility`, `parameter_count`, `generic_param_count`, `return_type_is_some`, `body_is_some`, `tracking_hash_check`.
//! *   **Good (but limited variety):** `attributes` (only empty), `docstring` (only `None`).
//! *   **Poor:** `cfgs` (only empty).
//! *   **Not Directly Tested by `ExpectedFunctionNode`:** Specific details of `parameters` (like `ParamData` fields) and `generic_params` (like `GenericParamNode` fields), and the actual `TypeId` of `return_type`. These are indirectly involved in ID generation and type resolution but not asserted field-by-field in these tests.
//!
//! ### 3. Differences in Testing `FunctionNode` vs. Other Nodes:
//!
//! Testing `FunctionNode` focuses on:
//! *   Signature elements: Presence and count of parameters and generic parameters, and presence of a return type.
//! *   Basic metadata: Name, visibility, attributes, docstrings, CFGs.
//! *   The `body_is_some` check confirms the function has a definition.
//!
//! Unlike `ConstNode` or `StaticNode`, there's no `value` to check. Unlike `ImportNode`, there's no complex path or renaming logic.
//!
//! ### 4. Lost Coverage from Old Tests (and `cfg`-gated tests):
//!
//! The `cfg`-gated tests (`test_function_node_consumes_point_in_restricted_duplicate` and `test_function_node_generic_func_in_restricted_duplicate`) performed more detailed checks on:
//! *   Specific `TypeId`s of parameters and return types by looking them up in the `graph.type_graph` and asserting their `TypeKind`.
//! *   Specific names of parameters.
//! *   Specific names of generic type parameters (though not their bounds in detail).
//!
//! This level of detail for `TypeId` and parameter/generic names is **not currently replicated** by the `paranoid_test_fields_and_values!` macro for `FunctionNode`, as `ExpectedFunctionNode` only checks counts and presence.
//!
//! ### 5. Suggestions for Future Inclusions/Improvements:
//!
//! *   **Attributes & Docstrings:** Add fixture functions with attributes and docstrings to improve coverage for these fields.
//! *   **CFGs:** Add fixture functions with `#[cfg(...)]` attributes.
//! *   **Detailed Parameter/Generic/Return Type Checks:**
//!     *   Consider expanding `ExpectedFunctionNode` (and the derive macro) to optionally include expected `TypeId`s or `TypeKind` representations for parameters and return types. This would be a significant enhancement.
//!     *   Alternatively, create separate, more targeted tests (perhaps not using the `paranoid_test_fields_and_values!` macro) for verifying the detailed structure of `ParamData` and `GenericParamNode` for selected complex functions, similar to what the old `cfg`-gated tests were attempting.
//! *   **No-Body Functions:** Add tests for functions declared in traits or `extern` blocks that have no body (`body_is_some: false`).
//! *   **Async/Const/Unsafe Functions:** The current fixture doesn't heavily feature these. While the parser should handle them, specific tests could verify if any unique metadata related to these keywords (if stored on `FunctionNode`) is captured.

#![cfg(test)]

use crate::common::run_phases_and_collect;
use crate::common::ParanoidArgs;
use crate::paranoid_test_fields_and_values; // For EXPECTED_FUNCTIONS_ARGS
use lazy_static::lazy_static;
use ploke_core::ItemKind;
use std::collections::HashMap;
use syn_parser::error::SynParserError; // Import ItemKind and TypeKind from ploke_core
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::ExpectedFunctionNode; // For ExpectedFunctionNode and Attribute
use syn_parser::parser::nodes::PrimaryNodeIdTrait;
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
            return_type_is_some: false, // Returns implicit unit
            body_is_some: true,
        });
        // consumes_point in src/func/return_types.rs
        m.insert("crate::func::return_types::consumes_point", ExpectedFunctionNode { // Renamed key to be unique
            name: "consumes_point",
            visibility: VisibilityKind::Crate, // pub(crate)
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_params_count: 0,
            return_type_is_some: true, // returns bool
            body_is_some: true,
        });
        // generic_func in src/func/return_types.rs
        m.insert("crate::func::return_types::generic_func", ExpectedFunctionNode { // Renamed key
            name: "generic_func",
            visibility: VisibilityKind::Crate, // pub(crate)
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 2,
            generic_params_count: 2,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
            return_type_is_some: true, // returns MathOperation
            body_is_some: true,
        });
        // consumes_point in src/func/return_types.rs/restricted_duplicate
        m.insert("crate::func::return_types::restricted_duplicate::consumes_point", ExpectedFunctionNode {
            name: "consumes_point",
            visibility: VisibilityKind::Crate, // pub(crate)
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 1,
            generic_params_count: 0,
            return_type_is_some: true, // returns bool
            body_is_some: true,
        });
        // generic_func in src/func/return_types.rs/restricted_duplicate
        m.insert("crate::func::return_types::restricted_duplicate::generic_func", ExpectedFunctionNode {
            name: "generic_func",
            visibility: VisibilityKind::Crate, // pub(crate)
            attributes: vec![],
            docstring: None,
            cfgs: vec![],
            tracking_hash_check: true,
            parameter_count: 2,
            generic_params_count: 2,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
            generic_params_count: 0,
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
    let data_key = "crate::duplicate_names::process_tuple";
    let args = EXPECTED_FUNCTIONS_ARGS.get(data_key).unwrap_or_else(|| {
        panic!("ParanoidArgs not found for key: {}", data_key);
    });
    let exp_func = EXPECTED_FUNCTIONS_DATA.get(data_key).unwrap();

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
        .get("crate::duplicate_names::process_tuple")
        .expect("The specified node was not found in they map of expected function nodes.");

    let mut value_matches_iter: HashMap<_, _> = expected_func_node
        .find_node_by_values(test_info.target_data())
        .map(|n| (n.id, n))
        .collect();
    let macro_found_node = value_matches_iter.remove(&node.id).unwrap();
    assert!(macro_found_node.id.to_pid() == node.id.to_pid());
    for (dup_id, dup_node) in value_matches_iter {
        assert!(
            node.id.to_pid() != dup_id.to_pid(),
            "{}, Expected ({}), Actual ({})\nData dump:\n {:#?}",
            "Duplicate FunctionNodeId found.",
            node.id.to_pid(),
            dup_id.to_pid(),
            dup_node
        );
        log::warn!(target: LOG_TEST_FUNCTION,
            "{}: {}\n{}\n\t{}\n\t{} {}\n\t{}",
            "Duplicate values on different path: ",
            "",
            "Two targets were found with matching values.",
            "This indicates that there were duplicate functions at different path locations.",
            "That is fine, so long as you expected to find a duplicate function with the same",
            "name, vis, attrs, docstring, trackinghash, and cfgs in two different files.",
            "If you are seeing this check it means their Ids were correctly not duplicates."
        );
    }
    // assert!(expected_const_node.check_all_fields(node));
    Ok(())
}

paranoid_test_fields_and_values!(
    func_node_process_tuple,
    "crate::process_tuple",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_process_slice,
    "crate::process_slice",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_process_array,
    "crate::process_array",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_process_ref,
    "crate::process_ref",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_process_mut_ref,
    "crate::process_mut_ref",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_apply_op,
    "crate::apply_op",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_process_const_ptr,
    "crate::process_const_ptr",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_process_mut_ptr,
    "crate::process_mut_ptr",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_draw_object,
    "crate::draw_object",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_process_impl_trait_arg,
    "crate::process_impl_trait_arg",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_create_impl_trait_return,
    "crate::create_impl_trait_return",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_inferred_type_example,
    "crate::inferred_type_example",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_consumes_point,
    "crate::func::return_types::consumes_point",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
); // Renamed key to be unique
paranoid_test_fields_and_values!(
    func_node_generic_func,
    "crate::func::return_types::generic_func",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
); // Renamed key
paranoid_test_fields_and_values!(
    func_node_math_operation_consumer,
    "crate::func::return_types::math_operation_consumer",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_math_operation_producer,
    "crate::func::return_types::math_operation_producer",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_consumes_point_dup,
    "crate::func::return_types::restricted_duplicate::consumes_point",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_generic_func_dup,
    "crate::func::return_types::restricted_duplicate::generic_func",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
);
paranoid_test_fields_and_values!(
    func_node_process_tuple_dup,
    "crate::duplicate_names::process_tuple",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
); // Renamed key
paranoid_test_fields_and_values!(
    func_node_process_slice_dup,
    "crate::duplicate_names::process_slice",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
); // Renamed key
paranoid_test_fields_and_values!(
    func_node_process_array_dup,
    "crate::duplicate_names::process_array",
    EXPECTED_FUNCTIONS_ARGS,                         // args_map
    EXPECTED_FUNCTIONS_DATA,                         // expected_data_map
    syn_parser::parser::nodes::FunctionNode,         // node_type
    syn_parser::parser::nodes::ExpectedFunctionNode, // derived Expeced*Node
    as_function,                                     // downcast_method
    LOG_TEST_FUNCTION                                // log_target
); // Renamed key
   // --- Test Cases ---

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
