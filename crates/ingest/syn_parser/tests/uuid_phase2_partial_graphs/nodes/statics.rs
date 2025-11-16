//! Tests for `StaticNode` parsing and field extraction.
//!
//! ## Test Coverage Analysis
//!
//! This analysis evaluates the coverage of `StaticNode`s based on the
//! `tests/fixture_crates/fixture_nodes/src/const_static.rs` fixture and
//! variations in `StaticNode` properties.
//!
//! ### 1. Coverage of `static` items from the `const_static.rs` fixture:
//!
//! **Fixture `static` items:**
//! *   `TOP_LEVEL_STR: &str`
//! *   `TOP_LEVEL_COUNTER: u32` (mutable)
//! *   `TOP_LEVEL_CRATE_STATIC: &str` (pub(crate))
//! *   `TUPLE_STATIC: (i32, bool)`
//! *   `DOC_ATTR_STATIC: &str` (with cfg and doc)
//! *   `inner_mod::INNER_MUT_STATIC: bool` (mutable, pub(super))
//!
//! **Coverage Status:**
//! *   **Covered by `EXPECTED_STATICS_DATA` and `paranoid_test_fields_and_values!` macro:**
//!     *   `TOP_LEVEL_COUNTER` (tested by `test_top_level_counter_fields_and_values`)
//!     *   `DOC_ATTR_STATIC` (tested by `test_doc_attr_static_fields_and_values`)
//!     *   `inner_mod::INNER_MUT_STATIC` (tested by `test_inner_mut_static_fields_and_values`)
//! *   **Present in `EXPECTED_STATICS_ARGS` but NOT in `EXPECTED_STATICS_DATA` or tested by `paranoid_test_fields_and_values!`:**
//!     *   `crate::const_static::TOP_LEVEL_STR`
//!     *   `crate::const_static::TOP_LEVEL_CRATE_STATIC`
//!     *   `crate::const_static::TUPLE_STATIC`
//!
//! **Conclusion for Fixture Coverage:**
//! Out of the 6 top-level or inner-module `static` items in the fixture:
//! *   3 are fully tested with detailed field checks.
//! *   3 have `ParanoidArgs` defined but lack detailed field check tests.
//!
//! ### 2. Coverage of `StaticNode` Property Variations:
//!
//! Based on the items covered by `paranoid_test_fields_and_values!`:
//!
//! *   `id: StaticNodeId`: Implicitly covered.
//! *   `name: String`: Good coverage (different unique names).
//! *   `span: (usize, usize)`: Not directly asserted by value.
//! *   `visibility: VisibilityKind`: Good coverage (`Public`, `Inherited`, `Restricted`). `Crate` visibility is in the fixture but not in a detailed test for `StaticNode`.
//! *   `type_id: TypeId`: Good coverage for ensuring `TypeId` is synthetic.
//! *   `is_mutable: bool`: Excellent coverage (both `true` and `false`).
//! *   `value: Option<String>`: Good coverage for `Some` with different literal types (integer, string, boolean).
//! *   `attributes: Vec<Attribute>`: Fair coverage (empty and simple `#[allow(dead_code)]`). More complex attributes could be added.
//! *   `docstring: Option<String>`: Excellent coverage (`Some` and `None`).
//! *   `tracking_hash: Option<TrackingHash>`: Good coverage for ensuring presence.
//! *   `cfgs: Vec<String>`: Good coverage (no `cfg` and single `cfg`). Multiple `cfg`s on one item are not covered.
//!
//! **Conclusion for Property Variation Coverage:**
//! Most `StaticNode` fields have good to excellent coverage.
//! *   **Areas for potential expansion:** Testing `VisibilityKind::Crate`, more complex attributes, and multiple `cfg`s on a single static item.
//!
//! ## Differences in Testing `StaticNode` vs. Other Nodes
//!
//! Testing `StaticNode`s shares many similarities with testing other item nodes like `ConstNode`,
//! particularly in checking common fields such as `name`, `visibility`, `type_id`, `attributes`,
//! `docstring`, `cfgs`, and `tracking_hash`. The `paranoid_test_fields_and_values!` macro
//! framework is designed to be generic enough to handle these commonalities.
//!
//! However, `StaticNode`s have specific characteristics that differentiate their testing:
//!
//! 1.  **`is_mutable: bool` Field:**
//!     This field is unique to `StaticNode` (among `ConstNode` and `StaticNode`). Tests for
//!     `StaticNode` must explicitly verify the correctness of this boolean flag, ensuring
//!     that `static` items are correctly identified as mutable (`static mut`) or immutable.
//!     The `ExpectedStaticNode` data structure includes an `is_mutable` field, and the
//!     `derive(ExpectedData)` macro generates the corresponding `is_is_mutable_match_debug`
//!     check.
//!
//! 2.  **`value: Option<String>` Field:**
//!     While `ConstNode` also has a `value` field, the nature of expressions allowed for
//!     `static` items can be slightly different. Static items require constant initializers,
//!     but they don't have the same compile-time evaluation constraints as `const` items
//!     (e.g., `static` initializers can involve non-`const fn` calls if the result is known
//!     at link time, though `syn` parsing primarily captures the literal expression).
//!     The tests focus on ensuring the parsed string representation of the initializer is correct.
//!
//! 3.  **Associated Statics:**
//!     Unlike `const` items, `static` items cannot be directly associated with `impl` blocks
//!     (i.e., `impl Foo { static BAR: i32 = 0; }` is not allowed). They can appear in traits
//!     as associated items, but this is less common than associated constants. The current
//!     fixture `const_static.rs` does not include associated statics, so this aspect is not
//!     yet specifically tested for `StaticNode`s. If such patterns were to be supported and
//!     parsed, specific tests would be needed.
//!
//! In summary, while the overall testing strategy is consistent, `StaticNode` tests place
//! particular emphasis on the `is_mutable` flag and ensure that the parsing of their
//! initializers is handled correctly, accommodating the range of expressions valid for
//! static variable declarations.

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
        m.insert("crate::const_static::TOP_LEVEL_COUNTER", ExpectedStaticNode {
            name: "TOP_LEVEL_COUNTER",
            visibility: VisibilityKind::Public,
            type_id_check: true,
            is_mutable: true,
            value: Some("0"),
            attributes: vec![],
            docstring: Some("A top-level public mutable static counter."),
            tracking_hash_check: true,
            cfgs: vec![],
        });
         m.insert("crate::const_static::DOC_ATTR_STATIC", ExpectedStaticNode {
            name: "DOC_ATTR_STATIC",
            visibility: VisibilityKind::Inherited,
            type_id_check: true,
            is_mutable: false,
            value: Some("\"Linux specific\""), // Correct value from fixture
            attributes: vec![], // cfg is handled separately
            docstring: Some("This is a documented static variable.\n\nThis variable is specifically configured for Linux targets and contains a\nstring that describes its platform-specific behavior.\n\n# Test Edit\nThis comment was added by the AI assistant for testing purposes."),
            tracking_hash_check: true,
            // Note: cfg string includes quotes as parsed by syn
            cfgs: vec!["target_os = \"linux\"".to_string()], // Store expected cfgs here
        });
        m.insert("crate::const_static::inner_mod::INNER_MUT_STATIC", ExpectedStaticNode {
            name: "INNER_MUT_STATIC",
            visibility: VisibilityKind::Restricted(vec!["super".to_string()]),
            type_id_check: true,
            is_mutable: true,
            value: Some("false"),
            attributes: vec![
                 Attribute {name:"allow".to_string(),args:vec!["dead_code".to_string()],value:None },
            ],
            docstring: None,
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
    let expected_key = "crate::const_static::TOP_LEVEL_COUNTER";
    let args = EXPECTED_STATICS_ARGS.get(expected_key).unwrap_or_else(|| {
        panic!("ParanoidArgs not found for key: {}", expected_key);
    });
    let exp_static = EXPECTED_STATICS_DATA.get(expected_key).unwrap();

    // Generate the expected PrimaryNodeId using the method on ParanoidArgs
    let test_info = args.generate_pid(&successful_graphs).inspect_err(|e| {
        log::warn!(target: LOG_TEST_STATIC, "PID generation failed for '{}' (Error: {:?}). Running direct value checks:", args.ident, e);
        let target_graph = successful_graphs
            .iter()
            .find(|pg| pg.file_path.ends_with(args.relative_file_path))
            .unwrap_or_else(|| panic!("Target graph '{}' not found for value checks after PID generation failure for '{}'.", args.relative_file_path, args.ident));

        let _found = exp_static.find_node_by_values(target_graph).count();
        let _ = args.check_graph(target_graph);
    })?;

    // Find the node using the generated ID within the correct graph
    let node = test_info
        .target_data() // This is &ParsedCodeGraph
        .find_node_unique(test_info.test_pid().into()) // Uses the generated PID
        .inspect_err(|e| {
            let target_graph = test_info.target_data();
            let _ = args.check_graph(target_graph);
            let count = exp_static.find_node_by_values(target_graph).count();
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
            exp_static.is_name_match_debug(node),
            exp_static.is_visibility_match_debug(node),
            exp_static.is_attributes_match_debug(node),
            exp_static.is_type_id_match_debug(node),
            exp_static.is_value_match_debug(node),
            exp_static.is_docstring_match_debug(node),
            exp_static.is_tracking_hash_match_debug(node),
            exp_static.is_cfgs_match_debug(node),
        ]
        .contains(&false)
    });
    let expected_static_node = EXPECTED_STATICS_DATA
        .get("crate::const_static::TOP_LEVEL_COUNTER")
        .expect("The specified node was not found in they map of expected static nodes.");

    let mut node_matches_iter = expected_static_node
        .find_node_by_values(test_info.target_data())
        .filter(|stat| stat.id.to_pid() == node.id.to_pid());
    let macro_found_node = node_matches_iter.next().unwrap();
    println!(
        "FucntionNode found using new macro: {:#?}",
        macro_found_node
    );
    println!("StaticNode found using old methods: {:#?}", node);
    assert!(macro_found_node.id.to_pid() == node.id.to_pid());
    for dup in node_matches_iter {
        assert!(
            node.id.to_pid() != dup.id.to_pid(),
            "Duplicate StaticNodeId found"
        );
        log::warn!(target: LOG_TEST_STATIC,
            "{}: {}\n{}\n\t{}\n\t{} {}\n\t{}",
            "Duplicate values on different path: ",
            "",
            "Two targets were found with matching values.",
            "This indicates that there were duplicate statics at different path locations.",
            "That is fine, so long as you expected to find a duplicate static with the same",
            "name, vis, attrs, docstring, trackinghash, and cfgs in two different files.",
            "If you are seeing this check it means their Ids were correctly not duplicates."
        );
    }
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
