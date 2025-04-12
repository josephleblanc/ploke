#![cfg(feature = "uuid_ids")] // Gate the whole module

use crate::common::paranoid::const_static_helpers::*; // TODO: Create this helper
use crate::common::paranoid::find_file_module_node_paranoid; // Use module helper
use crate::common::uuid_ids_utils::*;
use ploke_common::{fixtures_crates_dir, workspace_root};
use ploke_core::{NodeId, TrackingHash, TypeId};
use std::{collections::HashMap, path::Path};
use syn_parser::parser::nodes::{Attribute, ValueKind};
use syn_parser::parser::types::VisibilityKind;
use syn_parser::{
    discovery::{run_discovery_phase, DiscoveryOutput},
    parser::{
        analyze_files_parallel,
        graph::CodeGraph,
        nodes::{ModuleNode, ValueNode, Visible},
        relations::{GraphId, RelationKind},
        visitor::ParsedCodeGraph,
    },
};

// Test Plan for ValueNode (const/static) in Phase 2 (uuid_ids)
// ============================================================
// Fixture: tests/fixture_crates/fixture_nodes/src/const_static.rs
// Helper Location: tests/common/paranoid/const_static_helpers.rs

// --- Test Tiers ---

// Tier 1: Basic Smoke Tests
// Goal: Quickly verify that ValueNodes are created for various const/static items
//       and basic properties (IDs, hash, kind) are present. Uses full crate parse.
// ---------------------------------------------------------------------------------
// #[test] fn test_const_static_basic_smoke_test_full_parse()
//  - Use run_phase1_phase2("fixture_nodes")
//  - Find the ParsedCodeGraph for const_static.rs
//  - Iterate through expected const/static names (e.g., TOP_LEVEL_INT, TOP_LEVEL_STR, INNER_CONST)
//  - For each:
//      - Find the ValueNode (e.g., using graph.values.iter().find(|v| v.name == ...))
//      - Assert node exists.
//      - Assert node.id is NodeId::Synthetic(_).
//      - Assert node.tracking_hash is Some(TrackingHash(_)).
//      - Assert node.type_id is TypeId::Synthetic(_).
//      - Assert node.kind matches (Constant or Static { is_mutable }).
//      - Assert node.visibility is not Unknown (basic check).

// Tier 2: Targeted Field Verification
// Goal: Verify each field of the ValueNode struct individually for specific examples.
//       These tests act as diagnostics if specific fields break later. Use detailed asserts.
//       Can use single-file parsing helper if appropriate, or full parse.
// ---------------------------------------------------------------------------------
// #[test] fn test_value_node_field_id_regeneration()
//  - Target: TOP_LEVEL_INT
//  - Find the ValueNode.
//  - Get context: crate_namespace, file_path, module_path (["crate"]), name ("TOP_LEVEL_INT"), span.
//  - Regenerate expected NodeId::Synthetic using NodeId::generate_synthetic.
//  - Assert_eq!(node.id, regenerated_id, "Mismatch for ID field. Expected: {}, Actual: {}", ...);

// #[test] fn test_value_node_field_name()
//  - Target: TOP_LEVEL_BOOL
//  - Find the ValueNode.
//  - Assert_eq!(node.name, "TOP_LEVEL_BOOL", "Mismatch for name field. Expected: {}, Actual: {}", ...);

// #[test] fn test_value_node_field_visibility_public()
//  - Target: TOP_LEVEL_BOOL (pub)
//  - Find the ValueNode.
//  - Assert_eq!(node.visibility, VisibilityKind::Public, "Mismatch for visibility field. Expected: {:?}, Actual: {:?}", ...);

// #[test] fn test_value_node_field_visibility_inherited()
//  - Target: TOP_LEVEL_INT (private)
//  - Find the ValueNode.
//  - Assert_eq!(node.visibility, VisibilityKind::Inherited, "Mismatch for visibility field. Expected: {:?}, Actual: {:?}", ...);

// #[test] fn test_value_node_field_visibility_crate()
//  - Target: INNER_CONST (pub(crate))
//  - Find the ValueNode.
//  - Assert_eq!(node.visibility, VisibilityKind::Restricted(vec!["crate".into()]), "Mismatch for visibility field. Expected: {:?}, Actual: {:?}", ...); // Assuming Restricted("crate") representation

// #[test] fn test_value_node_field_visibility_super()
//  - Target: INNER_MUT_STATIC (pub(super))
//  - Find the ValueNode.
//  - Assert_eq!(node.visibility, VisibilityKind::Restricted(vec!["super".into()]), "Mismatch for visibility field. Expected: {:?}, Actual: {:?}", ...); // Assuming Restricted("super") representation

// #[test] fn test_value_node_field_type_id_presence()
//  - Target: ARRAY_CONST
//  - Find the ValueNode.
//  - Assert!(matches!(node.type_id, TypeId::Synthetic(_)), "type_id field should be Synthetic. Actual: {:?}", node.type_id);

// #[test] fn test_value_node_field_kind_const()
//  - Target: TOP_LEVEL_INT
//  - Find the ValueNode.
//  - Assert_eq!(node.kind, ValueKind::Constant, "Mismatch for kind field. Expected: {:?}, Actual: {:?}", ...);

// #[test] fn test_value_node_field_kind_static_imm()
//  - Target: TOP_LEVEL_STR
//  - Find the ValueNode.
//  - Assert_eq!(node.kind, ValueKind::Static { is_mutable: false }, "Mismatch for kind field. Expected: {:?}, Actual: {:?}", ...);

// #[test] fn test_value_node_field_kind_static_mut()
//  - Target: TOP_LEVEL_COUNTER
//  - Find the ValueNode.
//  - Assert_eq!(node.kind, ValueKind::Static { is_mutable: true }, "Mismatch for kind field. Expected: {:?}, Actual: {:?}", ...);

// #[test] fn test_value_node_field_value_string()
//  - Target: TOP_LEVEL_INT (= 10)
//  - Find the ValueNode.
//  - Assert_eq!(node.value.as_deref(), Some("10"), "Mismatch for value field. Expected: {:?}, Actual: {:?}", ...);
//  - Target: EXPR_CONST (= 5 * 2 + 1)
//  - Find the ValueNode.
//  - Assert_eq!(node.value.as_deref(), Some("5 * 2 + 1"), "Mismatch for value field. Expected: {:?}, Actual: {:?}", ...); // Verify expression is captured

// #[test] fn test_value_node_field_attributes_single()
//  - Target: DOC_ATTR_STATIC (#[cfg(target_os = "linux")])
//  - Find the ValueNode.
//  - Assert_eq!(node.attributes.len(), 1, "Expected 1 attribute, found {}. Attrs: {:?}", node.attributes.len(), node.attributes);
//  - Assert_eq!(node.attributes[0].name, "cfg");
//  - Assert!(node.attributes[0].args.contains(&"target_os = \"linux\"".to_string())); // Check specific arg if possible

// #[test] fn test_value_node_field_attributes_multiple()
//  - Target: doc_attr_const (#[deprecated(...)], #[allow(...)])
//  - Find the ValueNode.
//  - Assert_eq!(node.attributes.len(), 2, "Expected 2 attributes, found {}. Attrs: {:?}", node.attributes.len(), node.attributes);
//  - Assert!(node.attributes.iter().any(|a| a.name == "deprecated"), "Missing 'deprecated' attribute");
//  - Assert!(node.attributes.iter().any(|a| a.name == "allow"), "Missing 'allow' attribute");
//  - // Maybe check specific args for one of them

// #[test] fn test_value_node_field_docstring()
//  - Target: TOP_LEVEL_INT ("A top-level private constant...")
//  - Find the ValueNode.
//  - Assert!(node.docstring.is_some(), "Expected docstring, found None");
//  - Assert!(node.docstring.as_deref().unwrap_or("").contains("top-level private constant"), "Docstring mismatch. Expected contains: '{}', Actual: {:?}", "...", node.docstring);

// #[test] fn test_value_node_field_tracking_hash_presence()
//  - Target: ALIASED_CONST
//  - Find the ValueNode.
//  - Assert!(node.tracking_hash.is_some(), "tracking_hash field should be Some. Actual: {:?}", node.tracking_hash);
//  - Assert!(matches!(node.tracking_hash, Some(TrackingHash(_))), "tracking_hash should contain a Uuid");

// Tier 3: Subfield Variations
// Goal: Verify specific variations within complex fields like `visibility` and `kind`.
//       These might overlap with Tier 2 but ensure explicit coverage of variants.
// ---------------------------------------------------------------------------------
// (Covered by Tier 2 tests for visibility and kind variants)

// Tier 4: Basic Connection Tests
// Goal: Verify the `Contains` relationship between modules and ValueNodes.
// ---------------------------------------------------------------------------------
// #[test] fn test_value_node_relation_contains_file_module()
//  - Target: TOP_LEVEL_INT in "crate" module (const_static.rs)
//  - Use full parse: run_phase1_phase2("fixture_nodes")
//  - Find ParsedCodeGraph for const_static.rs.
//  - Find crate module node (file-level root) using find_file_module_node_paranoid.
//  - Find ValueNode for TOP_LEVEL_INT.
//  - Assert relation exists: assert_relation_exists(graph, GraphId::Node(module_id), GraphId::Node(value_id), RelationKind::Contains, "...");
//  - Assert value_id is in module_node.items().

// #[test] fn test_value_node_relation_contains_inline_module()
//  - Target: INNER_CONST in "crate::inner_mod"
//  - Use full parse.
//  - Find ParsedCodeGraph for const_static.rs.
//  - Find inline module node for inner_mod (using find_inline_module_by_path or similar helper).
//  - Find ValueNode for INNER_CONST.
//  - Assert relation exists: assert_relation_exists(graph, GraphId::Node(module_id), GraphId::Node(value_id), RelationKind::Contains, "...");
//  - Assert value_id is in module_node.items().

// Tier 5: Extreme Paranoia Tests
// Goal: Perform exhaustive checks on one complex const and one complex static,
//       mirroring the rigor of ModuleNode tests. Use paranoid helpers.
// ---------------------------------------------------------------------------------
// #[test] fn test_value_node_paranoid_const_doc_attr()
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
//      - kind == Constant
//      - value == Some("3.14") // Or maybe the approx constant representation? Check syn output.
//      - attributes.len() == 2 (check names/args specifically)
//      - docstring contains "This is a documented constant."
//      - tracking_hash is Some
//  - Verify TypeId:
//      - Find the TypeNode corresponding to node.type_id.
//      - Assert TypeNode.name == "f64".
//      - Assert TypeNode.kind == TypeKind::Primitive.
//      - Assert TypeNode.id matches regenerated TypeId::Synthetic based on context (namespace, file, type string "f64").
//  - Verify Relation:
//      - Find crate module node using find_file_module_node_paranoid.
//      - Assert Contains relation exists from module to value node.
//      - Assert value node ID is in module.items().
//  - Verify Uniqueness (within this file's graph):
//      - Assert no other ValueNode in graph.values has the same ID.
//      - Assert no other ValueNode in graph.values has *exactly* the same name AND module path.
//      - Assert no other TypeNode in graph.type_graph has the same TypeId (unless it's genuinely the same primitive/path type).

// #[test] fn test_value_node_paranoid_static_mut_inner_mod()
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
//      - attributes is empty
//      - docstring is None
//      - tracking_hash is Some
//  - Verify TypeId:
//      - Find TypeNode for node.type_id.
//      - Assert TypeNode.name == "bool".
//      - Assert TypeNode.kind == TypeKind::Primitive.
//      - Assert TypeNode.id matches regenerated TypeId::Synthetic based on context (namespace, file, type string "bool").
//  - Verify Relation:
//      - Find inner_mod module node (e.g., find_inline_module_by_path).
//      - Assert Contains relation exists from module to value node.
//      - Assert value node ID is in module.items().
//  - Verify Uniqueness (within this file's graph):
//      - Assert no other ValueNode has the same ID.
//      - Assert no other ValueNode has the same name AND module path.
//      - Assert no other TypeNode has the same TypeId (unless it's bool again).

// --- Helper Functions (To be created in common/paranoid/const_static_helpers.rs) ---
// fn find_value_node_paranoid<'a>(...) -> &'a ValueNode
//   - Similar structure to find_function_node_paranoid or find_module_node_paranoid.
//   - Takes parsed_graphs, fixture_name, relative_file_path, expected_module_path, value_name.
//   - Finds the ParsedCodeGraph.
//   - Finds the ModuleNode for the expected_module_path within that graph.
//   - Filters graph.values by name AND checks if the ID is in the ModuleNode's items.
//   - Asserts exactly one candidate remains.
//   - Extracts span from the found ValueNode.
//   - Regenerates NodeId::Synthetic using context + extracted span.
//   - Asserts found ID == regenerated ID.
//   - Returns the validated ValueNode reference.

// fn find_value_node_basic<'a>(graph: &'a CodeGraph, module_path: &[String], value_name: &str) -> Option<&'a ValueNode>
//   - Non-paranoid helper for simpler tests.
//   - Finds module node by path.
//   - Finds value node by name within that module's items.
//   - Returns Option<&ValueNode>. (Maybe add to uuid_ids_utils.rs instead?)

// fn regenerate_value_node_id(...) -> NodeId
//   - Helper specifically for test_value_node_field_id_regeneration.
//   - Takes context (namespace, file, mod path, name, span).
//   - Calls NodeId::generate_synthetic.
//   - Returns the ID.

// fn regenerate_type_id(...) -> TypeId
//   - Helper for paranoid tests to verify TypeId.
//   - Takes context (namespace, file, type_string).
//   - Calls TypeId::generate_synthetic.
//   - Returns the ID.
