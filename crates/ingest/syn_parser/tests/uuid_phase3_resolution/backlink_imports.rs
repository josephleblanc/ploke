#![cfg(test)]
//! Regression tests that ensure parsed definitions link back to their corresponding import nodes.
//!
//! ## Scope
//!
//! * **Fixture:** `tests/fixture_crates/fixture_nodes/src/imports.rs`
//! * **Goal:** Verify that every supported import kind (structs, unit structs, consts, statics,
//!   unions, macros, module re-exports, etc.) emits a `TreeRelation` from the defining node to the
//!   `ImportNode` in `crate::imports`.
//! * **Live Gate:** These tests enforce that the ModuleTree establishes the
//!   definition→import relation for every supported import. They should be kept up-to-date
//!   with fixture additions so regressions surface immediately.
//!
//! ## Structure & Rationale
//!
//! * Uses the shared relation-paranoid harness so each test resolves the exact definition node and
//!   exact import node, then asserts that one and only one `ImportedBy` relation links them.
//! * A lightweight macro expands into individual `#[test]` functions so each item still shows up
//!   independently in test output while sharing the cached graph/tree fixture.
//! * The parsed graph + module tree are cached via `lazy_static!` so the fixture is only parsed
//!   once, keeping the regression tests fast enough for frequent local runs or pre-commit hooks.
//! * Additional fixture imports should only require adding another `backlink_case!` entry plus
//!   referencing the item in the fixture coverage doc.

use lazy_static::lazy_static;
use ploke_core::ItemKind;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::resolve::module_tree::ModuleTree;

use crate::common::build_tree_for_tests;
use crate::common::relation_paranoid::{ExpectedTreeRelation, import, item};
use crate::paranoid_tree_relation_case;

const FIXTURE_NAME: &str = "fixture_nodes";
const DEFAULT_IMPORTS_MODULE_PATH: &[&str] = &["crate", "imports"];

lazy_static! {
    static ref BACKLINK_FIXTURE: (ParsedCodeGraph, ModuleTree) = build_tree_for_tests(FIXTURE_NAME);
}

macro_rules! backlink_case {
    ($name:ident, $path:expr, $item:expr, $kind:expr, $import:expr) => {
        paranoid_tree_relation_case!(
            $name,
            graph: &BACKLINK_FIXTURE.0,
            tree: &BACKLINK_FIXTURE.1,
            expected: ExpectedTreeRelation::ImportedBy {
                source: item($path, $item, $kind),
                target: import(DEFAULT_IMPORTS_MODULE_PATH, $import),
            }
        );
    };
}

macro_rules! backlink_case_in_module {
    ($name:ident, $path:expr, $item:expr, $kind:expr, $import_module:expr, $import:expr) => {
        paranoid_tree_relation_case!(
            $name,
            graph: &BACKLINK_FIXTURE.0,
            tree: &BACKLINK_FIXTURE.1,
            expected: ExpectedTreeRelation::ImportedBy {
                source: item($path, $item, $kind),
                target: import($import_module, $import),
            }
        );
    };
}

backlink_case!(
    struct_sample_struct_backlinks,
    &["crate", "structs"],
    "SampleStruct",
    ItemKind::Struct,
    "MySimpleStruct"
);

backlink_case!(
    struct_sample_struct_crate_vis_backlinks,
    &["crate", "structs"],
    "SampleStruct",
    ItemKind::Struct,
    "CrateVisibleStruct"
);

backlink_case!(
    unit_struct_backlinks,
    &["crate", "structs"],
    "UnitStruct",
    ItemKind::Struct,
    "UnitStruct"
);

backlink_case!(
    tuple_struct_backlinks,
    &["crate", "structs"],
    "TupleStruct",
    ItemKind::Struct,
    "TupleStruct"
);

backlink_case!(
    const_bool_backlinks,
    &["crate", "const_static"],
    "TOP_LEVEL_BOOL",
    ItemKind::Const,
    "TOP_LEVEL_BOOL"
);

backlink_case!(
    static_counter_backlinks,
    &["crate", "const_static"],
    "TOP_LEVEL_COUNTER",
    ItemKind::Static,
    "TOP_LEVEL_COUNTER"
);

backlink_case!(
    union_int_or_float_backlinks,
    &["crate", "unions"],
    "IntOrFloat",
    ItemKind::Union,
    "IntOrFloat"
);

backlink_case!(
    macro_documented_backlinks,
    &["crate", "macros"],
    "documented_macro",
    ItemKind::Macro,
    "documented_macro"
);

backlink_case!(
    module_traits_alias_backlinks,
    &["crate"],
    "traits",
    ItemKind::Module,
    "TraitsMod"
);

backlink_case!(
    enum_sample_enum1_backlinks,
    &["crate", "enums"],
    "SampleEnum1",
    ItemKind::Enum,
    "SampleEnum1"
);

backlink_case!(
    enum_enum_with_data_backlinks,
    &["crate", "enums"],
    "EnumWithData",
    ItemKind::Enum,
    "EnumWithData"
);

backlink_case!(
    trait_simple_trait_backlinks,
    &["crate", "traits"],
    "SimpleTrait",
    ItemKind::Trait,
    "SimpleTrait"
);

backlink_case_in_module!(
    trait_simple_trait_restricted_alias_backlinks,
    &["crate", "traits"],
    "SimpleTrait",
    ItemKind::Trait,
    &["crate", "imports", "sub_imports", "restricted_scope"],
    "RestrictedTraitAlias"
);

backlink_case!(
    trait_simple_trait_cfg_alias_backlinks,
    &["crate", "traits"],
    "SimpleTrait",
    ItemKind::Trait,
    "CfgTraitAlias"
);

backlink_case!(
    trait_generic_trait_alias_backlinks,
    &["crate", "traits"],
    "GenericTrait",
    ItemKind::Trait,
    "MyGenTrait"
);

backlink_case!(
    trait_glob_simple_trait_backlinks,
    &["crate", "traits"],
    "SimpleTrait",
    ItemKind::Trait,
    "crate::traits::*"
);

backlink_case!(
    trait_glob_documented_trait_backlinks,
    &["crate", "traits"],
    "DocumentedTrait",
    ItemKind::Trait,
    "crate::traits::*"
);

backlink_case!(
    trait_glob_crate_trait_backlinks,
    &["crate", "traits"],
    "CrateTrait",
    ItemKind::Trait,
    "crate::traits::*"
);

backlink_case!(
    type_alias_simple_id_backlinks,
    &["crate", "type_alias"],
    "SimpleId",
    ItemKind::TypeAlias,
    "SimpleId"
);

backlink_case!(
    cfg_struct_alias_backlinks,
    &["crate", "structs"],
    "CfgOnlyStruct",
    ItemKind::Struct,
    "CfgStructAlias"
);
