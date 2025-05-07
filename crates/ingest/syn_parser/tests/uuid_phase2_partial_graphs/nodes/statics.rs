#![cfg(test)]
use crate::common::run_phases_and_collect;
use crate::common::ParanoidArgs;
use crate::paranoid_test_fields_and_values;
use lazy_static::lazy_static;
use ploke_core::ItemKind;
use std::collections::HashMap;
use syn_parser::error::SynParserError;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::Attribute;
use syn_parser::parser::nodes::ExpectedStaticNode;
use syn_parser::parser::nodes::PrimaryNodeIdTrait;
use syn_parser::parser::types::VisibilityKind;

const LOG_TEST_STATIC: &str = "log_statics_test";

lazy_static! {
    // Map from ident -> ExpectedStaticNode
    static ref EXPECTED_STATICS_DATA: HashMap<&'static str, ExpectedStaticNode> = {
        let mut m = HashMap::new();
        m.insert("TOP_LEVEL_COUNTER", ExpectedStaticNode {
            name: "TOP_LEVEL_COUNTER",
            visibility: VisibilityKind::Public,
            type_id_check: true,
            is_mutable: true,
            value: Some("0"),
            attributes: vec![],
            docstring_contains: Some("A top-level public mutable static counter."),
            tracking_hash_check: true,
            cfgs: vec![],
        });
         m.insert("DOC_ATTR_STATIC", ExpectedStaticNode {
            name: "DOC_ATTR_STATIC",
            visibility: VisibilityKind::Inherited,
            type_id_check: true,
            is_mutable: false,
            value: Some("\"Linux specific\""), // Correct value from fixture
            attributes: vec![], // cfg is handled separately
            docstring_contains: Some("his is a documented static variable."), // AI: I removed the
            // `T` to see if the test would fail.
            tracking_hash_check: true,
            // Note: cfg string includes quotes as parsed by syn
            cfgs: vec!["target_os = \"linux\"".to_string()], // Store expected cfgs here
        });
        m.insert("INNER_MUT_STATIC", ExpectedStaticNode {
            name: "INNER_MUT_STATIC",
            visibility: VisibilityKind::Restricted(vec!["super".to_string()]),
            type_id_check: true,
            is_mutable: true,
            value: Some("false"),
            attributes: vec![
                 Attribute {name:"allow".to_string(),args:vec!["dead_code".to_string()],value:None },
            ],
            docstring_contains: None,
            tracking_hash_check: true,
            cfgs: vec![],
        });
        // Add more static examples if needed
        m
    };
}

lazy_static! {
    static ref EXPECTED_STATICS_ARGS: HashMap<&'static str, ParanoidArgs<'static>> = {
        let mut m = HashMap::new();
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

        m.insert("crate::const_static::TUPLE_STATIC", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "TUPLE_STATIC",
            expected_cfg: None,
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Static,
        });
        m.insert("crate::const_static::DOC_ATTR_STATIC", ParanoidArgs {
            fixture: "fixture_nodes",
            relative_file_path: "src/const_static.rs",
            ident: "DOC_ATTR_STATIC",
            expected_cfg: Some(&["target_os = \"linux\""]), // This one has a CFG
            expected_path: &["crate", "const_static"],
            item_kind: ItemKind::Static,
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

#[test]
fn test_static_nodes() -> Result<(), SynParserError> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .try_init();
    // Original was Result<()> which is FixtureError
    // Collect successful graphs
    let successful_graphs = run_phases_and_collect("fixture_nodes");

    // Use ParanoidArgs to find the node
    let args_key = "crate::const_static::TOP_LEVEL_COUNTER";
    let args = EXPECTED_STATICS_ARGS.get(args_key).unwrap_or_else(|| {
        panic!("ParanoidArgs not found for key: {}", args_key);
    });
    let exp_const = EXPECTED_STATICS_DATA.get(args.ident).unwrap();

    // Generate the expected PrimaryNodeId using the method on ParanoidArgs
    let test_info = args.generate_pid(&successful_graphs).inspect_err(|e| {
        log::warn!(target: LOG_TEST_STATIC, "PID generation failed for '{}' (Error: {:?}). Running direct value checks:", args.ident, e);
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
            log::warn!(target: LOG_TEST_STATIC, "Node lookup by PID '{}' failed for '{}', found {} matching values with find_node_by_values (Error: {:?}). Running direct value checks:", test_info.test_pid(), args.ident, count, e);
        })?;

    assert_eq!(
        node.name(), // Use the GraphNode trait method
        args.ident,
        "Mismatch for name field. Expected: '{}', Actual: '{}'",
        args.ident,
        node.name()
    );

    let node = node.as_static().unwrap();
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
    let expected_const_node = EXPECTED_STATICS_DATA
        .get("TOP_LEVEL_COUNTER")
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

paranoid_test_fields_and_values!(
    test_top_level_counter_fields_and_values,
    "crate::const_static::TOP_LEVEL_COUNTER",
    EXPECTED_STATICS_ARGS,
    EXPECTED_STATICS_DATA,
    syn_parser::parser::nodes::StaticNode,
    syn_parser::parser::nodes::ExpectedStaticNode,
    as_static,
    LOG_TEST_STATIC
);

paranoid_test_fields_and_values!(
    test_doc_attr_static_fields_and_values,
    "crate::const_static::DOC_ATTR_STATIC",
    EXPECTED_STATICS_ARGS,
    EXPECTED_STATICS_DATA,
    syn_parser::parser::nodes::StaticNode,
    syn_parser::parser::nodes::ExpectedStaticNode,
    as_static,
    LOG_TEST_STATIC
);

paranoid_test_fields_and_values!(
    test_inner_mut_static_fields_and_values,
    "crate::const_static::inner_mod::INNER_MUT_STATIC",
    EXPECTED_STATICS_ARGS,
    EXPECTED_STATICS_DATA,
    syn_parser::parser::nodes::StaticNode,
    syn_parser::parser::nodes::ExpectedStaticNode,
    as_static,
    LOG_TEST_STATIC
);
