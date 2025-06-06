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
            let successful_graphs = $crate::common::run_phases_and_collect($fixture_name);

            // 2. Define ParanoidArgs
            // Assuming ParanoidArgs is in scope, typically via crate::common::...
            let args = $crate::common::ParanoidArgs {
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
            let found_node: &dyn syn_parser::parser::graph::GraphNode =
                graph_data.find_node_unique(expected_pid.into())?;

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
/// for a specific node type (e.g., ConstNode, StaticNode).
///
/// It assumes the existence of:
///   - An `Expected<NodeType>Data` struct (generated by `derive(ExpectedData)`).
///   - A `lazy_static!` HashMap named `$args_map` mapping string keys to `ParanoidArgs`.
///   - A `lazy_static!` HashMap named `$expected_data_map` mapping idents (`&str`) to `Expected<NodeType>Data`.
///   - A log target constant named `$log_target`.
///
/// It performs the following steps:
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
/// * `$test_name`: Identifier for the generated test function.
/// * `$data_key`: Expression evaluating to `&str`, the key for `$args_map` and `$expected_data_map`.
///     * Note: Must have matching `$data_key` for both items.
/// * `$args_map`: Identifier of the `lazy_static!` map containing `ParanoidArgs`.
/// * `$expected_data_map`: Identifier of the `lazy_static!` map containing `Expected*Data`.
/// * `$node_type`: Path to the node struct type (e.g., `crate::parser::nodes::ConstNode`).
/// * `$expected_data_type`: Path to the expected data struct type (e.g., `crate::parser::nodes::ExpectedConstNode`).
/// * `$downcast_method`: Identifier of the method to downcast `&dyn GraphNode` (e.g., `as_const`).
/// * `$log_target`: Identifier of the log target constant (e.g., `LOG_TEST_CONST`).
///
/// # Panics
/// Panics if `ParanoidArgs` or `Expected*Data` is not found for the given keys/idents.
/// Panics if `find_node_by_values` does not find exactly one matching node.
/// Panics if the node found by ID cannot be downcast using `$downcast_method`.
///
/// # Example
/// ```ignore
/// // Assuming EXPECTED_CONSTS_ARGS, EXPECTED_CONSTS_DATA, LOG_TEST_CONST are in scope
/// paranoid_test_fields_and_values!(
///     my_const_full_check,
///     "crate::constants::MY_SPECIAL_CONST", // args_key
///     EXPECTED_CONSTS_ARGS,                 // args_map
///     EXPECTED_CONSTS_DATA,                 // expected_data_map
///     crate::parser::nodes::ConstNode,      // node_type
///     crate::parser::nodes::ExpectedConstNode, // expected_data_type
///     as_const,                             // downcast_method
///     LOG_TEST_CONST                        // log_target
/// );
/// ```
#[macro_export]
macro_rules! paranoid_test_fields_and_values {
    (
        $test_name:ident,
        $data_key:expr,
        $args_map:ident,
        $expected_data_map:ident,
        $node_type:path,
        $expected_data_type:path,
        $downcast_method:ident,
        $log_target:ident
    ) => {
        #[test]
        fn $test_name() -> Result<(), syn_parser::error::SynParserError> {
            let _ = env_logger::builder()
                .is_test(true)
                .format_timestamp(None)
                .try_init();

            // 1. Look up ParanoidArgs and ExpectedConstData from provided maps
            let args = $args_map.get($data_key).unwrap_or_else(|| {
                panic!("ParanoidArgs not found for key: {}", $data_key);
            });
            // Ensure the type annotation matches the parameter
            let expected_data: &$expected_data_type = $expected_data_map.get($data_key).unwrap_or_else(|| {
                 panic!("{} not found for key: {}", stringify!($expected_data_type), $data_key);
            });

            // 2. Use the lazy_static parsed fixture based on args.fixture
            let successful_graphs = match args.fixture {
                "fixture_nodes" => &*$crate::common::PARSED_FIXTURE_CRATE_NODES,
                "file_dir_detection" => &*$crate::common::PARSED_FIXTURE_CRATE_DIR_DETECTION,
                "fixture_types" => &*$crate::common::PARSED_FIXTURE_CRATE_TYPES,
                _ => panic!("Unknown fixture name for lazy_static lookup: {}. Ensure it's defined in tests/common/parsed_fixtures.rs and matched here.", args.fixture),
            };

            // 3. Find the target ParsedCodeGraph using relative_file_path from retrieved args
            let target_graph_data = successful_graphs
                .iter()
                .find(|pg| pg.file_path.ends_with(args.relative_file_path))
                .unwrap_or_else(|| {
                    panic!(
                        "Target graph '{}' not found for item '{}'.",
                        args.relative_file_path, $data_key
                    )
                });

            args.check_graph(target_graph_data)?; // Log graph context

            // 5. Attempt ID-based lookup and individual field checks
            // Store the matching id to disambiguate value matches.
            // NOTE: We must disambiguate using the ID, as opposed to choosing to store the
            // disambiguiting information directly on the node (e.g. the node path), because the
            // node path may change between phase 2 and phase 3, and it does not make sense to
            // track the changeable node path before it is canonized in phase 3.
            // Subnote: This is primarily due to complexity introduced by tracking the `#[path]`
            // attribute of a node.
            let node_pid_result = match args.generate_pid(&successful_graphs) {
                Ok(test_info) => {
                    match test_info.target_data().find_node_unique(test_info.test_pid().into()) {
                        Ok(node) => {
                            // Use the parameterized downcast method
                            if let Some(specific_node) = node.$downcast_method() {
                               // Use the parameterized log target
                               log::info!(target: $log_target, "Performing individual field checks for ident '{}' via ID lookup.", args.ident);
                               // Call the check_all_fields method generated by the derive macro
                               // Pass the graph to check_all_fields
                               assert!(expected_data.check_all_fields(specific_node),
                                    "One or more field checks failed (see logs for ident '{}' with qualified path (data_key) '{}')",
                                    args.ident,
                                    $data_key
                                );

                               // --- Add Relation Check ---
                               // Convert expected_path from &[&str] to Vec<String> for the function call
                                let expected_path_vec: Vec<String> = args.expected_path.iter().map(|s| s.to_string()).collect();
                                // Find parent module using the Vec<String> slice
                                // return Some(parent_mod_id) unless it's a file-level module
                                match target_graph_data
                                    .find_module_by_path_checked(&expected_path_vec)
                                {
                                    Ok(parent_module) => {
                                // Check for Contains relation
                                let relation_found = target_graph_data.relations().iter().any(|rel| {
                                    matches!(rel, syn_parser::parser::relations::SyntacticRelation::Contains { source, target }
                                        if *source == parent_module.id && *target == specific_node.id.to_pid())
                                });
                                assert!(
                                    relation_found,
                                    "Missing SyntacticRelation::Contains from parent module {} to node {}. Data dump of parent ModuleNode info follows:\n{:#?}",
                                    parent_module.id, specific_node.id, parent_module,
                                );
                                // Use the parameterized log target
                                log::debug!(target: $log_target, "   Relation Check: Found Contains relation from parent module.");
                                // --- End Relation Check ---
                                    },
                                    Err(_) => {
                                        let _ = target_graph_data.find_module_by_file_path_checked(std::path::Path::new(args.relative_file_path))?;
                                        log::debug!(target: $log_target, "   Relation Check: Skipping contains relation check for parent of file-level module.");
                                    }
                                };

                                Ok(specific_node.id.to_pid())

                            } else {
                                // Use the parameterized node type name
                                panic!("Node found by ID for '{}' was not a {}.", args.ident, stringify!($node_type));
                            }
                        }
                        Err(e) => {
                            // Use the parameterized log target
                            log::warn!(target: $log_target, "Node lookup by PID '{}' failed for '{}' (Error: {:?}). Proceeding with value-based check only.", test_info.test_pid(), args.ident, e);
                            Err(e)
                        }
                    }
                }
                Err(e) => {
                    // Use the parameterized log target
                    log::warn!(target: $log_target, "PID generation failed for '{}' (Error: {:?}). Proceeding with value-based check only.", args.ident, e);
                    Err(e)
                }
            };

            // 6. Perform value-based lookup and count assertion
            // Use the parameterized log target
            log::info!(target: $log_target, "Performing value-based lookup for '{}'.", args.ident);
            // Iterator in `find_node_by_values` prints logging info
            // This should allow all logging info to be shown while the value matched nodes are
            // collected.
            let matched_nodes_by_value: Vec<_> = expected_data.find_node_by_values(target_graph_data).collect();
            // Checks for actual match of node id in values.
            let node_pid_match = node_pid_result.inspect_err(|_| {
                log::trace!(target: $log_target,
                    "Found {} matches by value.\n{:=^60}\n{}\n {:=^60}\n{:#?}",
                    matched_nodes_by_value.len(),
                    " Matched Items ",
                    matched_nodes_by_value
                        .iter()
                        .enumerate()
                        .map(|(i, node)| {
                            format!("Match #{}:\n\tName: {}\n\tExpected Path: {:?}\n\tId: {}\n\tFull Id: {:?}",
                                i, args.ident, args.expected_path, node.id, node.id
                            )
                    }).collect::<Vec<String>>().join("\n"),
                    " Matched Item Info Dump ",
                    matched_nodes_by_value
                );
            })?;
            let value_matches_not_pid = matched_nodes_by_value.iter().filter(|n| n.id.to_pid() != node_pid_match);
            for value_dup_node in value_matches_not_pid {
            // Sanity Check: Not really necessary to assert here after filter.
            assert!(
                node_pid_match != value_dup_node.id.to_pid(),
                "Duplicate FunctionNodeId found for Expected: {}, Actual: {}. Data dump of duplicate node follows: {:#?}",
                node_pid_match, value_dup_node.id.to_pid(),
                value_dup_node
            );
            log::warn!(target: $log_target,
                "{}: {}\n{}\n\t{}\n\t{} {}\n\t{}",
                "Duplicate values on different path: ",
                "",
                "More than one target was found with matching values.",
                "This indicates that there were duplicate nodes at different path locations.",
                "That is fine, so long as you expected to find a duplicate nodes with the same",
                "values (e.g. name, docstring, etc) in two different files.",
                "If you are seeing this check it means their Ids were correctly not duplicates."
            );
            }

            let value_and_pid_matches: Vec<_> = matched_nodes_by_value.iter().filter(|n| n.id.to_pid() == node_pid_match).collect();
            assert_eq!(value_and_pid_matches.len(), 1,
                    "Expected to find exactly one {} matching values and ID for '{}'. Found {}.\n{} {:#?}",
                    stringify!($node_type),
                    $data_key,
                    matched_nodes_by_value.len(),
                    "Data dump of duplicate nodes:",
                    value_and_pid_matches,
            );

            Ok(())
        }
    };
}

#[macro_export]
macro_rules! paranoid_test_setup {
    (
        $setup_name:ident,
        $data_key:expr,
        $args_map:ident,
        $expected_data_map:ident,
        $node_type:path,
        $expected_data_type:path,
        $downcast_method:ident,
        $log_target:ident
    ) => {
        fn $setup_name() -> Result<( $node_type, &'static ParsedCodeGraph ), syn_parser::error::SynParserError> {
            // NOTE: See if the logging works in the other macro below and delete this if so
            // let _ = env_logger::builder()
            //     .is_test(true)
            //     .format_timestamp(None)
            //     .try_init();

            // 1. Look up ParanoidArgs and ExpectedConstData from provided maps
            let args = $args_map.get($data_key).unwrap_or_else(|| {
                panic!("ParanoidArgs not found for key: {}", $data_key);
            });
            // Ensure the type annotation matches the parameter
            let expected_data: &$expected_data_type = $expected_data_map.get($data_key).unwrap_or_else(|| {
                 panic!("{} not found for key: {}", stringify!($expected_data_type), $data_key);
            });

            // 2. Use the lazy_static parsed fixture based on args.fixture
            let successful_graphs = match args.fixture {
                "fixture_nodes" => &*$crate::common::PARSED_FIXTURE_CRATE_NODES,
                "file_dir_detection" => &*$crate::common::PARSED_FIXTURE_CRATE_DIR_DETECTION,
                "fixture_types" => &*$crate::common::PARSED_FIXTURE_CRATE_TYPES,
                _ => panic!("Unknown fixture name for lazy_static lookup: {}. Ensure it's defined in tests/common/parsed_fixtures.rs and matched here.", args.fixture),
            };

            // 3. Find the target ParsedCodeGraph using relative_file_path from retrieved args
            let graph_data = successful_graphs
                .iter()
                .find(|pg| pg.file_path.ends_with(args.relative_file_path))
                .unwrap_or_else(|| {
                    panic!(
                        "Target graph '{}' not found for item '{}'.",
                        args.relative_file_path, $data_key
                    )
                });

            args.check_graph(graph_data)?; // Log graph context

            // 5. Attempt ID-based lookup and individual field checks
            // Store the matching id to disambiguate value matches.
            // NOTE: We must disambiguate using the ID, as opposed to choosing to store the
            // disambiguiting information directly on the node (e.g. the node path), because the
            // node path may change between phase 2 and phase 3, and it does not make sense to
            // track the changeable node path before it is canonized in phase 3.
            // Subnote: This is primarily due to complexity introduced by tracking the `#[path]`
            // attribute of a node.
            let node_result = match args.generate_pid(&successful_graphs) {
                Ok(test_info) => {
                    match test_info.target_data().find_node_unique(test_info.test_pid().into()) {
                        Ok(node) => {
                            // Use the parameterized downcast method
                            if let Some(specific_node) = node.$downcast_method() {
                               // Use the parameterized log target
                               log::info!(target: $log_target, "Performing individual field checks for ident '{}' via ID lookup.", args.ident);
                               // Call the check_all_fields method generated by the derive macro
                               // Pass the graph to check_all_fields
                               assert!(expected_data.check_all_fields(specific_node),
                                    "One or more field checks failed (see logs for ident '{}' with qualified path (data_key) '{}')",
                                    args.ident,
                                    $data_key
                                );

                               // --- Add Relation Check ---
                               // Convert expected_path from &[&str] to Vec<String> for the function call
                                let expected_path_vec: Vec<String> = args.expected_path.iter().map(|s| s.to_string()).collect();
                                // Find parent module using the Vec<String> slice
                                // return Some(parent_mod_id) unless it's a file-level module
                                match graph_data
                                    .find_module_by_path_checked(&expected_path_vec)
                                {
                                    Ok(parent_module) => {
                                // Check for Contains relation
                                let relation_found = graph_data.relations().iter().any(|rel| {
                                    matches!(rel, syn_parser::parser::relations::SyntacticRelation::Contains { source, target }
                                        if *source == parent_module.id && *target == specific_node.id.to_pid())
                                });
                                assert!(
                                    relation_found,
                                    "Missing SyntacticRelation::Contains from parent module {} to node {}. Data dump of parent ModuleNode info follows:\n{:#?}",
                                    parent_module.id, specific_node.id, parent_module,
                                );
                                // Use the parameterized log target
                                log::debug!(target: $log_target, "   Relation Check: Found Contains relation from parent module.");
                                // --- End Relation Check ---
                                    },
                                    Err(_) => {
                                        let _ = graph_data.find_module_by_file_path_checked(std::path::Path::new(args.relative_file_path))?;
                                        log::debug!(target: $log_target, "   Relation Check: Skipping contains relation check for parent of file-level module.");
                                    }
                                };

                                Ok(specific_node.clone())

                            } else {
                                // Use the parameterized node type name
                                panic!("Node found by ID for '{}' was not a {}.", args.ident, stringify!($node_type));
                            }
                        }
                        Err(e) => {
                            // Use the parameterized log target
                            log::warn!(target: $log_target, "Node lookup by PID '{}' failed for '{}' (Error: {:?}). Proceeding with value-based check only.", test_info.test_pid(), args.ident, e);
                            Err(e)
                        }
                    }
                }
                Err(e) => {
                    // Use the parameterized log target
                    log::warn!(target: $log_target, "PID generation failed for '{}' (Error: {:?}). Proceeding with value-based check only.", args.ident, e);
                    Err(e)
                }
            };

            // 6. Perform value-based lookup and count assertion
            // Use the parameterized log target
            log::info!(target: $log_target, "Performing value-based lookup for '{}'.", args.ident);
            // Iterator in `find_node_by_values` prints logging info
            // This should allow all logging info to be shown while the value matched nodes are
            // collected.
            let matched_nodes_by_value: Vec<_> = expected_data.find_node_by_values(graph_data).collect();
            // Checks for actual match of node id in values.
            let target_node = node_result.inspect_err(|_| {
                log::trace!(target: $log_target,
                    "Found {} matches by value.\n{:=^60}\n{}\n {:=^60}\n{:#?}",
                    matched_nodes_by_value.len(),
                    " Matched Items ",
                    matched_nodes_by_value
                        .iter()
                        .enumerate()
                        .map(|(i, node)| {
                            format!("Match #{}:\n\tName: {}\n\tExpected Path: {:?}\n\tId: {}\n\tFull Id: {:?}",
                                i, args.ident, args.expected_path, node.id, node.id
                            )
                    }).collect::<Vec<String>>().join("\n"),
                    " Matched Item Info Dump ",
                    matched_nodes_by_value
                );
            })?;
            let node_pid_match = target_node.id.to_pid();
            let value_matches_not_pid = matched_nodes_by_value.iter().filter(|n| n.id.to_pid() != node_pid_match);
            for value_dup_node in value_matches_not_pid {
            // Sanity Check: Not really necessary to assert here after filter.
            assert!(
                node_pid_match != value_dup_node.id.to_pid(),
                "Duplicate FunctionNodeId found for Expected: {}, Actual: {}. Data dump of duplicate node follows: {:#?}",
                node_pid_match, value_dup_node.id.to_pid(),
                value_dup_node
            );
            log::warn!(target: $log_target,
                "{}: {}\n{}\n\t{}\n\t{} {}\n\t{}",
                "Duplicate values on different path: ",
                "",
                "More than one target was found with matching values.",
                "This indicates that there were duplicate nodes at different path locations.",
                "That is fine, so long as you expected to find a duplicate nodes with the same",
                "values (e.g. name, docstring, etc) in two different files.",
                "If you are seeing this check it means their Ids were correctly not duplicates."
            );
            }

            let value_and_pid_matches: Vec<_> = matched_nodes_by_value.iter().filter(|n| n.id.to_pid() == node_pid_match).collect();
            assert_eq!(value_and_pid_matches.len(), 1,
                    "Expected to find exactly one {} matching values and ID for '{}'. Found {}.\n{} {:#?}",
                    stringify!($node_type),
                    $data_key,
                    matched_nodes_by_value.len(),
                    "Data dump of duplicate nodes:",
                    value_and_pid_matches,
            );

            Ok(( target_node, graph_data ))
        }
    };
}

#[macro_export]
macro_rules! run_paranoid_test {
    ($setup:ident, $test_name:ident $(, $test_body:expr)?) => {
        #[test]
        fn $test_name() -> Result<(), syn_parser::error::SynParserError> {
            let _ = env_logger::builder()
                .is_test(true)
                .format_timestamp(None)
                .try_init();

            let item = $setup()?;
            $(
                $test_body(item)?
            )?;
            Ok(())
        }
    };
}
