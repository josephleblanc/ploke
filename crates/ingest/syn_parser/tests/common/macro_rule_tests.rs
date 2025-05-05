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
