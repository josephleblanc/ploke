//! Tests for `TraitNode` parsing and field extraction.
//!
//! ## Test Coverage Analysis
//!
//! *   **Fixture:** `tests/fixture_crates/fixture_nodes/src/traits.rs`
//! *   **Tests:** `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/traits.rs` (using `paranoid_test_fields_and_values!`)
//!
//! ### 1. Coverage of Fixture Items:
//!
//! The `EXPECTED_TRAITS_ARGS` and `EXPECTED_TRAITS_DATA` maps cover all 20 distinct trait items from the `fixture_nodes/src/traits.rs` fixture. This includes:
//! *   Traits at the module root (`crate::traits`).
//! *   Traits within the nested `inner` module (`crate::traits::inner`).
//!
//! The covered traits test a variety of:
//! *   Visibilities (`Public`, `Inherited` (private), `Crate`, `Restricted(["super"])`).
//! *   Generic parameters (type, lifetime, multiple, with bounds).
//! *   Supertraits (none, single, multiple, generic).
//! *   Attributes (none, `#[must_use]`).
//! *   Docstrings (none, present).
//! *   Method counts (1 or 2 methods per trait).
//! *   `unsafe` traits.
//! *   Traits with methods using `Self` and associated types/consts (though these are primarily tested via method presence/counts).
//!
//! **Conclusion for Fixture Coverage:** Excellent. All trait items from the specified fixture file are covered by the `paranoid_test_fields_and_values!` tests.
//!
//! ### 2. Coverage of `TraitNode` Property Variations (via `ExpectedTraitNode`):
//!
//! Based on the 20 items covered:
//!
//! *   `id: TraitNodeId`: Implicitly covered by ID generation and lookup.
//! *   `name: String`: Excellent coverage (various unique names).
//! *   `span: (usize, usize)`: Not directly asserted by value in the new tests.
//! *   `visibility: VisibilityKind`: Excellent coverage (`Public`, `Inherited`, `Crate`, `Restricted`).
//! *   `methods: Vec<MethodNode>`: Covered by `methods_count`. All tested traits have 1 or 2 methods.
//! *   `generic_params: Vec<GenericParamNode>`: Covered by `generic_params_count`. Tests cover 0, 1, and 3 generic parameters.
//! *   `super_traits: Vec<TypeId>`: Covered by `super_traits_count`. Tests cover 0, 1, and 3 supertraits.
//! *   `attributes: Vec<Attribute>`: Good coverage (mostly empty, one with `#[must_use]`).
//! *   `docstring: Option<String>`: Good coverage (mostly `None`, one with a docstring).
//! *   `tracking_hash: Option<TrackingHash>`: Covered by `tracking_hash_check: true` for all.
//! *   `cfgs: Vec<String>`: Poor coverage (all tested traits have no `cfg` attributes, so `vec![]` is consistently checked).
//!
//! **Conclusion for Property Variation Coverage (with `ExpectedTraitNode`):**
//! *   **Excellent:** `name`, `visibility`, `methods_count`, `generic_params_count`, `super_traits_count`, `tracking_hash_check`.
//! *   **Good (but limited variety):** `attributes`, `docstring`.
//! *   **Poor:** `cfgs`.
//! *   **Not Directly Tested by `ExpectedTraitNode`:** Specific details of `methods` (like `MethodNode` fields: name, parameters, return type, docs), `generic_params` (like `GenericParamNode` fields: name, kind, bounds), and `super_traits` (the actual `TypeId`s and their resolved paths).
//!
//! ### 3. Differences in Testing `TraitNode` vs. Other Nodes:
//!
//! Testing `TraitNode` with the current `ExpectedTraitNode` focuses on:
//! *   Counts for `methods`, `generic_params`, and `super_traits`.
//! *   Basic metadata: Name, visibility, attributes, docstrings, CFGs.
//!
//! Unlike `FunctionNode` where parameter/return type presence is checked, `TraitNode`'s method details are reduced to a count. Similarly, supertrait identities are reduced to a count.
//!
//! ### 4. Lost Coverage from Old Tests (Regressions if old tests were removed):
//!
//! The old tests (still present in this file but `cfg`-gated) performed more detailed checks:
//! *   **Method Details:**
//!     *   Specific method names.
//!     *   Parameter counts and `is_self` for methods.
//!     *   Return type presence and `TypeId` lookup to check `TypeKind` for methods.
//!     *   Docstrings on methods.
//! *   **SuperTrait Details:**
//!     *   Specific `TypeId`s of supertraits were resolved to `TypeNode`s, and their `kind` (e.g., `TypeKind::Named { path, .. }`) and `related_types` (for generics) were asserted. This allowed verifying *which* traits were supertraits.
//! *   **Generic Parameter Details (for trait generics):** The old tests had TODOs for detailed checks, so this is not a direct regression from asserted behavior but remains an area not covered by the new macro's count-based check.
//! *   **`unsafe` flag:** The old tests noted that `TraitNode` doesn't have an `is_unsafe` flag. This observation remains.
//!
//! If the old tests were removed, the new macro-based tests would not verify the specific names, signatures, or types of individual methods or supertraits, only their counts.
//!
//! ### 5. Suggestions for Future Inclusions/Improvements:
//!
//! *   **CFGs:** Add fixture traits with `#[cfg(...)]` attributes to improve coverage for this field.
//! *   **Detailed Method/SuperTrait Checks:**
//!     *   To regain lost coverage, consider creating a few targeted, manual test functions (not using the `paranoid_test_fields_and_values!` macro) for 1-2 complex traits. These tests would manually iterate `trait_node.methods` and `trait_node.super_traits` to assert specific details, similar to the old tests.
//!     *   Alternatively, explore enhancing `ExpectedTraitNode` and the `derive_expected_data` macro to support `Vec<ExpectedMethodNode>` or `Vec<ExpectedSuperTraitInfo>` if this level of detail is desired across many tests, but this is a significantly larger undertaking for the derive macro.
//! *   **Associated Types/Consts:** Currently, these are not direct fields on `TraitNode`. If they were added, `ExpectedTraitNode` and tests would need to be updated. Methods related to them are implicitly part of `methods_count`.
//! *   **`unsafe` flag:** If an `is_unsafe` field is added to `TraitNode` in the future, tests should cover it.
//! *   **Relation Checks:** The `paranoid_test_fields_and_values!` macro checks `Module Contains Trait`. If `Trait Contains Method` relations (or others) become important for Phase 2, they would need separate assertion.

use crate::common::run_phases_and_collect;
use crate::common::ParanoidArgs;
use crate::paranoid_test_fields_and_values;
use lazy_static::lazy_static;
use ploke_core::ItemKind;
use std::collections::HashMap;
use syn_parser::error::SynParserError;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::{Attribute, ExpectedTraitNode, PrimaryNodeIdTrait};
use syn_parser::parser::types::VisibilityKind;

pub const LOG_TEST_TRAIT: &str = "log_test_trait";

lazy_static! {
    static ref EXPECTED_TRAITS_ARGS: HashMap<&'static str, ParanoidArgs<'static>> = {
        let mut m = HashMap::new();
        let fixture_name = "fixture_nodes";
        let rel_path = "src/traits.rs";

        m.insert(
            "crate::traits::SimpleTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "SimpleTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::InternalTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "InternalTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::CrateTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "CrateTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::DocumentedTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "DocumentedTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::GenericTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "GenericTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::LifetimeTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "LifetimeTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::ComplexGenericTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "ComplexGenericTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::AssocTypeTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "AssocTypeTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::AssocTypeWithBounds",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "AssocTypeWithBounds",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::AssocConstTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "AssocConstTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::SuperTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "SuperTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::MultiSuperTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "MultiSuperTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::GenericSuperTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "GenericSuperTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::AttributedTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "AttributedTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::UnsafeTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "UnsafeTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::inner::InnerSecretTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "InnerSecretTrait",
                expected_path: &["crate", "traits", "inner"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::inner::InnerPublicTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "InnerPublicTrait",
                expected_path: &["crate", "traits", "inner"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::inner::SuperGraphNodeTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "SuperGraphNodeTrait",
                expected_path: &["crate", "traits", "inner"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::SelfUsageTrait",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "SelfUsageTrait",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m.insert(
            "crate::traits::SelfInAssocBound",
            ParanoidArgs {
                fixture: fixture_name,
                relative_file_path: rel_path,
                ident: "SelfInAssocBound",
                expected_path: &["crate", "traits"],
                item_kind: ItemKind::Trait,
                expected_cfg: None,
            },
        );
        m
    };
    static ref EXPECTED_TRAITS_DATA: HashMap<&'static str, ExpectedTraitNode> = {
        let mut m = HashMap::new();

        m.insert(
            "crate::traits::SimpleTrait",
            ExpectedTraitNode {
                name: "SimpleTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::InternalTrait",
            ExpectedTraitNode {
                name: "InternalTrait",
                visibility: VisibilityKind::Inherited,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::CrateTrait",
            ExpectedTraitNode {
                name: "CrateTrait",
                visibility: VisibilityKind::Crate,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::DocumentedTrait",
            ExpectedTraitNode {
                name: "DocumentedTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: Some("Documented public trait"),
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::GenericTrait",
            ExpectedTraitNode {
                name: "GenericTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 1,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::LifetimeTrait",
            ExpectedTraitNode {
                name: "LifetimeTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 1,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::ComplexGenericTrait",
            ExpectedTraitNode {
                name: "ComplexGenericTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 3,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::AssocTypeTrait",
            ExpectedTraitNode {
                name: "AssocTypeTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::AssocTypeWithBounds",
            ExpectedTraitNode {
                name: "AssocTypeWithBounds",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::AssocConstTrait",
            ExpectedTraitNode {
                name: "AssocConstTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::SuperTrait",
            ExpectedTraitNode {
                name: "SuperTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 1,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::MultiSuperTrait",
            ExpectedTraitNode {
                name: "MultiSuperTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 3,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::GenericSuperTrait",
            ExpectedTraitNode {
                name: "GenericSuperTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 1,
                super_traits_count: 1,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::AttributedTrait",
            ExpectedTraitNode {
                name: "AttributedTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![Attribute {
                    name: "must_use".to_string(),
                    args: vec![],
                    value: Some("Trait results should be used".to_string()),
                }],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::UnsafeTrait",
            ExpectedTraitNode {
                name: "UnsafeTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::inner::InnerSecretTrait",
            ExpectedTraitNode {
                name: "InnerSecretTrait",
                visibility: VisibilityKind::Inherited,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::inner::InnerPublicTrait",
            ExpectedTraitNode {
                name: "InnerPublicTrait",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::inner::SuperGraphNodeTrait",
            ExpectedTraitNode {
                name: "SuperGraphNodeTrait",
                visibility: VisibilityKind::Restricted(vec!["super".to_string()]),
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 1,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::SelfUsageTrait",
            ExpectedTraitNode {
                name: "SelfUsageTrait",
                visibility: VisibilityKind::Public,
                methods_count: 2,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m.insert(
            "crate::traits::SelfInAssocBound",
            ExpectedTraitNode {
                name: "SelfInAssocBound",
                visibility: VisibilityKind::Public,
                methods_count: 1,
                generic_params_count: 0,
                super_traits_count: 0,
                attributes: vec![],
                docstring: None,
                tracking_hash_check: true,
                cfgs: vec![],
            },
        );
        m
    };
}

paranoid_test_fields_and_values!(
    trait_node_simple_trait,
    "crate::traits::SimpleTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_internal_trait,
    "crate::traits::InternalTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_crate_trait,
    "crate::traits::CrateTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_documented_trait,
    "crate::traits::DocumentedTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_generic_trait,
    "crate::traits::GenericTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_lifetime_trait,
    "crate::traits::LifetimeTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_complex_generic_trait,
    "crate::traits::ComplexGenericTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_assoc_type_trait,
    "crate::traits::AssocTypeTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_assoc_type_with_bounds,
    "crate::traits::AssocTypeWithBounds",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_assoc_const_trait,
    "crate::traits::AssocConstTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_super_trait,
    "crate::traits::SuperTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_multi_super_trait,
    "crate::traits::MultiSuperTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_generic_super_trait,
    "crate::traits::GenericSuperTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_attributed_trait,
    "crate::traits::AttributedTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_unsafe_trait,
    "crate::traits::UnsafeTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_inner_secret_trait,
    "crate::traits::inner::InnerSecretTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_inner_public_trait,
    "crate::traits::inner::InnerPublicTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_super_graph_node_trait,
    "crate::traits::inner::SuperGraphNodeTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_self_usage_trait,
    "crate::traits::SelfUsageTrait",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);
paranoid_test_fields_and_values!(
    trait_node_self_in_assoc_bound,
    "crate::traits::SelfInAssocBound",
    EXPECTED_TRAITS_ARGS,
    EXPECTED_TRAITS_DATA,
    syn_parser::parser::nodes::TraitNode,
    syn_parser::parser::nodes::ExpectedTraitNode,
    as_trait,
    LOG_TEST_TRAIT
);

// --- Old Test Cases (Kept as per instruction) ---
#[cfg(not(feature = "type_bearing_ids"))]
use crate::common::paranoid::find_trait_node_paranoid;
// Gate the whole module
#[cfg(not(feature = "type_bearing_ids"))]
use crate::common::uuid_ids_utils::*;
#[cfg(not(feature = "type_bearing_ids"))]
use ploke_core::TypeKind;
#[cfg(not(feature = "type_bearing_ids"))]
use syn_parser::parser::nodes::GraphId;
// Import TypeKind from ploke_core
// Import UnionNode specifically
// use syn_parser::parser::types::VisibilityKind; // Already imported above
#[cfg(not(feature = "type_bearing_ids"))]
use syn_parser::parser::{nodes::GraphNode, relations::RelationKind};

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_trait_node_simple_trait_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let trait_name = "SimpleTrait";
    let relative_file_path = "src/traits.rs";
    let module_path = vec!["crate".to_string(), "traits".to_string()]; // Defined at top level of file

    let trait_node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );

    // --- Assertions ---
    let graph = &results // Need graph for type/relation lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(trait_node.id(), NodeId::Synthetic(_)));
    assert!(
        trait_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(trait_node.name(), trait_name);
    assert_eq!(trait_node.visibility(), VisibilityKind::Public);
    assert!(trait_node.attributes.is_empty());
    assert!(trait_node.docstring.is_none());
    assert!(trait_node.generic_params.is_empty());
    assert!(trait_node.super_traits.is_empty());

    // Methods (fn required_method(&self) -> i32;)
    assert_eq!(trait_node.methods.len(), 1);
    let method_node = &trait_node.methods[0];
    assert_eq!(method_node.name(), "required_method");
    assert!(matches!(method_node.id(), NodeId::Synthetic(_)));
    assert!(method_node.tracking_hash.is_some()); // Methods within traits should have hashes
    assert_eq!(method_node.parameters.len(), 1); // &self
    assert!(method_node.parameters[0].is_self);
    assert!(method_node.return_type.is_some());
    let return_type = find_type_node(graph, method_node.return_type.unwrap());
    assert!(matches!(&return_type.kind, TypeKind::Named { path, .. } if path == &["i32"]));

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains Trait
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(trait_node.id()),
        RelationKind::Contains,
        "Expected ModuleNode to Contain TraitNode",
    );

    // 2. Trait Contains Method (Assuming RelationKind::TraitMethod exists)
    // NOTE: Note yet implemented
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_trait_node_complex_generic_trait_paranoid() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let trait_name = "ComplexGenericTrait";
    let relative_file_path = "src/traits.rs";
    let module_path = vec!["crate".to_string(), "traits".to_string()];

    let trait_node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );

    // --- Assertions ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;

    // Basic Node Properties
    assert!(matches!(trait_node.id(), NodeId::Synthetic(_)));
    assert!(trait_node.tracking_hash.is_some());
    assert_eq!(trait_node.name(), trait_name);
    assert_eq!(trait_node.visibility(), VisibilityKind::Public);
    assert!(trait_node.attributes.is_empty());
    assert!(trait_node.docstring.is_none());
    assert!(trait_node.super_traits.is_empty());

    // Generics <'a, T: Debug + Clone, S: Send + Sync> where T: 'a
    assert_eq!(trait_node.generic_params.len(), 3);
    // TODO: Add detailed checks for generic param kinds, bounds, and where clauses

    // Methods (fn complex_process(&'a self, item: T, other: S) -> &'a T;)
    assert_eq!(trait_node.methods.len(), 1);
    let method_node = &trait_node.methods[0];
    assert_eq!(method_node.name(), "complex_process");
    assert!(matches!(method_node.id(), NodeId::Synthetic(_)));
    assert!(method_node.tracking_hash.is_some());
    assert_eq!(method_node.parameters.len(), 3); // &'a self, item: T, other: S
    assert!(method_node.parameters[0].is_self);
    let param_t_type = find_type_node(graph, method_node.parameters[1].type_id);
    let param_s_type = find_type_node(graph, method_node.parameters[2].type_id);
    assert!(matches!(&param_t_type.kind, TypeKind::Named { path, .. } if path == &["T"]));
    assert!(matches!(&param_s_type.kind, TypeKind::Named { path, .. } if path == &["S"]));

    assert!(method_node.return_type.is_some());
    let return_type_node = find_type_node(graph, method_node.return_type.unwrap());
    // Check return type &'a T
    assert!(matches!(&return_type_node.kind, TypeKind::Reference { .. }));
    assert_eq!(return_type_node.related_types.len(), 1);
    let referenced_return_type = find_type_node(graph, return_type_node.related_types[0]);
    assert!(matches!(&referenced_return_type.kind, TypeKind::Named { path, .. } if path == &["T"]));

    // --- Paranoid Relation Checks ---
    let module_id = find_inline_module_by_path(graph, &module_path)
        .expect("Failed to find module node for relation check")
        .id();

    // 1. Module Contains Trait
    assert_relation_exists(
        graph,
        GraphId::Node(module_id),
        GraphId::Node(trait_node.id()),
        RelationKind::Contains,
        "Module->Trait",
    );

    // 2. Trait Contains Method
    // NOTE: Not yet implemented
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_other_trait_nodes() {
    let fixture_name = "fixture_nodes";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let relative_file_path = "src/traits.rs";

    // --- Find the relevant graph ---
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .expect("ParsedCodeGraph for traits.rs not found")
        .graph;

    let module_id_crate =
        find_inline_module_by_path(graph, &["crate".to_string(), "traits".to_string()])
            .expect("Failed to find top-level module node")
            .id();
    let module_id_inner = find_inline_module_by_path(
        graph,
        &[
            "crate".to_string(),
            "traits".to_string(),
            "inner".to_string(),
        ],
    )
    .expect("Failed to find inner module node")
    .id();

    // --- Test Individual Traits ---

    // InternalTrait (private)
    let trait_name = "InternalTrait";
    let module_path = vec!["crate".to_string(), "traits".to_string()];
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    assert_eq!(node.methods.len(), 1); // default_method
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );
    // TODO: Add assertion for trait method if/when implemented here for
    // "InternalTrait->default_method"

    // CrateTrait (crate visible)
    let trait_name = "CrateTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(
        node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()])
    ); // pub(crate)
    assert_eq!(node.methods.len(), 1); // crate_method
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // DocumentedTrait (documented)
    let trait_name = "DocumentedTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert!(node.docstring.is_some());
    assert_eq!(node.docstring.as_deref(), Some("Documented public trait"));
    assert_eq!(node.methods.len(), 1); // documented_method
    assert!(node.methods[0].docstring.is_some()); // Check method docstring too
    assert_eq!(
        node.methods[0].docstring.as_deref(),
        Some("Required method documentation") // Note leading whitespace already stripped
    );
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // GenericTrait<T>
    let trait_name = "GenericTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.generic_params.len(), 1);
    assert_eq!(node.methods.len(), 1); // process
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // LifetimeTrait<'a>
    let trait_name = "LifetimeTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.generic_params.len(), 1);
    // TODO: Check generic param is lifetime 'a'
    assert_eq!(node.methods.len(), 1); // get_ref
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // AssocTypeTrait
    let trait_name = "AssocTypeTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    // NOTE: Associated types are not stored directly on TraitNode yet
    assert_eq!(node.methods.len(), 1); // generate
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // AssocTypeWithBounds
    let trait_name = "AssocTypeWithBounds";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    // NOTE: Associated types are not stored directly on TraitNode yet
    assert_eq!(node.methods.len(), 1); // generate_bounded
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // AssocConstTrait
    let trait_name = "AssocConstTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    // NOTE: Associated consts are not stored directly on TraitNode yet
    assert_eq!(node.methods.len(), 1); // get_id
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // SuperTrait: SimpleTrait
    let trait_name = "SuperTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.super_traits.len(), 1);
    // Check the supertrait TypeId corresponds to SimpleTrait
    let super_trait_id = node.super_traits[0];
    let super_trait_type = find_type_node(graph, super_trait_id);
    assert!(
        matches!(&super_trait_type.kind, TypeKind::Named { path, .. } if path == &["SimpleTrait"]),
        "\nExpected path: '&[\"SimpleTrait\"]' for TypeKind::Named in TypeNode, found: 
    TypeKind::Named path:{:?}
    Complete super_trait TypeNode: 
{:#?}",
        &super_trait_type.kind,
        &super_trait_type
    );
    assert_eq!(node.methods.len(), 1); // super_method
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // MultiSuperTrait: SimpleTrait + InternalTrait + Debug
    let trait_name = "MultiSuperTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.super_traits.len(), 3);
    // TODO: Check all 3 supertrait TypeIds (SimpleTrait, InternalTrait, Debug)
    assert_eq!(node.methods.len(), 1); // multi_super_method
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // GenericSuperTrait<T>: GenericTrait<T>
    let trait_name = "GenericSuperTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.generic_params.len(), 1); // <T>
    assert_eq!(node.super_traits.len(), 1);
    // Check supertrait TypeId corresponds to GenericTrait<T>
    let super_trait_id = node.super_traits[0];
    let super_trait_type = find_type_node(graph, super_trait_id);
    assert!(
        matches!(&super_trait_type.kind, TypeKind::Named { path, .. } if path == &["GenericTrait"])
    );
    assert_eq!(super_trait_type.related_types.len(), 1); // <T>
    assert_eq!(node.methods.len(), 1); // generic_super_method
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // AttributedTrait
    let trait_name = "AttributedTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.attributes.len(), 1);
    assert_eq!(node.attributes[0].name, "must_use");
    assert_eq!(
        node.attributes[0].value.as_deref(),
        Some("Trait results should be used")
    );
    assert_eq!(node.methods.len(), 1); // calculate
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // UnsafeTrait
    let trait_name = "UnsafeTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    // TODO: Check if TraitNode has an `is_unsafe` flag - currently it doesn't seem to.
    assert_eq!(node.methods.len(), 1); // unsafe_method
                                       // TODO: Check if method_node has an `is_unsafe` flag.
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // --- Traits inside `inner` module ---
    let module_path_inner = vec![
        "crate".to_string(),
        "traits".to_string(),
        "inner".to_string(),
    ];

    // InnerSecretTrait (private in private mod)
    let trait_name = "InnerSecretTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        trait_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Inherited);
    assert_eq!(node.methods.len(), 1); // secret_op
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // InnerPublicTrait (pub in private mod)
    let trait_name = "InnerPublicTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        trait_name,
    );
    assert_eq!(node.visibility(), VisibilityKind::Public); // Public within its module
    assert_eq!(node.methods.len(), 1); // public_inner_op
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // SuperGraphNodeTrait (pub(super))
    let trait_name = "SuperGraphNodeTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path_inner,
        trait_name,
    );
    assert_eq!(
        node.visibility(),
        VisibilityKind::Restricted(vec!["super".to_string()])
    ); // pub(super)
    assert_eq!(node.super_traits.len(), 1); // super::SimpleTrait
    assert_eq!(node.methods.len(), 1); // super_visible_op
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_inner),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // --- Traits with Self usage ---
    let module_path = vec!["crate".to_string(), "traits".to_string()]; // Back to top level

    // SelfUsageTrait
    let trait_name = "SelfUsageTrait";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    assert_eq!(node.methods.len(), 2);
    // TODO: Check method signatures involving Self
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );

    // SelfInAssocBound
    let trait_name = "SelfInAssocBound";
    let node = find_trait_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        trait_name,
    );
    // NOTE: Associated types not stored on TraitNode yet
    assert_eq!(node.methods.len(), 1); // get_related
    assert_relation_exists(
        graph,
        GraphId::Node(module_id_crate),
        GraphId::Node(node.id()),
        RelationKind::Contains,
        trait_name,
    );
}
