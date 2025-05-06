//! Contains macro_rules for reducing boilerplate in tests.

/// Macro to generate a test function that performs a "paranoid" check on a node's name.
///
/// It handles:
/// 1. Running Phase 1 & 2 to get parsed graphs.
/// 2. Defining `ParanoidArgs` based on macro inputs.
/// 3. Calling `args.generate_pid()` to get a `TestInfo` struct (which includes the target graph and generated PID).
/// 4. Finding the node in the target graph using the generated PID via `find_node_unique`.
/// 5. Asserting that the `found_node.name()` matches the input `ident`.
///
/// # Arguments
///
/// * `$test_name`: Identifier for the generated test function.
/// * `fixture`: String literal for the fixture directory name.
/// * `relative_file_path`: String literal for the relative path to the source file within the fixture.
/// * `ident`: String literal for the identifier (name) of the item to check.
/// * `expected_path`: Slice literal `&[&str]` for the expected module path of the item's parent.
/// * `item_kind`: An `ploke_core::ItemKind` variant specifying the kind of the item.
/// * `expected_cfg`: An `Option<&[&str]>` for the expected CFG conditions.
///
/// # Example
/// ```ignore
/// paranoid_test_name_check!(
///     my_node_name_test,
///     fixture: "my_fixture",
///     relative_file_path: "src/items.rs",
///     ident: "MY_ITEM",
///     expected_path: &["crate", "items_module"],
///     item_kind: ItemKind::Const,
///     expected_cfg: None
/// );
/// ```
#[macro_export]
macro_rules! paranoid_test_name_check {
    (
        $test_name:ident,
        fixture: $fixture_name:expr,
        relative_file_path: $rel_path:expr,
        ident: $item_ident:expr,
        expected_path: $exp_path:expr,
        item_kind: $kind:expr,
        expected_cfg: $cfg:expr
    ) => {
        #[test]
        fn $test_name() -> Result<(), syn_parser::error::SynParserError> {
            // 1. Run phases
            // Assuming run_phases_and_collect is in scope, typically via crate::common::...
            let successful_graphs = crate::common::run_phases_and_collect($fixture_name);

            // 2. Define ParanoidArgs
            // Assuming ParanoidArgs is in scope, typically via crate::common::...
            let args = crate::common::ParanoidArgs {
                fixture: $fixture_name,
                relative_file_path: $rel_path,
                ident: $item_ident,
                expected_path: $exp_path,
                item_kind: $kind,
                expected_cfg: $cfg,
            };

            // 3. Generate PID and get TestInfo
            let test_info = args.generate_pid(&successful_graphs)?;

            // 4. Find the node using the generated ID
            // GraphAccess and GraphNode traits need to be in scope for find_node_unique and name()
            // Assuming test_info.target_data() returns &ParsedCodeGraph which impl GraphAccess
            // and found_node will be &dyn GraphNode
            let graph_data = test_info.target_data();
            let expected_pid = test_info.test_pid();
            let found_node: &dyn syn_parser::parser::graph::GraphNode = graph_data.find_node_unique(expected_pid.into())?;

            // 5. Perform the name assertion
            assert_eq!(
                found_node.name(),
                args.ident,
                "Mismatch for name field. Expected: '{}', Actual: '{}'",
                args.ident,
                found_node.name()
            );

            Ok(())
        }
    };
}

/// Macro to generate a test function that performs detailed field checks and a value-based lookup
/// for a CONST item.
///
/// It handles:
/// 1. Running Phase 1 & 2 to get parsed graphs.
/// 2. Defining `ParanoidArgs` based on macro inputs.
/// 3. Retrieving the corresponding `ExpectedConstData` for the item.
/// 4. Attempting to find the node via ID using `args.generate_pid()` and `find_node_unique`.
/// 5. If ID lookup succeeds, it calls each `is_*_match_debug` method from `ExpectedConstData` on the found node.
/// 6. Regardless of ID lookup success/failure (or as a primary check), it uses `ExpectedConstData::find_node_by_values`
///    to ensure the item can be found based on its expected field values, asserting that exactly one match is found.
///
/// # Arguments
///
/// * `$test_name`: Identifier for the generated test function.
/// * `fixture`: String literal for the fixture directory name.
/// * `relative_file_path`: String literal for the relative path to the source file within the fixture.
/// * `ident`: String literal for the identifier (name) of the const item to check.
/// * `expected_path`: Slice literal `&[&str]` for the expected module path of the item's parent.
/// * `expected_cfg`: An `Option<&[&str]>` for the expected CFG conditions.
///
/// # Panics
/// Panics if `ExpectedConstData` is not found for the given `ident`.
/// Panics if `find_node_by_values` does not find exactly one matching node.
///
/// # Example
/// ```ignore
/// paranoid_test_fields_and_values_const!(
///     my_const_full_check,
///     fixture: "my_fixture",
///     relative_file_path: "src/constants.rs",
///     ident: "MY_SPECIAL_CONST",
///     expected_path: &["crate", "constants_module"],
///     expected_cfg: None
/// );
/// ```
#[macro_export]
macro_rules! paranoid_test_fields_and_values_const {
    (
        $test_name:ident,
        fixture: $fixture_name:expr,
        relative_file_path: $rel_path:expr,
        ident: $item_ident:expr,
        expected_path: $exp_path:expr,
        expected_cfg: $cfg:expr
    ) => {
        #[test]
        fn $test_name() -> Result<(), syn_parser::error::SynParserError> {
            let _ = env_logger::builder()
                .is_test(true)
                .format_timestamp(None)
                .try_init();

            // 1. Run phases
            let successful_graphs = crate::common::run_phases_and_collect($fixture_name);

            // 2. Define ParanoidArgs
            let args = crate::common::ParanoidArgs {
                fixture: $fixture_name,
                relative_file_path: $rel_path,
                ident: $item_ident,
                expected_path: $exp_path,
                item_kind: ploke_core::ItemKind::Const, // Specific to ConstNode
                expected_cfg: $cfg,
            };

            // 3. Get ExpectedConstData
            // Ensure EXPECTED_CONSTS_DATA is in scope, typically from the test module
            // e.g., use crate::uuid_phase2_partial_graphs::nodes::const_static::EXPECTED_CONSTS_DATA;
            let expected_data = crate::uuid_phase2_partial_graphs::nodes::const_static::EXPECTED_CONSTS_DATA
                .get($item_ident)
                .unwrap_or_else(|| panic!("ExpectedConstData not found for ident: {}", $item_ident));

            // 4. Find the target ParsedCodeGraph
            let target_graph_data = successful_graphs
                .iter()
                .find(|pg| pg.file_path.ends_with(args.relative_file_path))
                .unwrap_or_else(|| {
                    panic!(
                        "Target graph '{}' not found for item '{}'.",
                        args.relative_file_path, args.ident
                    )
                });
            
            args.check_graph(target_graph_data)?; // Log graph context

            // 5. Attempt ID-based lookup and individual field checks
            match args.generate_pid(&successful_graphs) {
                Ok(test_info) => {
                    match test_info.target_data().find_node_unique(test_info.test_pid().into()) {
                        Ok(node) => {
                            if let Some(const_node) = node.as_const() {
                                log::info!(target: crate::uuid_phase2_partial_graphs::nodes::const_static::LOG_TEST_CONST, "Performing individual field checks for '{}' via ID lookup.", args.ident);
                                assert!(expected_data.is_name_match_debug(const_node), "Name mismatch for {}", args.ident);
                                assert!(expected_data.is_vis_match_debug(const_node), "Visibility mismatch for {}", args.ident);
                                assert!(expected_data.is_attr_match_debug(const_node), "Attributes mismatch for {}", args.ident);
                                assert!(expected_data.is_type_id_check_match_debug(const_node), "Type ID check mismatch for {}", args.ident);
                                assert!(expected_data.is_value_match_debug(const_node), "Value mismatch for {}", args.ident);
                                assert!(expected_data.is_docstring_contains_match_debug(const_node), "Docstring mismatch for {}", args.ident);
                                assert!(expected_data.is_tracking_hash_check_match_debug(const_node), "Tracking hash check mismatch for {}", args.ident);
                                assert!(expected_data.is_cfgs_match_debug(const_node), "CFGs mismatch for {}", args.ident);
                            } else {
                                panic!("Node found by ID for '{}' was not a ConstNode.", args.ident);
                            }
                        }
                        Err(e) => {
                            log::warn!(target: crate::uuid_phase2_partial_graphs::nodes::const_static::LOG_TEST_CONST, "Node lookup by PID '{}' failed for '{}' (Error: {:?}). Proceeding with value-based check only.", test_info.test_pid(), args.ident, e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!(target: crate::uuid_phase2_partial_graphs::nodes::const_static::LOG_TEST_CONST, "PID generation failed for '{}' (Error: {:?}). Proceeding with value-based check only.", args.ident, e);
                }
            }

            // 6. Perform value-based lookup and count assertion
            log::info!(target: crate::uuid_phase2_partial_graphs::nodes::const_static::LOG_TEST_CONST, "Performing value-based lookup for '{}'.", args.ident);
            let matching_nodes_by_value: Vec<_> = expected_data.find_node_by_values(target_graph_data).collect();
            assert_eq!(
                matching_nodes_by_value.len(),
                1,
                "Expected to find exactly one ConstNode matching values for '{}'. Found {}.",
                args.ident,
                matching_nodes_by_value.len()
            );
            // Optionally, further assert that the found node by value is indeed the one we expect,
            // if we had a way to get its ID and compare with a regenerated one.
            // For now, finding exactly one is the primary assertion here.

            Ok(())
        }
    };
}
