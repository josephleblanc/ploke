#![cfg(test)]
use crate::common::run_phases_and_collect;
use crate::common::ParanoidArgs;
use crate::paranoid_test_fields_and_values; // Corrected macro name
// Removed: use crate::paranoid_test_fields_and_values_const;
use crate::paranoid_test_name_check;
use lazy_static::lazy_static;
use ploke_core::ItemKind;
use std::collections::HashMap;
use syn_parser::error::SynParserError;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::Attribute;
use syn_parser::parser::nodes::PrimaryNodeIdTrait;
use syn_parser::parser::types::VisibilityKind;

// NOTE: Tests for associated types (`test_associated_type_found_in_impl`, `test_associated_type_found_in_trait`)
// are omitted for now as the current `const_static.rs` fixture does not contain associated types.
// They should be added when fixtures are updated or when testing traits/impls specifically.

pub const LOG_TEST_CONST: &str = "log_test_const";

// Struct to hold expected fields for a ConstNode
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedConstData {
    name: &'static str,
    visibility: VisibilityKind,
    type_id_check: bool, // Just check if it's Synthetic for now
    value: Option<&'static str>,
    attributes: Vec<Attribute>, // Store expected non-cfg attributes
    docstring_contains: Option<&'static str>,
    tracking_hash_check: bool, // Check if Some
    cfgs: Vec<String>,
}

// Removed: use syn_parser::parser::nodes::ExpectedConstNode;
// The generated struct is in crate::parser::nodes::consts

// --- Lazy Static Maps ---
lazy_static! {
    // Map from ident -> ExpectedConstData
    static ref EXPECTED_CONSTS_DATA: HashMap<&'static str, ExpectedConstNode> = {
        let mut m = HashMap::new();
        m.insert("TOP_LEVEL_INT", ExpectedConstNode {
            name: "TOP_LEVEL_INT",
            visibility: VisibilityKind::Inherited,
            type_id_check: true,
            value: Some("10"),
            attributes: vec![],
            docstring_contains: Some("top-level private constant"),
            tracking_hash_check: true,
            cfgs: vec![],
        });
        m.insert("doc_attr_const", ExpectedConstNode {
            name: "doc_attr_const",
            visibility: VisibilityKind::Public,
            type_id_check: true,
            value: Some("3.14"),
            attributes: vec![
                Attribute {name:"deprecated".to_string(),args:vec!["note = \"Use NEW_DOC_ATTR_CONST instead\"".to_string()],value:None },
                // Corrected args for allow attribute
                Attribute {name:"allow".to_string(),args:vec!["non_upper_case_globals".to_string(), "clippy :: approx_constant".to_string()],value:None },
            ],
            docstring_contains: Some("This is a documented constant."),
            tracking_hash_check: true,
            cfgs: vec![],
        });
        m.insert("TOP_LEVEL_BOOL", ExpectedConstNode {
            name: "TOP_LEVEL_BOOL",
            visibility: VisibilityKind::Public,
            type_id_check: true,
            value: Some("true"),
            attributes: vec![],
            docstring_contains: Some("top-level public constant"),
            tracking_hash_check: true,
            cfgs: vec![],
        });
        m.insert("INNER_CONST", ExpectedConstNode {
            name: "INNER_CONST",
            visibility: VisibilityKind::Crate, // pub(crate)
            type_id_check: true,
            value: Some("1"),
            attributes: vec![],
            docstring_contains: None, // No doc comment
            tracking_hash_check: true,
            cfgs: vec![],
        });
        m.insert("ARRAY_CONST", ExpectedConstNode {
            name: "ARRAY_CONST",
            visibility: VisibilityKind::Inherited,
            type_id_check: true,
            value: Some("[1 , 2 , 3]"), // Assuming minimal spacing
            attributes: vec![],
            docstring_contains: None,
            tracking_hash_check: true,
            cfgs: vec![],
        });
        m.insert("STRUCT_CONST", ExpectedConstNode {
            name: "STRUCT_CONST",
            visibility: VisibilityKind::Inherited,
            type_id_check: true,
            value: Some("SimpleStruct { x : 99 , y : true }"), // Assuming syn spacing
            attributes: vec![],
            docstring_contains: Some("Constant struct instance."),
            tracking_hash_check: true,
            cfgs: vec![],
        });
        m.insert("ALIASED_CONST", ExpectedConstNode {
            name: "ALIASED_CONST",
            visibility: VisibilityKind::Inherited,
            type_id_check: true,
            value: Some("- 5"),
            attributes: vec![],
            docstring_contains: Some("Constant using a type alias."),
            tracking_hash_check: true,
            cfgs: vec![],
        });
        m.insert("EXPR_CONST", ExpectedConstNode {
            name: "EXPR_CONST",
            visibility: VisibilityKind::Inherited,
            type_id_check: true,
            value: Some("5 * 2 + 1"),
            attributes: vec![],
            docstring_contains: None,
            tracking_hash_check: true,
            cfgs: vec![],
        });
        m.insert("FN_CALL_CONST", ExpectedConstNode {
            name: "FN_CALL_CONST",
            visibility: VisibilityKind::Inherited,
            type_id_check: true,
            value: Some("five ()"), // Assuming space before ()
            attributes: vec![],
            docstring_contains: Some("Constant initialized with a call to a const function."),
            tracking_hash_check: true,
            cfgs: vec![],
        });
        // Add more const examples if needed
        m
    };
}

// Define the static array using ParanoidArgs
lazy_static! {
    static ref EXPECTED_CONSTS_ARGS: HashMap<&'static str, ParanoidArgs<'static>> = {
        let mut m = HashMap::new();
        m.insert("crate::const_static::TOP_LEVEL_INT", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "TOP_LEVEL_INT",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Const,
        });
        m.insert("crate::const_static::TOP_LEVEL_BOOL", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "TOP_LEVEL_BOOL",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Const,
        });
        m.insert("crate::const_static::TOP_LEVEL_STR", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "TOP_LEVEL_STR",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Static,
        });
        m.insert("crate::const_static::TOP_LEVEL_COUNTER", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "TOP_LEVEL_COUNTER",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Static,
        });
        m.insert("crate::const_static::TOP_LEVEL_CRATE_STATIC", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "TOP_LEVEL_CRATE_STATIC", // pub(crate)
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Static,
        });
        m.insert("crate::const_static::ARRAY_CONST", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "ARRAY_CONST",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Const,
        });
        m.insert("crate::const_static::TUPLE_STATIC", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "TUPLE_STATIC",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Static,
        });
        m.insert("crate::const_static::STRUCT_CONST", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "STRUCT_CONST",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Const,
        });
        m.insert("crate::const_static::ALIASED_CONST", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "ALIASED_CONST",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Const,
        });
        m.insert("crate::const_static::EXPR_CONST", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "EXPR_CONST",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Const,
        });
        m.insert("crate::const_static::FN_CALL_CONST", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "FN_CALL_CONST",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Const,
        });
        m.insert("crate::const_static::doc_attr_const", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "doc_attr_const",
            expected_cfg: None, // Attributes are not CFGs
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Const,
        });
        m.insert("crate::const_static::DOC_ATTR_STATIC", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "DOC_ATTR_STATIC",
            expected_cfg: Some(&["target_os = \"linux\""]), // This one has a CFG
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Static,
        });
        // --- Inner Mod Items ---
        m.insert("crate::const_static::inner_mod::INNER_CONST", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs", // Defined in this file
            ident: "INNER_CONST",
            expected_cfg: None,
            expected_path: &["crate", "const_static", "inner_mod"], // Path within the file
            item_kind: ItemKind::Const,
        });
        m.insert("crate::const_static::inner_mod::INNER_MUT_STATIC", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs", // Defined in this file
            ident: "INNER_MUT_STATIC",
            expected_cfg: None,
            expected_path: &["crate", "const_static", "inner_mod"], // Path within the file
            item_kind: ItemKind::Static,
        });
        m
    };
}

// Replaced by macro invocation below
// TODO: Comment out after verifying that both this test and the macro replacing it are correctly
// running before removing this test
#[test]
fn test_value_node_field_name_standard() -> Result<(), SynParserError> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .try_init();
    // Original was Result<()> which is FixtureError
    // Collect successful graphs
    let successful_graphs = run_phases_and_collect("fixture_nodes");

    // Use ParanoidArgs to find the node
    let args_key = "crate::const_static::TOP_LEVEL_BOOL";
    let args = EXPECTED_CONSTS_ARGS.get(args_key).unwrap_or_else(|| {
        panic!("ParanoidArgs not found for key: {}", args_key);
    });
    let exp_const = EXPECTED_CONSTS_DATA.get(args.ident).unwrap();

    // Generate the expected PrimaryNodeId using the method on ParanoidArgs
    let test_info = args.generate_pid(&successful_graphs).inspect_err(|e| {
        log::warn!(target: LOG_TEST_CONST, "PID generation failed for '{}' (Error: {:?}). Running direct value checks:", args.ident, e);
        let target_graph = successful_graphs
            .iter()
            .find(|pg| pg.file_path.ends_with(args.relative_file_path))
            .unwrap_or_else(|| panic!("Target graph '{}' not found for value checks after PID generation failure for '{}'.", args.relative_file_path, args.ident));

        let _found = exp_const.find_node_by_values(target_graph).count();
        let _ = args.check_graph(target_graph);
    })?;

    // Find the node using the generated ID within the correct graph
    let node = test_info
        .target_data() // This is &ParsedCodeGraph
        .find_node_unique(test_info.test_pid().into()) // Uses the generated PID
        .inspect_err(|e| {
            let target_graph = test_info.target_data();
            let _ = args.check_graph(target_graph);
            let count = exp_const.find_node_by_values(target_graph).count();
            log::warn!(target: LOG_TEST_CONST, "Node lookup by PID '{}' failed for '{}', found {} matching values with find_node_by_values (Error: {:?}). Running direct value checks:", test_info.test_pid(), args.ident, count, e);
        })?;

    assert_eq!(
        node.name(), // Use the GraphNode trait method
        args.ident,
        "Mismatch for name field. Expected: '{}', Actual: '{}'",
        args.ident,
        node.name()
    );

    let node = node.as_const().unwrap();
    assert!({
        ![
            exp_const.is_name_match_debug(node),
            exp_const.is_visibility_match_debug(node),
            exp_const.is_attributes_match_debug(node),
            exp_const.is_type_id_match_debug(node),
            exp_const.is_value_match_debug(node),
            exp_const.is_docstring_match_debug(node),
            exp_const.is_tracking_hash_match_debug(node),
            exp_const.is_cfgs_match_debug(node),
        ]
        .contains(&false)
    });
    let expected_const_node = EXPECTED_CONSTS_DATA
        .get("crate::const_static::TOP_LEVEL_BOOL")
        .expect("The specified node was not found in they map of expected const nodes.");

    let macro_found_node = expected_const_node
        .find_node_by_values(test_info.target_data())
        .next()
        .unwrap();
    println!("ConstNode found using new macro: {:#?}", macro_found_node);
    println!("ConstNode found using old methods: {:#?}", node);
    assert!(macro_found_node.id.to_pid() == node.id.to_pid());
    // assert!(expected_const_node.check_all_fields(node));
    Ok(())
}

paranoid_test_name_check!(
    test_value_node_field_name_macro_generated,
    fixture: "fixture_nodes",
    relative_file_path: "src/const_static.rs",
    ident: "TOP_LEVEL_BOOL",
    expected_path: &["crate", "const_static"],
    item_kind: ItemKind::Const,
    expected_cfg: None
);

paranoid_test_fields_and_values!(
    test_top_level_bool_fields_and_values,
    "crate::const_static::TOP_LEVEL_BOOL",   // args_key
    EXPECTED_CONSTS_ARGS,                    // args_map
    EXPECTED_CONSTS_DATA,                    // expected_data_map
    crate::parser::nodes::ConstNode,         // node_type
    crate::parser::nodes::consts::ExpectedConstNode, // Corrected path
    as_const,                                // downcast_method
    LOG_TEST_CONST                           // log_target
);

paranoid_test_fields_and_values!(
    test_top_level_int_fields_and_values,
    "crate::const_static::TOP_LEVEL_INT",
    EXPECTED_CONSTS_ARGS,
    EXPECTED_CONSTS_DATA,
    crate::parser::nodes::ConstNode,
    crate::parser::nodes::consts::ExpectedConstNode, // Corrected path
    as_const,
    LOG_TEST_CONST
);

paranoid_test_fields_and_values!(
    test_doc_attr_const_fields_and_values,
    "crate::const_static::doc_attr_const",
    EXPECTED_CONSTS_ARGS,
    EXPECTED_CONSTS_DATA,
    crate::parser::nodes::ConstNode,
    crate::parser::nodes::consts::ExpectedConstNode, // Corrected path
    as_const,
    LOG_TEST_CONST
);

paranoid_test_fields_and_values!(
    test_inner_const_fields_and_values,
    "crate::const_static::inner_mod::INNER_CONST",
    EXPECTED_CONSTS_ARGS,
    EXPECTED_CONSTS_DATA,
    crate::parser::nodes::ConstNode,
    crate::parser::nodes::consts::ExpectedConstNode, // Corrected path
    as_const,
    LOG_TEST_CONST
);

paranoid_test_fields_and_values!(
    test_array_const_fields_and_values,
    "crate::const_static::ARRAY_CONST",
    EXPECTED_CONSTS_ARGS,
    EXPECTED_CONSTS_DATA,
    crate::parser::nodes::ConstNode,
    crate::parser::nodes::consts::ExpectedConstNode, // Corrected path
    as_const,
    LOG_TEST_CONST
);

paranoid_test_fields_and_values!(
    test_struct_const_fields_and_values,
    "crate::const_static::STRUCT_CONST",
    EXPECTED_CONSTS_ARGS,
    EXPECTED_CONSTS_DATA,
    crate::parser::nodes::ConstNode,
    crate::parser::nodes::consts::ExpectedConstNode, // Corrected path
    as_const,
    LOG_TEST_CONST
);

paranoid_test_fields_and_values!(
    test_aliased_const_fields_and_values,
    "crate::const_static::ALIASED_CONST",
    EXPECTED_CONSTS_ARGS,
    EXPECTED_CONSTS_DATA,
    crate::parser::nodes::ConstNode,
    crate::parser::nodes::consts::ExpectedConstNode, // Corrected path
    as_const,
    LOG_TEST_CONST
);

paranoid_test_fields_and_values!(
    test_expr_const_fields_and_values,
    "crate::const_static::EXPR_CONST",
    EXPECTED_CONSTS_ARGS,
    EXPECTED_CONSTS_DATA,
    crate::parser::nodes::ConstNode,
    crate::parser::nodes::consts::ExpectedConstNode, // Corrected path
    as_const,
    LOG_TEST_CONST
);

paranoid_test_fields_and_values!(
    test_fn_call_const_fields_and_values,
    "crate::const_static::FN_CALL_CONST",
    EXPECTED_CONSTS_ARGS,
    EXPECTED_CONSTS_DATA,
    crate::parser::nodes::ConstNode,
    crate::parser::nodes::consts::ExpectedConstNode, // Corrected path
    as_const,
    LOG_TEST_CONST
);
