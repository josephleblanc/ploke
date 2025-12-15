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
//! * A single helper (`expect_backlink_for_item`) asserts that a relation exists from a definition
//!   identified by module path/name/kind to the targeted import node.
//! * A lightweight macro (`backlink_case!`) expands into individual `#[test]` functions so that
//!   each item still shows up independently in test output, while avoiding duplicated boilerplate.
//! * The parsed graph + module tree are cached via `lazy_static!` so the fixture is only parsed
//!   once, keeping the regression tests fast enough for frequent local runs or pre-commit hooks.
//! * Additional fixture imports should only require adding another `backlink_case!` entry plus
//!   referencing the item in the fixture coverage doc.

use env_logger;
use lazy_static::lazy_static;
use ploke_core::ItemKind;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::nodes::AnyNodeId;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::resolve::module_tree::ModuleTree;

use crate::common::build_tree_for_tests;
use crate::common::resolution::find_item_id_by_path_name_kind_checked;

const FIXTURE_NAME: &str = "fixture_nodes";
const DEFAULT_IMPORTS_MODULE_PATH: &[&str] = &["crate", "imports"];

lazy_static! {
    static ref BACKLINK_FIXTURE: (ParsedCodeGraph, ModuleTree) = build_tree_for_tests(FIXTURE_NAME);
}

fn expect_backlink_for_item(
    definition_module_path: &[&str],
    definition_name: &str,
    definition_kind: ItemKind,
    import_module_path: &[&str],
    import_visible_name: &str,
) {
    let _ = env_logger::builder().is_test(true).try_init();
    let (graph, tree) = &*BACKLINK_FIXTURE;

    let def_any_id = find_item_id_by_path_name_kind_checked(
        graph,
        definition_module_path,
        definition_name,
        definition_kind,
    )
    .unwrap_or_else(|err| {
        panic!(
            "Failed to locate definition {}::{definition_name} ({definition_kind:?}): {err:?}",
            definition_module_path.join("::"),
        )
    });

    let imports_module = graph
        .find_module_by_path_checked(&module_path_vec(import_module_path))
        .expect("imports module path should exist in fixture");

    let import = imports_module
        .imports
        .iter()
        .find(|imp| imp.visible_name == import_visible_name)
        .unwrap_or_else(|| {
            panic!(
                "Import `{}` not found in module {:?}",
                import_visible_name, import_module_path
            )
        });

    let import_any_id = AnyNodeId::from(import.id);

    let has_backlink = tree.tree_relations().iter().any(|tr| {
        let rel = tr.rel();
        rel.source() == def_any_id && rel.target() == import_any_id
    });

    assert!(
        has_backlink,
        "Expected backlink from definition {}::{definition_name} ({definition_kind:?}) to import `{}`.",
        definition_module_path.join("::"),
        import_visible_name
    );
}

macro_rules! backlink_case {
    ($name:ident, $path:expr, $item:expr, $kind:expr, $import:expr) => {
        #[test]
        fn $name() {
            expect_backlink_for_item($path, $item, $kind, DEFAULT_IMPORTS_MODULE_PATH, $import);
        }
    };
}

macro_rules! backlink_case_in_module {
    ($name:ident, $path:expr, $item:expr, $kind:expr, $import_module:expr, $import:expr) => {
        #[test]
        fn $name() {
            expect_backlink_for_item($path, $item, $kind, $import_module, $import);
        }
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

fn module_path_vec(path: &[&str]) -> Vec<String> {
    path.iter().map(|seg| (*seg).to_string()).collect()
}
