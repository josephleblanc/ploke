#![cfg(test)]
use crate::common::gen_pid_paranoid;
use crate::common::run_phase1_phase2;
use crate::common::run_phases_and_collect;
use syn_parser::parser::nodes::HasAttributes;
use syn_parser::utils::LogStyle;
use syn_parser::utils::LogStyleDebug;
// Use re-exports from paranoid mod
use crate::common::FixtureError;
use crate::common::ParanoidArgs;
use crate::paranoid_test_name_check;
use lazy_static::lazy_static;
use ploke_common::fixtures_crates_dir;
use ploke_core::ItemKind;
use ploke_core::NodeId;
use ploke_core::TypeKind;
use ploke_core::{TrackingHash, TypeId};
use std::collections::HashMap;
use syn_parser::error::SynParserError;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::Attribute;
use syn_parser::parser::nodes::ConstNode;
use syn_parser::parser::nodes::GraphNode;
use syn_parser::parser::types::VisibilityKind;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::TestIds; // Import specific nodes and Attribute
pub const LOG_TEST_CONST: &str = "log_test_const";
// Struct to hold expected fields for a ConstNode
#[derive(Debug, Clone, PartialEq)]
struct ExpectedConstData {
    name: &'static str,
    visibility: VisibilityKind,
    type_id_check: bool, // Just check if it's Synthetic for now
    value: Option<&'static str>,
    attributes: Vec<Attribute>, // Store expected non-cfg attributes
    docstring_contains: Option<&'static str>,
    tracking_hash_check: bool, // Check if Some
    cfgs: Vec<String>,
}

impl ExpectedConstData {
    pub fn find_node_by_values<'a>(
        &'a self,
        parsed: &'a ParsedCodeGraph,
    ) -> impl Iterator<Item = &ConstNode> {
        parsed
            .graph
            .consts
            .iter()
            .filter(move |&n| self.is_name_match_debug(n))
            .filter(move |n| self.is_vis_match_debug(n))
            .filter(move |n| self.is_attr_match_debug(n))
            .filter(move |n| self.is_type_id_check_match_debug(n))
            .filter(move |n| self.is_value_match_debug(n))
            .filter(move |n| self.is_docstring_contains_match_debug(n))
            .filter(move |n| self.is_tracking_hash_check_match_debug(n))
            .filter(move |n| self.is_cfgs_match_debug(n))
    }

    fn is_name_match_debug(&self, node: &ConstNode) -> bool {
        log::debug!(target: LOG_TEST_CONST,
            "   {} | Expected '{}' == Actual '{}': {}",
            "Names Match?".to_string().log_step(),
            self.name.log_name(),
            node.name().log_name(), // Use GraphNode::name() which returns &str
            (self.name == node.name()).to_string().log_vis() // Compare names and log boolean result
        );
        self.name == node.name()
    }
    fn is_vis_match_debug(&self, node: &ConstNode) -> bool {
        log::debug!(target: LOG_TEST_CONST,
            "   {} | Expected '{}' == Actual '{}': {}",
            "Names Match?".to_string().log_step(),
            self.visibility.log_vis_debug(),
            node.visibility().log_vis_debug(), // Use GraphNode::name() which returns &str
            (self.visibility == *node.visibility()).to_string().log_vis() // Compare names and log boolean result
        );
        self.visibility == *node.visibility()
    }
    fn is_attr_match_debug(&self, node: &ConstNode) -> bool {
        log::debug!(target: LOG_TEST_CONST,
            "   {} | Expected '{}' == Actual '{}': {}",
            "Names Match?".to_string().log_step(),
            self.attributes.log_green_debug(),
            node.attributes().log_green_debug(), // Use GraphNode::name() which returns &str
            (self.attributes== node.attributes()).to_string().log_vis() // Compare names and log boolean result
        );
        self.attributes == node.attributes()
    }

    fn is_type_id_check_match_debug(&self, node: &ConstNode) -> bool {
        let actual_check_passes = matches!(node.type_id, TypeId::Synthetic(_));
        log::debug!(target: LOG_TEST_CONST,
            "   {} | Expected check pass '{}' == Actual check pass '{}': {}",
            "TypeId Check Match?".to_string().log_step(),
            self.type_id_check.to_string().log_name(),
            actual_check_passes.to_string().log_name(),
            (self.type_id_check == actual_check_passes).to_string().log_vis()
        );
        self.type_id_check == actual_check_passes
    }

    fn is_value_match_debug(&self, node: &ConstNode) -> bool {
        let actual_value = node.value.as_deref();
        log::debug!(target: LOG_TEST_CONST,
            "   {} | Expected '{:?}' == Actual '{:?}': {}",
            "Value Match?".to_string().log_step(),
            self.value.log_name_debug(),
            actual_value.log_name_debug(),
            (self.value == actual_value).to_string().log_vis()
        );
        self.value == actual_value
    }

    fn is_docstring_contains_match_debug(&self, node: &ConstNode) -> bool {
        let actual_docstring = node.docstring.as_deref();
        let check_passes = match self.docstring_contains {
            Some(expected_substr) => {
                actual_docstring.map_or(false, |s| s.contains(expected_substr))
            }
            None => actual_docstring.is_none(),
        };
        log::debug!(target: LOG_TEST_CONST,
            "   {} | Expected contains '{:?}' in Actual '{:?}': {}",
            "Docstring Contains Match?".to_string().log_step(),
            self.docstring_contains.log_name_debug(),
            actual_docstring.log_name_debug(),
            check_passes.to_string().log_vis()
        );
        check_passes
    }

    fn is_tracking_hash_check_match_debug(&self, node: &ConstNode) -> bool {
        let actual_check_passes = matches!(node.tracking_hash, Some(TrackingHash(_)));
        log::debug!(target: LOG_TEST_CONST,
            "   {} | Expected check pass '{}' == Actual check pass '{}': {}",
            "TrackingHash Check Match?".to_string().log_step(),
            self.tracking_hash_check.to_string().log_name(),
            actual_check_passes.to_string().log_name(),
            (self.tracking_hash_check == actual_check_passes).to_string().log_vis()
        );
        self.tracking_hash_check == actual_check_passes
    }

    fn is_cfgs_match_debug(&self, node: &ConstNode) -> bool {
        // For CFGs, order doesn't matter, so we sort before comparison for stability.
        let mut actual_cfgs = node.cfgs().to_vec();
        actual_cfgs.sort_unstable();
        let mut expected_cfgs_sorted = self.cfgs.clone();
        expected_cfgs_sorted.sort_unstable();

        log::debug!(target: LOG_TEST_CONST,
            "   {} | Expected (sorted) '{:?}' == Actual (sorted) '{:?}': {}",
            "CFGs Match?".to_string().log_step(),
            expected_cfgs_sorted.log_green_debug(),
            actual_cfgs.log_green_debug(),
            (expected_cfgs_sorted == actual_cfgs).to_string().log_vis()
        );
        expected_cfgs_sorted == actual_cfgs
    }
}

// Struct to hold expected fields for a StaticNode
#[derive(Debug, Clone, PartialEq)]
struct ExpectedStaticData {
    name: &'static str,
    visibility: VisibilityKind,
    type_id_check: bool,
    is_mutable: bool,
    value: Option<&'static str>,
    attributes: Vec<Attribute>,
    docstring_contains: Option<&'static str>,
    tracking_hash_check: bool,
    cfgs: Vec<String>,
}

lazy_static! {
    // Map from ident -> ExpectedConstData
    static ref EXPECTED_CONSTS_DATA: HashMap<&'static str, ExpectedConstData> = {
        let mut m = HashMap::new();
        m.insert("TOP_LEVEL_INT", ExpectedConstData {
            name: "TOP_LEVEL_INT",
            visibility: VisibilityKind::Inherited,
            type_id_check: true,
            value: Some("10"),
            attributes: vec![],
            docstring_contains: Some("top-level private constant"),
            tracking_hash_check: true,
            cfgs: vec![],
        });
        m.insert("doc_attr_const", ExpectedConstData {
            name: "doc_attr_const",
            visibility: VisibilityKind::Public,
            type_id_check: true,
            value: Some("3.14"),
            attributes: vec![
                // Note: Arg parsing might simplify this in the future
                Attribute {name:"deprecated".to_string(),args:vec!["note = \"Use NEW_DOC_ATTR_CONST instead\"".to_string()],value:None },
                Attribute {name:"allow".to_string(),args:vec!["unused".to_string()],value:None },
            ],
            docstring_contains: Some("This is a documented constant."),
            tracking_hash_check: true,
            cfgs: vec![],
        });
        // Add more const examples if needed
        m
    };

    // Map from ident -> ExpectedStaticData
    static ref EXPECTED_STATICS_DATA: HashMap<&'static str, ExpectedStaticData> = {
        let mut m = HashMap::new();
        m.insert("TOP_LEVEL_COUNTER", ExpectedStaticData {
            name: "TOP_LEVEL_COUNTER",
            visibility: VisibilityKind::Public,
            type_id_check: true,
            is_mutable: true,
            value: Some("0"),
            attributes: vec![],
            docstring_contains: None,
            tracking_hash_check: true,
            cfgs: vec![],
        });
         m.insert("DOC_ATTR_STATIC", ExpectedStaticData {
            name: "DOC_ATTR_STATIC",
            visibility: VisibilityKind::Inherited,
            type_id_check: true,
            is_mutable: false,
            value: Some("true"),
            attributes: vec![], // cfg is handled separately
            docstring_contains: Some("A static variable with a doc comment and attribute."),
            tracking_hash_check: true,
            // Note: cfg string includes quotes as parsed by syn
            cfgs: vec!["target_os = \"linux\"".to_string()], // Store expected cfgs here
        });
        m.insert("INNER_MUT_STATIC", ExpectedStaticData {
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

// Previous versions.
// Define expected items: (name, kind, is_mutable, visibility_check)
// Visibility check is basic: true if it should NOT be Inherited
// let expected_consts = vec![
//     ("TOP_LEVEL_INT", ItemKind::Const, false), // private
//     ("TOP_LEVEL_BOOL", ItemKind::Const, true), // pub
//     ("INNER_CONST", ItemKind::Const, true),    // pub(crate) (in mod)
//     ("ARRAY_CONST", ItemKind::Const, false),   // private
//     ("STRUCT_CONST", ItemKind::Const, false),  // private
//     ("ALIASED_CONST", ItemKind::Const, false), // private
//     ("EXPR_CONST", ItemKind::Const, false),    // private
//     ("FN_CALL_CONST", ItemKind::Const, false), // private
//     ("doc_attr_const", ItemKind::Const, true), // pub
// ];
// let expected_statics = vec![
//     ("TOP_LEVEL_STR", ItemKind::Static, false), // private
//     ("TOP_LEVEL_COUNTER", ItemKind::Static, true, true), // pub
//     ("TOP_LEVEL_CRATE_STATIC", ItemKind::Static, false, true), // pub(crate)
//     ("TUPLE_STATIC", ItemKind::Static, false, false), // private
//     ("DOC_ATTR_STATIC", ItemKind::Static, false, false), // private
//     ("INNER_MUT_STATIC", ItemKind::Static, true, true), // pub(super) (in mod)
// ];
// ("IMPL_CONST", ItemKind::Const, true),     // Ignored: Limitation - Associated const in impl not parsed (see 02c_phase2_known_limitations.md)
// ("TRAIT_REQ_CONST", ItemKind::Const, false), // Ignored: Limitation - Associated const in trait impl not parsed (see 02c_phase2_known_limitations.md)

// Define the static array using ParanoidArgs
static EXPECTED_ITEMS: &[ParanoidArgs] = &[
    // --- Top Level Items ---
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "TOP_LEVEL_INT",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Const,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "TOP_LEVEL_BOOL",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Const,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "TOP_LEVEL_STR",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Static,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "TOP_LEVEL_COUNTER",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Static,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "TOP_LEVEL_CRATE_STATIC", // pub(crate)
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Static,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "ARRAY_CONST",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Const,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "TUPLE_STATIC",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Static,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "STRUCT_CONST",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Const,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "ALIASED_CONST",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Const,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "EXPR_CONST",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Const,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "FN_CALL_CONST",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Const,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "doc_attr_const",
        expected_cfg: None, // Attributes are not CFGs
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Const,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "DOC_ATTR_STATIC",
        expected_cfg: Some(&["target_os = \"linux\""]), // This one has a CFG
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Static,
    },
    // --- Inner Mod Items ---
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs", // Defined in this file
        ident: "INNER_CONST",
        expected_cfg: None,
        expected_path: &["crate", "const_static", "inner_mod"], // Path within the file
        item_kind: ItemKind::Const,
    },
    ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs", // Defined in this file
        ident: "INNER_MUT_STATIC",
        expected_cfg: None,
        expected_path: &["crate", "const_static", "inner_mod"], // Path within the file
        item_kind: ItemKind::Static,
    },
];

// Test Plan for ValueNode (const/static) in Phase 2 (uuid_ids)
// ============================================================
// Fixture: tests/fixture_crates/fixture_nodes/src/const_static.rs
// Helper Location: tests/common/paranoid/const_static_helpers.rs

// --- Test Tiers ---

// Tier 1: Basic Smoke Tests
// Goal: Quickly verify that ValueNodes are created for various const/static items
//       and basic properties (IDs, hash, kind) are present. Uses full crate parse.
// ---------------------------------------------------------------------------------
// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_const_static_basic_smoke_test_full_parse()
//  - Use run_phase1_phase2("fixture_nodes")
//  - Find the ParsedCodeGraph for const_static.rs
//  - Iterate through expected const/static names (e.g., TOP_LEVEL_INT, TOP_LEVEL_STR, INNER_CONST)
//  - For each:
//      - Find the ValueNode (e.g., using graph.values.iter().find(|v| v.name == ...))
//      - Assert node exists.
//      - Assert node.id is NodeId::Synthetic(_).
//      - Assert node.tracking_hash is Some(TrackingHash(_)).
//      - Assert node.type_id is TypeId::Synthetic(_).
//      - Assert node.kind() matches (Const or Static { is_mutable }).
//      - Assert node.visibility is not Inherited if it should be something else (basic check).
#[test]
fn basic_smoke_test() -> anyhow::Result<()> {
    let results = run_phase1_phase2("fixture_nodes");
    assert!(!results.is_empty(), "Phase 1 & 2 failed to produce results");

    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");

    let target_data = results
        .iter()
        .find_map(|res| match res {
            Ok(data) if data.file_path == fixture_path => Some(data),
            _ => None,
        })
        .unwrap_or_else(|| panic!("ParsedCodeGraph for '{}' not found", fixture_path.display()));

    let graph = &target_data.graph;

    assert!(!graph.consts.is_empty(), "CodeGraph contains no Consts");
    assert!(!graph.statics.is_empty(), "CodeGraph contains no Statics");

    // Check consts
    for item in EXPECTED_ITEMS
        .iter()
        .filter(|cnst| cnst.item_kind == ItemKind::Const)
    {
        let node = graph
            .consts
            .iter()
            .find(|s| s.name == item.ident)
            .ok_or_else(|| FixtureError::NotFound(item.ident.to_string()))?;

        assert!(
            matches!(node.id.base_tid(), NodeId::Synthetic(_)),
            "Node '{}': ID should be Synthetic, found {:?}",
            item.ident,
            node.id
        );
        assert!(
            matches!(node.tracking_hash, Some(TrackingHash(_))),
            "Node '{}': tracking_hash should be Some(TrackingHash), found {:?}",
            item.ident,
            node.tracking_hash
        );
        assert!(
            matches!(node.type_id, TypeId::Synthetic(_)),
            "Node '{}': type_id should be Synthetic, found {:?}",
            item.ident,
            node.type_id
        );
        assert_eq!(
            node.kind(),
            item.item_kind,
            "Node '{}': Kind mismatch. Expected {:?}, found {:?}",
            item.ident,
            item.item_kind,
            node.kind()
        );

        // Remove visibility for now. Possibly move elsewhere.
        // assert_eq!(
        //     node.visibility, item.expected_visibility,
        //     "Node '{}': Expected {:?} visibility, found {:?}",
        //     item.ident, item.expected_visibility, node.visibility
        // );
    }
    // Check consts
    for item in EXPECTED_ITEMS
        .iter()
        .filter(|sttc| sttc.item_kind == ItemKind::Static)
    {
        let node = graph
            .consts
            .iter()
            .find(|s| s.name == item.ident)
            .ok_or_else(|| FixtureError::NotFound(item.ident.to_string()))?;

        assert!(
            matches!(node.id.base_tid(), NodeId::Synthetic(_)),
            "Node '{}': ID should be Synthetic, found {:?}",
            item.ident,
            node.id
        );
        assert!(
            matches!(node.tracking_hash, Some(TrackingHash(_))),
            "Node '{}': tracking_hash should be Some(TrackingHash), found {:?}",
            item.ident,
            node.tracking_hash
        );
        assert!(
            matches!(node.type_id, TypeId::Synthetic(_)),
            "Node '{}': type_id should be Synthetic, found {:?}",
            item.ident,
            node.type_id
        );
        assert_eq!(
            node.kind(),
            item.item_kind,
            "Node '{}': Kind mismatch. Expected {:?}, found {:?}",
            item.ident,
            item.item_kind,
            node.kind()
        );
        // Commenting out for now
        // assert_eq!(
        //     node.visibility, item.expected_visibility,
        //     "Node '{}': Expected {:?} visibility, found {:?}",
        //     item.ident, item.expected_visibility, node.visibility
        // );
    }
    Ok(())
}

// Tier 2: Targeted Field Verification
// Goal: Verify each field of the StaticNode or ConstNode struct individually for specific examples.
//       These tests act as diagnostics if specific fields break later. Use detailed asserts.
//       Uses full parse for consistency.
// ---------------------------------------------------------------------------------
#[test]
// #[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_id_regeneration_and_fields() -> Result<(), SynParserError> {
    // Renamed slightly
    let fixture = "fixture_nodes";
    let file_path_rel = "src/const_static.rs";

    // Use helper to collect graphs, handling errors
    let successful_graphs = run_phases_and_collect(fixture);

    // Find the specific graph for const_static.rs
    let target_data = successful_graphs
        .iter()
        .find(|g| g.file_path.ends_with(file_path_rel)) // Find by relative path suffix
        .ok_or_else(|| {
            SynParserError::TestHelperError(format!(
                "ParsedCodeGraph for fixture '{}' file '{}' not found",
                fixture, file_path_rel
            ))
        })?;
    let graph = &target_data.graph;

    // --- Define Representative Examples ---
    // Choose idents corresponding to keys in the lazy static maps
    let representative_idents = [
        "TOP_LEVEL_INT",
        "doc_attr_const",
        "TOP_LEVEL_COUNTER",
        "DOC_ATTR_STATIC",
        "INNER_MUT_STATIC",
    ];

    for ident in representative_idents {
        println!("Checking fields for: {}", ident);

        // Find the corresponding ParanoidArgs from the static list
        let item_args = EXPECTED_ITEMS
            .iter()
            .find(|i| i.ident == ident)
            .unwrap_or_else(|| panic!("ParanoidArgs not found for ident: {}", ident));

        // 1. Generate the expected ID using the paranoid helper
        let expected_pid = gen_pid_paranoid(item_args.clone(), &successful_graphs)?;

        // 2. Find the node using the generated ID
        let found_node = graph.find_node_unique(expected_pid.into())?;

        // 3. Compare Fields using lazy static data
        match item_args.item_kind {
            ItemKind::Const => {
                let const_node = found_node.as_const().ok_or_else(|| {
                    SynParserError::InternalState(format!(
                        "Node kind mismatch for ID {}: Expected {:?}, found {:?}",
                        found_node.any_id(),
                        item_args.item_kind,
                        found_node.kind()
                    ))
                })?;
                let expected_data = EXPECTED_CONSTS_DATA
                    .get(ident)
                    .unwrap_or_else(|| panic!("Expected const data not found for {}", ident));

                assert_eq!(
                    const_node.name, expected_data.name,
                    "Name mismatch for {}",
                    ident
                );
                assert_eq!(
                    const_node.visibility(),
                    &expected_data.visibility,
                    "Visibility mismatch for {}",
                    ident
                );
                assert_eq!(
                    matches!(const_node.type_id, TypeId::Synthetic(_)),
                    expected_data.type_id_check,
                    "TypeId check failed for {}",
                    ident
                );
                assert_eq!(
                    const_node.value.as_deref(),
                    expected_data.value,
                    "Value mismatch for {}",
                    ident
                );
                // Note: Attribute comparison might need refinement if order matters or args are complex
                assert_eq!(
                    const_node.attributes, expected_data.attributes,
                    "Attributes mismatch for {}",
                    ident
                );
                assert_eq!(
                    const_node
                        .docstring
                        .as_deref()
                        .map(|s| s.contains(expected_data.docstring_contains.unwrap_or("")))
                        .unwrap_or(false),
                    expected_data.docstring_contains.is_some(),
                    "Docstring mismatch for {}",
                    ident
                );
                assert_eq!(
                    matches!(const_node.tracking_hash, Some(TrackingHash(_))),
                    expected_data.tracking_hash_check,
                    "TrackingHash check failed for {}",
                    ident
                );
                // Sort CFGs for comparison
                let mut actual_cfgs = const_node.cfgs().to_vec();
                actual_cfgs.sort_unstable();
                let mut expected_cfgs_sorted = expected_data.cfgs.clone();
                expected_cfgs_sorted.sort_unstable();
                assert_eq!(
                    actual_cfgs, expected_cfgs_sorted,
                    "CFGs mismatch for {}",
                    ident
                );
            }
            ItemKind::Static => {
                let static_node = found_node.as_static().ok_or_else(|| {
                    SynParserError::InternalState(format!(
                        "Node kind mismatch for ID {}: Expected {:?}, found {:?}",
                        found_node.any_id(),
                        item_args.item_kind,
                        found_node.kind()
                    ))
                })?;
                let expected_data = EXPECTED_STATICS_DATA
                    .get(ident)
                    .unwrap_or_else(|| panic!("Expected static data not found for {}", ident));

                assert_eq!(
                    static_node.name, expected_data.name,
                    "Name mismatch for {}",
                    ident
                );
                assert_eq!(
                    static_node.visibility(),
                    &expected_data.visibility,
                    "Visibility mismatch for {}",
                    ident
                );
                assert_eq!(
                    matches!(static_node.type_id, TypeId::Synthetic(_)),
                    expected_data.type_id_check,
                    "TypeId check failed for {}",
                    ident
                );
                assert_eq!(
                    static_node.is_mutable, expected_data.is_mutable,
                    "is_mutable mismatch for {}",
                    ident
                );
                assert_eq!(
                    static_node.value.as_deref(),
                    expected_data.value,
                    "Value mismatch for {}",
                    ident
                );
                assert_eq!(
                    static_node.attributes, expected_data.attributes,
                    "Attributes mismatch for {}",
                    ident
                );
                assert_eq!(
                    static_node
                        .docstring
                        .as_deref()
                        .map(|s| s.contains(expected_data.docstring_contains.unwrap_or("")))
                        .unwrap_or(false),
                    expected_data.docstring_contains.is_some(),
                    "Docstring mismatch for {}",
                    ident
                );
                assert_eq!(
                    matches!(static_node.tracking_hash, Some(TrackingHash(_))),
                    expected_data.tracking_hash_check,
                    "TrackingHash check failed for {}",
                    ident
                );
                // Sort CFGs for comparison
                let mut actual_cfgs = static_node.cfgs().to_vec();
                actual_cfgs.sort_unstable();
                let mut expected_cfgs_sorted = expected_data.cfgs.clone();
                expected_cfgs_sorted.sort_unstable();
                assert_eq!(
                    actual_cfgs, expected_cfgs_sorted,
                    "CFGs mismatch for {}",
                    ident
                );
            }
            _ => panic!("Unexpected item kind in test: {:?}", item_args.item_kind),
        }
    }
    Ok(())
}

// fn test_value_node_field_name()
//  - Target: TOP_LEVEL_BOOL
//  - Find the ValueNode.
//  - Get context: crate_namespace, file_path, module_path (["crate"]), name ("TOP_LEVEL_INT"), span.
//  - Regenerate expected NodeId::Synthetic using NodeId::generate_synthetic.
//  - Assert_eq!(node.id, regenerated_id, "Mismatch for ID field. Expected: {}, Actual: {}", ...);

// fn test_value_node_field_name()
//  - Target: TOP_LEVEL_BOOL
//  - Find the ValueNode.
//  - Assert_eq!(node.name, "TOP_LEVEL_BOOL", "Mismatch for name field. Expected: {}, Actual: {}", ...);

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
    let args = ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: "TOP_LEVEL_BOOL",
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Const, // AI!: Make a new entry in `EXPECTED_ITEMS` for this item. Use
                                    // the contents of the `tests/fixture_crates/fixture_nodes/src/const_static.rs` to ensure
                                    // it is accurate AI!
    };
    let args = EXPECTED_ITEMS
        .iter()
        .find(|fixt| fixt.ident == args.ident)
        .unwrap();
    let exp_const = EXPECTED_CONSTS_DATA.get(args.ident).unwrap();

    // Generate the expected PrimaryNodeId using the method on ParanoidArgs
    let test_info = args.generate_pid(&successful_graphs).inspect_err(|e| {
        let _ = exp_const
            .find_node_by_values(
                &successful_graphs
                    .iter()
                    .find(|pg| pg.file_path.ends_with(args.relative_file_path))
                    .unwrap(),
            )
            .count();
    })?;

    // Find the node using the generated ID within the correct graph
    let node = test_info
        .target_data() // This is &ParsedCodeGraph
        .find_node_unique(test_info.test_pid().into()) // Uses the generated PID
        .inspect_err(|e| {
            let _ = exp_const
                .find_node_by_values(test_info.target_data())
                .count();
        })?;

    assert_eq!(
        node.name(), // Use the GraphNode trait method
        args.ident,
        "Mismatch for name field. Expected: '{}', Actual: '{}'",
        args.ident,
        node.name()
    );
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

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_visibility_public()
//  - Target: TOP_LEVEL_BOOL (pub)
//  - Find the ValueNode.
//  - Assert_eq!(node.visibility, VisibilityKind::Public, "Mismatch for visibility field. Expected: {:?}, Actual: {:?}", ...);
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_visibility_public() {
    // Target: TOP_LEVEL_BOOL (pub)
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "const_static".to_string()]; // Correct path
    let value_name = "TOP_LEVEL_BOOL";
    let expected_visibility = VisibilityKind::Public;

    let node = find_value_node_basic(graph, &module_path, value_name);

    assert_eq!(
        node.visibility, expected_visibility,
        "Mismatch for visibility field on '{}'. Expected: {:?}, Actual: {:?}",
        value_name, expected_visibility, node.visibility
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_visibility_inherited()
//  - Target: TOP_LEVEL_INT (private)
//  - Find the ValueNode.
//  - Assert_eq!(node.visibility, VisibilityKind::Inherited, "Mismatch for visibility field. Expected: {:?}, Actual: {:?}", ...);
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_visibility_inherited() {
    // Target: TOP_LEVEL_INT (private)
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "const_static".to_string()]; // Correct path
    let value_name = "TOP_LEVEL_INT";
    let expected_visibility = VisibilityKind::Inherited;

    let node = find_value_node_basic(graph, &module_path, value_name);

    assert_eq!(
        node.visibility, expected_visibility,
        "Mismatch for visibility field on '{}'. Expected: {:?}, Actual: {:?}",
        value_name, expected_visibility, node.visibility
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_visibility_crate()
//  - Target: INNER_CONST (pub(crate)) -> NOTE: Fixture has this in inner_mod
//  - Find the ValueNode.
//  - Assert_eq!(node.visibility, VisibilityKind::Crate, "Mismatch for visibility field. Expected: {:?}, Actual: {:?}", ...);
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_visibility_crate() {
    // Target: INNER_CONST (pub(crate)) in inner_mod
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    // Path to inner_mod within const_static module
    let module_path = vec![
        "crate".to_string(),
        "const_static".to_string(),
        "inner_mod".to_string(),
    ];
    let value_name = "INNER_CONST";
    // NOTE: Limitation - Expecting Restricted(["crate"]) instead of Crate due to current visitor implementation.
    // See docs/plans/uuid_refactor/02c_phase2_known_limitations.md
    let expected_visibility = VisibilityKind::Restricted(vec!["crate".to_string()]);

    let node = find_value_node_basic(graph, &module_path, value_name);

    assert_eq!(
        node.visibility, expected_visibility,
        "Mismatch for visibility field on '{}'. Expected: {:?}, Actual: {:?}",
        value_name, expected_visibility, node.visibility
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_visibility_super()
//  - Target: INNER_MUT_STATIC (pub(super)) in inner_mod
//  - Find the ValueNode.
//  - Assert_eq!(node.visibility, VisibilityKind::Restricted { path: vec!["super".into()], resolved_path: None }, "Mismatch for visibility field. Expected: {:?}, Actual: {:?}", ...);
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_visibility_super() {
    // Target: INNER_MUT_STATIC (pub(super)) in inner_mod
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    // Path to inner_mod within const_static module
    let module_path = vec![
        "crate".to_string(),
        "const_static".to_string(),
        "inner_mod".to_string(),
    ];
    let value_name = "INNER_MUT_STATIC";
    // Expecting Restricted variant with "super" path
    let expected_visibility = VisibilityKind::Restricted(vec!["super".to_string()]);

    let node = find_value_node_basic(graph, &module_path, value_name);

    assert_eq!(
        node.visibility, expected_visibility,
        "Mismatch for visibility field on '{}'. Expected: {:?}, Actual: {:?}",
        value_name, expected_visibility, node.visibility
    );
}

// fn test_value_node_field_type_id_presence()
//  - Target: ARRAY_CONST
//  - Find the ValueNode.
//  - Assert!(matches!(node.type_id, TypeId::Synthetic(_)), "type_id field should be Synthetic. Actual: {:?}", node.type_id);
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_type_id_presence() {
    // Target: ARRAY_CONST
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "const_static".to_string()]; // Correct path
    let value_name = "ARRAY_CONST";

    let node = find_value_node_basic(graph, &module_path, value_name);

    assert!(
        matches!(node.type_id, TypeId::Synthetic(_)),
        "type_id field for '{}' should be Synthetic. Actual: {:?}",
        value_name,
        node.type_id
    );
}

// fn test_value_node_field_kind_const()
//  - Target: TOP_LEVEL_INT
//  - Find the ValueNode.
//  - Assert_eq!(node.kind(), ItemKind::Const, "Mismatch for kind field. Expected: {:?}, Actual: {:?}", ...);
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_kind_const() {
    // Target: TOP_LEVEL_INT
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "const_static".to_string()]; // Correct path
    let value_name = "TOP_LEVEL_INT";
    let expected_kind = ItemKind::Const;

    let node = find_value_node_basic(graph, &module_path, value_name);

    assert_eq!(
        node.kind(),
        expected_kind,
        "Mismatch for kind field on '{}'. Expected: {:?}, Actual: {:?}",
        value_name,
        expected_kind,
        node.kind()
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_kind_static_imm()
//  - Target: TOP_LEVEL_STR
//  - Find the ValueNode.
//  - Assert_eq!(node.kind(), ItemKind::Static { is_mutable: false }, "Mismatch for kind field. Expected: {:?}, Actual: {:?}", ...);
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_kind_static_imm() {
    // Target: TOP_LEVEL_STR
    let value_name = "TOP_LEVEL_STR";
    let expected_kind = ItemKind::Static;

    // Use ParanoidArgs to find the node
    let args = ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: value_name,
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Static, // Correct kind
    };

    // Collect successful graphs
    let successful_graphs = run_phases_and_collect("fixture_nodes");

    // Generate the expected PrimaryNodeId using the method on ParanoidArgs
    let expected_pid = args
        .gen_pid_paranoid(&successful_graphs)
        .expect("Failed to generate PID for TOP_LEVEL_STR");

    // Find the specific graph for const_static.rs from the successful graphs
    let target_data =
        find_parsed_graph_by_path(&successful_graphs, "fixture_nodes", "src/const_static.rs")
            .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph; // Get the graph from the correct ParsedCodeGraph

    // Find the node using the generated ID within the correct graph
    let node = graph
        .find_node_unique(expected_pid.into()) // Uses the generated PID
        .expect("Failed to find node using generated PID");

    assert_eq!(
        node.kind(), // Use the GraphNode trait method
        expected_kind,
        "Mismatch for kind field on '{}'. Expected: {:?}, Actual: {:?}",
        value_name,
        expected_kind,
        node.kind()
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_kind_static_mut()
//  - Target: TOP_LEVEL_COUNTER
//  - Find the ValueNode.
//  - Assert_eq!(node.kind(), ItemKind::Static { is_mutable: true }, "Mismatch for kind field. Expected: {:?}, Actual: {:?}", ...);
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_kind_static_mut() {
    // Target: TOP_LEVEL_COUNTER
    let value_name = "TOP_LEVEL_COUNTER";
    let expected_kind = ItemKind::Static; // Static kind doesn't store mutability

    // Use ParanoidArgs to find the node
    let args = ParanoidArgs {
        fixture: "fixture_nodes",
        relative_file_path: "src/const_static.rs",
        ident: value_name,
        expected_cfg: None,
        expected_path: &["crate", "const_static"],
        item_kind: ItemKind::Static, // Correct kind
    };

    // Collect successful graphs
    let successful_graphs = run_phases_and_collect("fixture_nodes");

    // Generate the expected PrimaryNodeId using the method on ParanoidArgs
    let expected_pid = args
        .gen_pid_paranoid(&successful_graphs)
        .expect("Failed to generate PID for TOP_LEVEL_COUNTER");

    // Find the specific graph for const_static.rs from the successful graphs
    let target_data =
        find_parsed_graph_by_path(&successful_graphs, "fixture_nodes", "src/const_static.rs")
            .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph; // Get the graph from the correct ParsedCodeGraph

    // Find the node using the generated ID within the correct graph
    let node = graph
        .find_node_unique(expected_pid.into()) // Uses the generated PID
        .expect("Failed to find node using generated PID");

    assert_eq!(
        node.kind(), // Use the GraphNode trait method
        expected_kind,
        "Mismatch for kind field on '{}'. Expected: {:?}, Actual: {:?}",
        value_name,
        expected_kind,
        node.kind()
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_value_string()
//  - Target: TOP_LEVEL_INT (= 10)
//  - Find the ValueNode.
//  - Assert_eq!(node.value.as_deref(), Some("10"), "Mismatch for value field. Expected: {:?}, Actual: {:?}", ...);
//  - Target: EXPR_CONST (= 5 * 2 + 1)
//  - Find the ValueNode.
//  - Assert_eq!(node.value.as_deref(), Some("5 * 2 + 1"), "Mismatch for value field. Expected: {:?}, Actual: {:?}", ...); // Verify expression is captured
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_value_string() {
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "const_static".to_string()]; // Correct path

    // Target 1: TOP_LEVEL_INT (= 10)
    let value_name1 = "TOP_LEVEL_INT";
    let expected_value1 = Some("10");
    let node1 = find_value_node_basic(graph, &module_path, value_name1);
    assert_eq!(
        node1.value.as_deref(),
        expected_value1,
        "Mismatch for value field on '{}'. Expected: {:?}, Actual: {:?}",
        value_name1,
        expected_value1,
        node1.value
    );

    // Target 2: EXPR_CONST (= 5 * 2 + 1)
    let value_name2 = "EXPR_CONST";
    // Note: syn/quote preserves spacing, adjust expected value if fixture formatting changes
    let expected_value2 = Some("5 * 2 + 1");
    let node2 = find_value_node_basic(graph, &module_path, value_name2);
    assert_eq!(
        node2.value.as_deref(),
        expected_value2,
        "Mismatch for value field on '{}'. Expected: {:?}, Actual: {:?}",
        value_name2,
        expected_value2,
        node2.value
    );

    // Target 3: TOP_LEVEL_STR (= "hello world")
    let value_name3 = "TOP_LEVEL_STR";
    let expected_value3 = Some("\"hello world\""); // Expect quotes for string literals
    let node3 = find_value_node_basic(graph, &module_path, value_name3);
    assert_eq!(
        node3.value.as_deref(),
        expected_value3,
        "Mismatch for value field on '{}'. Expected: {:?}, Actual: {:?}",
        value_name3,
        expected_value3,
        node3.value
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_attributes_single()
//  - Target: DOC_ATTR_STATIC (#[cfg(target_os = "linux")])
//  - Find the ValueNode.
//  - Assert_eq!(node.attributes.len(), 1, "Expected 1 attribute, found {}. Attrs: {:?}", node.attributes.len(), node.attributes);
//  - Assert_eq!(node.attributes[0].name, "cfg");
//  - Assert!(node.attributes[0].args.contains(&"target_os = \"linux\"".to_string())); // Check specific arg if possible
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_attributes_single() {
    // Target: DOC_ATTR_STATIC (#[cfg(target_os = "linux")])
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "const_static".to_string()]; // Correct path
    let value_name = "DOC_ATTR_STATIC";

    let node = find_value_node_basic(graph, &module_path, value_name);

    // Assert that the `cfgs` field contains the expected string
    assert_eq!(
        node.cfgs.len(),
        1,
        "Node '{}': Expected 1 cfg string, found {}. Cfgs: {:?}",
        value_name,
        node.cfgs.len(),
        node.cfgs
    );
    // Check the content of the cfg string (whitespace might be normalized)
    let expected_cfg = "target_os = \"linux\"";
    assert!(
        node.cfgs[0].contains("target_os") && node.cfgs[0].contains("linux"),
        "Node '{}': CFG string mismatch. Expected contains '{}', Actual: '{}'",
        value_name,
        expected_cfg,
        node.cfgs[0]
    );
    // Also assert that the main attributes list is now empty for this node
    assert!(
        node.attributes.is_empty(),
        "Node '{}': Expected attributes list to be empty after filtering cfg, found: {:?}",
        value_name,
        node.attributes
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_attributes_multiple()
//  - Target: doc_attr_const (#[deprecated(...)], #[allow(...)])
//  - Find the ValueNode.
//  - Assert_eq!(node.attributes.len(), 2, "Expected 2 attributes, found {}. Attrs: {:?}", node.attributes.len(), node.attributes);
//  - Assert!(node.attributes.iter().any(|a| a.name == "deprecated"), "Missing 'deprecated' attribute");
//  - Assert!(node.attributes.iter().any(|a| a.name == "allow"), "Missing 'allow' attribute");
//  - // Maybe check specific args for one of them
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_attributes_multiple() {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .try_init();
    // Target: doc_attr_const (#[deprecated(...)], #[allow(...)])
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "const_static".to_string()]; // Correct path
    let value_name = "doc_attr_const";

    let node = find_value_node_basic(graph, &module_path, value_name);

    assert_eq!(
        node.attributes.len(),
        2,
        "Node '{}': Expected 2 attributes, found {}. Attrs: {:?}",
        value_name,
        node.attributes.len(),
        node.attributes
    );

    // Check for presence of specific attribute names
    let has_deprecated = node.attributes.iter().any(|a| a.name == "deprecated");
    let has_allow = node.attributes.iter().any(|a| a.name == "allow");

    assert!(
        has_deprecated,
        "Node '{}': Missing 'deprecated' attribute. Attrs: {:?}",
        value_name, node.attributes
    );
    assert!(
        has_allow,
        "Node '{}': Missing 'allow' attribute. Attrs: {:?}",
        value_name, node.attributes
    );

    // Optional: Check args for one attribute (e.g., deprecated)
    let deprecated_attr = node
        .attributes
        .iter()
        .find(|a| a.name == "deprecated")
        .unwrap(); // Safe unwrap due to check above
    let args_string = deprecated_attr.args.join(", ");
    assert!(
        args_string.contains("note") && args_string.contains("NEW_DOC_ATTR_CONST"),
        "Node '{}': Args for 'deprecated' mismatch. Expected contains 'note = \"...\"', found args: {:?}",
        value_name, deprecated_attr.args
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_docstring()
//  - Target: TOP_LEVEL_INT ("A top-level private constant...")
//  - Find the ValueNode.
//  - Assert!(node.docstring.is_some(), "Expected docstring, found None");
//  - Assert!(node.docstring.as_deref().unwrap_or("").contains("top-level private constant"), "Docstring mismatch. Expected contains: '{}', Actual: {:?}", "...", node.docstring);
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_docstring() {
    // Target: TOP_LEVEL_INT ("A top-level private constant...")
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "const_static".to_string()]; // Correct path
    let value_name = "TOP_LEVEL_INT";
    let expected_substring = "top-level private constant";

    let node = find_value_node_basic(graph, &module_path, value_name);

    assert!(
        node.docstring.is_some(),
        "Node '{}': Expected docstring, found None",
        value_name
    );

    let doc = node.docstring.as_deref().unwrap_or("");
    assert!(
        doc.contains(expected_substring),
        "Node '{}': Docstring mismatch. Expected contains: '{}', Actual: {:?}",
        value_name,
        expected_substring,
        node.docstring
    );

    // Target 2: TOP_LEVEL_STR (no doc comment)
    let value_name_no_doc = "TOP_LEVEL_STR";
    let node_no_doc = find_value_node_basic(graph, &module_path, value_name_no_doc);
    assert!(
        node_no_doc.docstring.is_none(),
        "Node '{}': Expected no docstring, found Some({:?})",
        value_name_no_doc,
        node_no_doc.docstring
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_field_tracking_hash_presence()
//  - Target: ALIASED_CONST
//  - Find the ValueNode.
//  - Assert!(node.tracking_hash.is_some(), "tracking_hash field should be Some. Actual: {:?}", node.tracking_hash);
//  - Assert!(matches!(node.tracking_hash, Some(TrackingHash(_))), "tracking_hash should contain a Uuid");
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_field_tracking_hash_presence() {
    // Target: ALIASED_CONST
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let module_path = vec!["crate".to_string(), "const_static".to_string()]; // Correct path
    let value_name = "ALIASED_CONST";

    let node = find_value_node_basic(graph, &module_path, value_name);

    assert!(
        node.tracking_hash.is_some(),
        "Node '{}': tracking_hash field should be Some. Actual: {:?}",
        value_name,
        node.tracking_hash
    );
    assert!(
        matches!(node.tracking_hash, Some(TrackingHash(_))),
        "Node '{}': tracking_hash should contain a Uuid. Actual: {:?}",
        value_name,
        node.tracking_hash
    );
}

// Tier 3: Subfield Variations
// Goal: Verify specific variations within complex fields like `visibility` and `kind`.
//       These might overlap with Tier 2 but ensure explicit coverage of variants.
// ---------------------------------------------------------------------------------
// (Covered by Tier 2 tests for visibility and kind variants)

// Tier 4: Basic Connection Tests
// Goal: Verify the `Contains` relationship between modules and ValueNodes.
// ---------------------------------------------------------------------------------
// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_relation_contains_file_module()
//  - Target: TOP_LEVEL_INT in "crate" module (const_static.rs)
//  - Use full parse: run_phase1_phase2("fixture_nodes")
//  - Find ParsedCodeGraph for const_static.rs.
//  - Find crate module node (file-level root) using find_file_module_node_paranoid.
//  - Find ValueNode for TOP_LEVEL_INT.
//  - Assert relation exists: assert_relation_exists(graph, GraphId::Node(module_id), GraphId::Node(value_id), RelationKind::Contains, "...");
//  - Assert value_id is in module_node.items().
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_relation_contains_file_module() {
    // Target: TOP_LEVEL_INT in "crate::const_static" module (const_static.rs)
    let fixture = "fixture_nodes";
    let file_path_rel = "src/const_static.rs";
    let module_path = vec!["crate".to_string(), "const_static".to_string()];
    let value_name = "TOP_LEVEL_INT";

    // let results = run_phase1_phase2(fixture);
    // Process results: Filter out errors and collect Ok values
    let successful_graphs = run_phases_and_collect(fixture);

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .unwrap_or_else(|| panic!("ParsedCodeGraph for const_static.rs not found"));
    let graph = &target_data.graph;

    // Find the file-level module node using the processed graphs
    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(), // Pass as slice
        fixture,
        file_path_rel,
        &module_path,
    );
    let module_id = module_node.id();
    // Find the value node (using basic helper for now)
    let value_node = find_value_node_basic(graph, &module_path, value_name);
    let value_id = value_node.id();

    // Assert Contains relation exists
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(value_id),
        RelationKind::Contains,
        &format!(
            "Expected Module '{}' ({}) to Contain Value '{}' ({})",
            module_node.name, module_id, value_name, value_id
        ),
    );

    // Assert value_id is in module_node.items()
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&value_id)),
        "ValueNode ID {} not found in items list for Module '{}' ({})",
        value_id,
        module_node.name,
        module_id
    );
}

// #[test] #[cfg(not(feature = "type_bearing_ids"))]
// fn test_value_node_relation_contains_inline_module()
//  - Target: INNER_CONST in "crate::inner_mod"
//  - Use full parse.
//  - Find ParsedCodeGraph for const_static.rs.
//  - Find inline module node for inner_mod (using find_inline_module_node_paranoid).
//  - Find ValueNode for INNER_CONST.
//  - Assert relation exists: assert_relation_exists(graph, GraphId::Node(module_id), GraphId::Node(value_id), RelationKind::Contains, "...");
//  - Assert value_id is in module_node.items().
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_relation_contains_inline_module() {
    // Target: INNER_CONST in "crate::const_static::inner_mod"
    let fixture = "fixture_nodes";
    let file_path_rel = "src/const_static.rs"; // INNER_CONST is defined in this file
    let module_path = vec![
        "crate".to_string(),
        "const_static".to_string(),
        "inner_mod".to_string(),
    ];
    let value_name = "INNER_CONST";

    // Process results: Filter out errors and collect Ok values
    let successful_graphs = run_phases_and_collect(fixture);

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .unwrap_or_else(|| panic!("ParsedCodeGraph for const_static.rs not found"));
    let graph = &target_data.graph;

    // Find the inline module node using the processed graphs
    let module_node = find_inline_module_node_paranoid(
        successful_graphs.as_slice(), // Pass as slice
        fixture,
        file_path_rel,
        &module_path,
    );
    let module_id = module_node.id();
    // Find the value node (using basic helper)
    let value_node = find_value_node_basic(graph, &module_path, value_name);
    let value_id = value_node.id();

    // Assert Contains relation exists
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(value_id),
        RelationKind::Contains,
        &format!(
            "Expected Module '{}' ({}) to Contain Value '{}' ({})",
            module_node.name, module_id, value_name, value_id
        ),
    );

    // Assert value_id is in module_node.items()
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&value_id)),
        "ValueNode ID {} not found in items list for Module '{}' ({})",
        value_id,
        module_node.name,
        module_id
    );
}

// Tier 5: Extreme Paranoia Tests
// Goal: Perform exhaustive checks on one complex const and one complex static,
//       mirroring the rigor of ModuleNode tests. Use paranoid helpers.
// ---------------------------------------------------------------------------------
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_paranoid_const_doc_attr() {
    // Target: pub const doc_attr_const: f64 = 3.14; (in "crate::const_static" module)
    let fixture = "fixture_nodes";
    let file_path_rel = "src/const_static.rs";
    let module_path = vec!["crate".to_string(), "const_static".to_string()];
    let value_name = "doc_attr_const";

    let results = run_phase1_phase2(fixture);
    // Collect owned graphs, consuming the Ok results
    let successful_graphs: Vec<ParsedCodeGraph> = results
        .into_iter() // Use into_iter to consume
        .filter_map(|res| res.ok()) // Use ok() to get owned value
        .collect();

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let crate_namespace = target_data.crate_namespace;
    let file_path = &target_data.file_path;

    // 1. Find node using paranoid helper (includes ID check)
    let node = find_value_node_paranoid(
        successful_graphs.as_slice(),
        fixture,
        file_path_rel,
        &module_path,
        value_name,
    );

    // 2. Assert all fields have expected values
    assert_eq!(node.name, value_name, "Name mismatch");
    assert_eq!(
        node.visibility,
        VisibilityKind::Public,
        "Visibility mismatch"
    );
    assert!(
        matches!(node.type_id, TypeId::Synthetic(_)),
        "TypeId should be Synthetic"
    );
    assert_eq!(node.kind(), ItemKind::Const, "Kind mismatch");
    // Note: Value representation might depend on syn/quote formatting. Adjust if needed.
    assert_eq!(node.value.as_deref(), Some("3.14"), "Value string mismatch");
    assert_eq!(node.attributes.len(), 2, "Attribute count mismatch");
    assert!(
        node.attributes.iter().any(|a| a.name == "deprecated"),
        "Missing 'deprecated' attribute"
    );
    assert!(
        node.attributes.iter().any(|a| a.name == "allow"),
        "Missing 'allow' attribute"
    );
    assert!(node.docstring.is_some(), "Expected docstring, found None");
    assert!(
        node.docstring
            .as_deref()
            .unwrap_or("")
            .contains("This is a documented constant."),
        "Docstring content mismatch"
    );
    assert!(node.tracking_hash.is_some(), "Tracking hash should be Some");

    // 3. Verify TypeId
    let type_node = find_type_node(graph, node.type_id);
    // Assuming f64 is parsed as a Named path for now. Adjust if it's Primitive.
    // TODO: Confirm how primitive types like f64 are represented in TypeKind.
    //       If it's TypeKind::Primitive { name: "f64" }, adjust assertion.
    match &type_node.kind() {
        TypeKind::Named { path, .. } => {
            assert_eq!(path, &["f64"], "TypeNode path mismatch for f64");
        }
        // Add other arms if f64 might be represented differently
        _ => panic!("Unexpected TypeKind for f64: {:?}", type_node.kind()),
    }
    assert!(
        type_node.related_types.is_empty(),
        "f64 TypeNode should have no related types"
    );
    // Regenerate TypeId based on structure
    let type_kind = ploke_core::TypeKind::Named {
        path: vec!["f64".to_string()],
        is_fully_qualified: false,
    };
    let related_ids: &[TypeId] = &[];
    // Pass the ValueNode's ID as the parent scope for its type
    let expected_type_id = TypeId::generate_synthetic(
        crate_namespace,
        file_path,
        &type_kind,
        related_ids,
        Some(node.id()), // Use the node's own ID as parent scope
    );
    assert_eq!(
        node.type_id, expected_type_id,
        "TypeId mismatch for f64. Expected (regen): {}, Actual: {}",
        expected_type_id, node.type_id
    );

    // 4. Verify Relation
    let module_node = find_file_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture,
        file_path_rel,
        &module_path,
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        "Missing Contains relation from module to value node",
    );
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&node.id())),
        "ValueNode ID not found in module items list"
    );

    // 5. Verify Uniqueness (within this file's graph)
    let duplicate_id_count = graph.values.iter().filter(|v| v.id == node.id).count();
    assert_eq!(
        duplicate_id_count, 1,
        "Found duplicate ValueNode ID {} in graph.values",
        node.id
    );
    // Note: Name uniqueness check is implicitly handled by the paranoid helper's module filtering.
    let duplicate_type_id_count = graph
        .type_graph
        .iter()
        .filter(|t| t.id == node.type_id)
        .count();
    assert_eq!(
        duplicate_type_id_count, 1,
        "Found duplicate TypeNode ID {} in graph.type_graph",
        node.type_id
    );
}
// fn test_value_node_paranoid_const_doc_attr()
//  - Target: pub const doc_attr_const: f64 = 3.14; (in "crate" module)
//  - Use full parse.
//  - Find ParsedCodeGraph for const_static.rs.
//  - Define expected context: crate_namespace, file_path, module_path (["crate"]), name ("doc_attr_const").
//  - Use a new paranoid helper `find_value_node_paranoid` (to be created in const_static_helpers.rs)
//    - This helper finds the node by name/module path.
//    - It extracts the span from the found node.
//    - It regenerates the expected NodeId::Synthetic using the context and extracted span.
//    - It asserts the found node's ID matches the regenerated ID.
//    - It returns the validated &ValueNode.
//  - Assert all fields have expected values (using strict asserts with detailed messages):
//      - id (already checked by helper)
//      - name == "doc_attr_const"
//      - visibility == Public
//      - type_id is Synthetic
//      - kind == Const
//      - value == Some("3.14") // Or maybe the approx constant representation? Check syn output.
//      - attributes.len() == 2 (check names/args specifically)
//      - docstring contains "This is a documented constant."
//      - tracking_hash is Some
//  - Verify TypeId:
//      - Find the TypeNode corresponding to node.type_id.
//      - Assert TypeNode.name == "f64".
//      - Assert TypeNode.kind() == TypeKind::Primitive.
//      - Assert TypeNode.id matches regenerated TypeId::Synthetic based on context (namespace, file, type string "f64").
//  - Verify Relation:
//      - Find crate module node using find_file_module_node_paranoid.
//      - Assert Contains relation exists from module to value node.
//      - Assert value node ID is in module.items().
//  - Verify Uniqueness (within this file's graph):
//      - Assert no other ValueNode in graph.values has the same ID.
//      - Assert no other ValueNode in graph.values has *exactly* the same name AND module path.
//      - Assert no other TypeNode in graph.type_graph has the same TypeId (unless it's genuinely the same primitive/path type).

// fn test_value_node_paranoid_static_mut_inner_mod()
//  - Target: pub(super) static mut INNER_MUT_STATIC: bool = false; (in "crate::inner_mod")
//  - Use full parse.
//  - Find ParsedCodeGraph for const_static.rs.
//  - Define expected context: crate_namespace, file_path, module_path (["crate", "inner_mod"]), name ("INNER_MUT_STATIC").
//  - Use `find_value_node_paranoid` helper.
//  - Assert all fields:
//      - id (checked by helper)
//      - name == "INNER_MUT_STATIC"
//      - visibility == Restricted(vec!["super".into()])
//      - type_id is Synthetic
//      - kind == Static { is_mutable: true }
//      - value == Some("false")
//      - attributes contains #[allow(dead_code)]
//      - docstring is None
//      - tracking_hash is Some
//  - Verify TypeId:
//      - Find TypeNode for node.type_id.
//      - Assert TypeNode.name == "bool".
//      - Assert TypeNode.kind() == TypeKind::Primitive.
//      - Assert TypeNode.id matches regenerated TypeId::Synthetic based on context (namespace, file, type string "bool").
//  - Verify Relation:
//      - Find inner_mod module node (e.g., find_inline_module_by_path).
//      - Assert Contains relation exists from module to value node.
//      - Assert value node ID is in module.items().
//  - Verify Uniqueness (within this file's graph):
//      - Assert no other ValueNode has the same ID.
//      - Assert no other ValueNode has the same name AND module path.
//      - Assert no other TypeNode has the same TypeId (unless it's bool again).
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_value_node_paranoid_static_mut_inner_mod() {
    // Target: pub(super) static mut INNER_MUT_STATIC: bool = false; (in "crate::const_static::inner_mod")
    let fixture = "fixture_nodes";
    let file_path_rel = "src/const_static.rs";
    let module_path = vec![
        "crate".to_string(),
        "const_static".to_string(),
        "inner_mod".to_string(),
    ];
    let value_name = "INNER_MUT_STATIC";

    let results = run_phase1_phase2(fixture);
    // Collect owned graphs, consuming the Ok results
    let successful_graphs: Vec<ParsedCodeGraph> = results
        .into_iter() // Use into_iter to consume
        .filter_map(|res| res.ok()) // Use ok() to get owned value
        .collect();

    let target_data = successful_graphs
        .iter()
        .find(|d| d.file_path.ends_with(file_path_rel))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;
    let crate_namespace = target_data.crate_namespace;
    let file_path = &target_data.file_path;

    // 1. Find node using paranoid helper (includes ID check)
    let node = find_value_node_paranoid(
        successful_graphs.as_slice(),
        fixture,
        file_path_rel,
        &module_path,
        value_name,
    );

    // 2. Assert all fields
    assert_eq!(node.name, value_name, "Name mismatch");
    assert_eq!(
        node.visibility,
        VisibilityKind::Restricted(vec!["super".to_string()]),
        "Visibility mismatch"
    );
    assert!(
        matches!(node.type_id, TypeId::Synthetic(_)),
        "TypeId should be Synthetic"
    );
    // assert_eq!(
    //     node.kind(),
    //     ItemKind::Static { is_mutable: true },
    //     "Kind mismatch"
    // );
    assert_eq!(
        node.value.as_deref(),
        Some("false"),
        "Value string mismatch"
    );
    // Check for the specific #[allow(dead_code)] attribute
    assert_eq!(node.attributes.len(), 1, "Expected exactly one attribute");
    let attr = &node.attributes[0];
    assert_eq!(attr.name, "allow", "Attribute name should be 'allow'");
    assert!(
        attr.args.contains(&"dead_code".to_string()),
        "Attribute args should contain 'dead_code'"
    );
    assert!(node.docstring.is_none(), "Docstring should be None");
    assert!(node.tracking_hash.is_some(), "Tracking hash should be Some");

    // 3. Verify TypeId
    let type_node = find_type_node(graph, node.type_id);
    // Assuming bool is parsed as a Named path. Adjust if Primitive.
    // TODO: Confirm how primitive types like bool are represented in TypeKind.
    match &type_node.kind() {
        TypeKind::Named { path, .. } => {
            assert_eq!(path, &["bool"], "TypeNode path mismatch for bool");
        }
        _ => panic!("Unexpected TypeKind for bool: {:?}", type_node.kind()),
    }
    assert!(
        type_node.related_types.is_empty(),
        "bool TypeNode should have no related types"
    );
    // Regenerate TypeId based on structure
    let type_kind = ploke_core::TypeKind::Named {
        path: vec!["bool".to_string()],
        is_fully_qualified: false,
    };
    let related_ids: &[TypeId] = &[];
    // Pass the ValueNode's ID as the parent scope for its type
    let expected_type_id = TypeId::generate_synthetic(
        crate_namespace,
        file_path,
        &type_kind,
        related_ids,
        Some(node.id()), // Use the node's own ID as parent scope
    );

    #[cfg(feature = "verbose_debug")]
    crate::common::debug_printers::debug_print_static_info(
        graph,
        crate_namespace,
        file_path,
        node,
        type_node,
        type_kind,
        related_ids,
        expected_type_id,
    );
    assert_eq!(
        node.type_id, expected_type_id,
        "TypeId mismatch for bool. Expected (regen): {}, Actual: {},",
        expected_type_id, node.type_id
    );

    // 4. Verify Relation
    let module_node = find_inline_module_node_paranoid(
        successful_graphs.as_slice(),
        fixture,
        file_path_rel,
        &module_path,
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_node.id()),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        "Missing Contains relation from module to value node",
    );
    assert!(
        module_node
            .items()
            .is_some_and(|items| items.contains(&node.id())),
        "ValueNode ID not found in module items list"
    );

    // 5. Verify Uniqueness (within this file's graph)
    let duplicate_id_count = graph.values.iter().filter(|v| v.id == node.id).count();
    assert_eq!(
        duplicate_id_count, 1,
        "Found duplicate ValueNode ID {} in graph.values",
        node.id
    );
    // Check for duplicate TypeId for 'bool' - might exist if other bools are present
    let duplicate_type_id_count = graph
        .type_graph
        .iter()
        .filter(|t| t.id == node.type_id)
        .count();
    assert_eq!(
        duplicate_type_id_count, 1,
        "Found duplicate TypeNode ID {} for 'bool' in graph.type_graph",
        node.type_id
    );
}

// --- Helper Functions (Now defined) ---
// find_value_node_paranoid in common/paranoid/const_static_helpers.rs
//   - Takes parsed_graphs, fixture, relative_file_path, expected_module_path, value_name.
//   - Finds the ParsedCodeGraph.
//   - Finds the ModuleNode for the expected_module_path within that graph.
//   - Filters graph.values by name AND checks if the ID is in the ModuleNode's items.
//   - Asserts exactly one candidate remains.
//   - Extracts span from the found ValueNode.
//   - Regenerates NodeId::Synthetic using context + extracted span.
//   - Asserts found ID == regenerated ID.
//   - Returns the validated ValueNode reference.

// find_value_node_basic defined earlier in this file.

// regenerate_value_node_id - Handled inline within test_value_node_field_id_regeneration.

// --- Tests for Known Limitations ---

#[ignore = "Known Limitation: Associated const in impl blocks not parsed. See docs/plans/uuid_refactor/02c_phase2_known_limitations.md"]
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_associated_const_found_in_impl() {
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;

    // This assertion is expected to FAIL until the limitation is addressed.
    assert!(
        graph.values.iter().any(|v| v.name == "IMPL_CONST"),
        "ValueNode 'IMPL_CONST' (associated const in impl) was not found in graph.values"
    );
    // TODO: Add further checks once the node is found (e.g., visibility, kind, relation to impl block)
}

#[ignore = "Known Limitation: Associated const in trait impl blocks not parsed. See docs/plans/uuid_refactor/02c_phase2_known_limitations.md"]
#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_associated_const_found_in_trait_impl() {
    // Note: The fixture defines TRAIT_REQ_CONST within an `impl ExampleTrait for Container` block.
    let results = run_phase1_phase2("fixture_nodes");
    let fixture_path = fixtures_crates_dir()
        .join("fixture_nodes")
        .join("src")
        .join("const_static.rs");
    let target_data = results
        .iter()
        .find_map(|res| res.as_ref().ok().filter(|d| d.file_path == fixture_path))
        .expect("ParsedCodeGraph for const_static.rs not found");
    let graph = &target_data.graph;

    // This assertion is expected to FAIL until the limitation is addressed.
    assert!(
        graph.values.iter().any(|v| v.name == "TRAIT_REQ_CONST"),
        "ValueNode 'TRAIT_REQ_CONST' (associated const in trait impl) was not found in graph.values"
    );
    // TODO: Add further checks once the node is found
}

// NOTE: Tests for associated types (`test_associated_type_found_in_impl`, `test_associated_type_found_in_trait`)
// are omitted for now as the current `const_static.rs` fixture does not contain associated types.
// They should be added when fixtures are updated or when testing traits/impls specifically.

// regenerate_type_id - Handled inline within paranoid tests.
