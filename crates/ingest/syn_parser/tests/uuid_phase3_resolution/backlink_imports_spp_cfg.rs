#![cfg(test)]
//! Focused `ImportedBy` tests for cfg-heavy cases in `fixture_spp_edge_cases`.
//!
//! ## Test Coverage Analysis
//!
//! * **Fixture:** `tests/fixture_crates/fixture_spp_edge_cases`
//! * **Tests:** `crates/ingest/syn_parser/tests/uuid_phase3_resolution/backlink_imports_spp_cfg.rs`
//! * **Harness:** `paranoid_imported_by_case!` backed by exact endpoint regeneration in
//!   `relation_paranoid.rs`
//!
//! ### 1. Purpose of This Suite
//!
//! This file is not a general import test suite. The no-cfg exhaustive coverage already lives in
//! `backlink_imports_spp.rs`.
//!
//! This suite exists specifically to pin down the fault line where:
//! * glob imports intersect with cfg-gated exported children
//! * cfg-duplicated root modules appear in `ImportedBy` backlink surfaces
//! * exact endpoint matching must distinguish "current parser contract" from "future desired graph"
//!
//! ### 2. Current Contract vs. Target Contract
//!
//! The parser currently evaluates cfgs using the active/default feature path rather than preserving
//! a union of all syntactically present cfg branches.
//!
//! That means the current green tests intentionally assert today's behavior:
//! * for `glob_target::*`, only the currently active cfg child should backlink into the glob import
//! * for `tests::super::*`, only currently present root modules should appear as sources
//!
//! The stronger union-style model is still important, but it is not silently blended into the
//! current contract. Instead, it is preserved as ignored canary tests in this same file.
//!
//! ### 3. What the Paranoid Harness Actually Checks
//!
//! These are not loose existence tests and they are not just counting relations.
//!
//! For each expected source or target endpoint, the harness:
//! * regenerates the exact item/import/module identity expected from fixture path, file, parent
//!   scope, item kind, and cfg context
//! * requires that endpoint selection be unique
//! * asserts exact `ImportedBy { source, target }` relations
//! * asserts exact source-set cardinality for glob imports
//!
//! So a passing test here means:
//! * the graph contains the exact node/import/module we think it should
//! * the regenerated IDs match the actual stored IDs
//! * the backlink relation exists once and only once
//!
//! ### 4. Current Green Coverage
//!
//! The current-contract tests cover:
//! * root `pub use glob_target::*;`
//!   * verifies exact visible children under current cfg-eval behavior
//!   * verifies the active `not(feature = "glob_feat_a")` branch is the one present today
//! * `tests::super::*`
//!   * verifies exact root-visible imports/modules under the current cfg-eval/default-path model
//!   * verifies cfg-sensitive root module presence for the active `cfg_mod` branch
//!
//! ### 5. Ignored Canaries
//!
//! The ignored canaries deliberately target a stronger future model:
//! * both mutually exclusive cfg children visible behind `glob_target::*`
//! * both `cfg_mod` branches and impossible-but-syntactic cfg module shapes visible from
//!   `tests::super::*`
//!
//! These remain ignored until the resolver actually supports that broader graph contract.
//!
//! ### 6. Known Sharp Edges
//!
//! * cfg strings are matched exactly as stored by the parser, including formatting such as
//!   `not (feature = "...")`
//! * glob imports do not materialize as one import node per child; there is one glob `ImportNode`
//!   with many `ImportedBy` sources
//! * the debug helpers in this file are intentionally ignored and exist only to dump actual graph
//!   shape when the cfg surface shifts

use lazy_static::lazy_static;
use ploke_core::ItemKind;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::parser::graph::GraphAccess;
use syn_parser::parser::relations::SyntacticRelation;
use syn_parser::resolve::module_tree::ModuleTree;

use crate::common::build_tree_for_tests;
use crate::common::relation_paranoid::{
    ImportParanoidArgs, ModuleParanoidArgs, ModuleParanoidKind, paranoid_import, paranoid_item,
    paranoid_module,
};
use crate::common::{ParanoidArgs, run_phases_and_collect};
use crate::paranoid_imported_by_case;

const FIXTURE_NAME: &str = "fixture_spp_edge_cases";
const LIB_RS: &str = "src/lib.rs";
const GLOB_TARGET_MOD_RS: &str = "src/glob_target/mod.rs";

lazy_static! {
    static ref PARSED_FIXTURE: Vec<ParsedCodeGraph> = run_phases_and_collect(FIXTURE_NAME);
    static ref BACKLINK_FIXTURE: (ParsedCodeGraph, ModuleTree) = build_tree_for_tests(FIXTURE_NAME);
}

macro_rules! target_import {
    ($file:expr, $module_path:expr, $visible_name:expr, $source_path:expr, $original_name:expr, $is_glob:expr) => {
        ImportParanoidArgs {
            fixture: FIXTURE_NAME,
            relative_file_path: $file,
            expected_module_path: $module_path,
            visible_name: $visible_name,
            expected_path: $source_path,
            expected_original_name: $original_name,
            expected_is_glob: $is_glob,
        }
    };
}

macro_rules! item_src {
    ($file:expr, $path:expr, $ident:expr, $kind:expr) => {
        paranoid_item(ParanoidArgs {
            fixture: FIXTURE_NAME,
            relative_file_path: $file,
            expected_path: $path,
            ident: $ident,
            item_kind: $kind,
            expected_cfg: None,
        })
    };
}

macro_rules! item_src_cfg {
    ($file:expr, $path:expr, $ident:expr, $kind:expr, $cfg:expr) => {
        paranoid_item(ParanoidArgs {
            fixture: FIXTURE_NAME,
            relative_file_path: $file,
            expected_path: $path,
            ident: $ident,
            item_kind: $kind,
            expected_cfg: Some($cfg),
        })
    };
}

macro_rules! import_src {
    ($file:expr, $module_path:expr, $visible_name:expr, $source_path:expr, $original_name:expr, $is_glob:expr) => {
        paranoid_import(target_import!(
            $file,
            $module_path,
            $visible_name,
            $source_path,
            $original_name,
            $is_glob
        ))
    };
}

macro_rules! inline_mod_src {
    ($file:expr, $module_path:expr) => {
        paranoid_module(ModuleParanoidArgs {
            fixture: FIXTURE_NAME,
            relative_file_path: $file,
            expected_module_path: $module_path,
            kind: ModuleParanoidKind::InlineDefinition,
            expected_cfg: None,
        })
    };
}

macro_rules! inline_mod_src_cfg {
    ($file:expr, $module_path:expr, $cfg:expr) => {
        paranoid_module(ModuleParanoidArgs {
            fixture: FIXTURE_NAME,
            relative_file_path: $file,
            expected_module_path: $module_path,
            kind: ModuleParanoidKind::InlineDefinition,
            expected_cfg: Some($cfg),
        })
    };
}

macro_rules! decl_mod_src {
    ($file:expr, $module_path:expr) => {
        paranoid_module(ModuleParanoidArgs {
            fixture: FIXTURE_NAME,
            relative_file_path: $file,
            expected_module_path: $module_path,
            kind: ModuleParanoidKind::Declaration,
            expected_cfg: None,
        })
    };
}

macro_rules! spp_cfg_import_case {
    ($name:ident, $target:expr, [$($expected_source:expr),* $(,)?]) => {
        paranoid_imported_by_case!(
            $name,
            parsed_graphs: &PARSED_FIXTURE,
            tree: &BACKLINK_FIXTURE.1,
            target: $target,
            expected_sources: [$($expected_source),*]
        );
    };
}

spp_cfg_import_case!(
    root_glob_target_glob_import_has_exact_current_cfg_visible_children,
    target_import!(
        LIB_RS,
        &["crate"],
        "glob_target::*",
        &["glob_target"],
        None,
        true
    ),
    [
        item_src!(
            GLOB_TARGET_MOD_RS,
            &["crate", "glob_target"],
            "glob_public_item",
            ItemKind::Function
        ),
        item_src!(
            GLOB_TARGET_MOD_RS,
            &["crate", "glob_target"],
            "glob_crate_item",
            ItemKind::Function
        ),
        decl_mod_src!(
            GLOB_TARGET_MOD_RS,
            &["crate", "glob_target", "glob_sub_path"]
        ),
        item_src_cfg!(
            GLOB_TARGET_MOD_RS,
            &["crate", "glob_target"],
            "glob_item_cfg_not_a",
            ItemKind::Function,
            &["not (feature = \"glob_feat_a\")"]
        ),
        inline_mod_src!(
            GLOB_TARGET_MOD_RS,
            &["crate", "glob_target", "pub_sub_with_restricted"]
        )
    ]
);

spp_cfg_import_case!(
    tests_super_glob_import_includes_only_current_cfg_root_modules,
    target_import!(
        LIB_RS,
        &["crate", "tests"],
        "super::*",
        &["super"],
        None,
        true
    ),
    [
        item_src!(LIB_RS, &["crate"], "add", ItemKind::Function),
        import_src!(
            LIB_RS,
            &["crate"],
            "item_c",
            &["chain_c", "item_c"],
            None,
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "item_alt_d",
            &["chain_alt_d", "item_alt_d"],
            None,
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "glob_target::*",
            &["glob_target"],
            None,
            true
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "final_deep_item",
            &["deep11", "item11"],
            Some("item11"),
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "item_via_a",
            &["branch_a", "branch_item"],
            Some("branch_item"),
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "item_via_b",
            &["branch_b", "branch_item"],
            Some("branch_item"),
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "final_renamed_item",
            &["rename_step2", "renamed2"],
            Some("renamed2"),
            false
        ),
        inline_mod_src_cfg!(LIB_RS, &["crate", "cfg_mod"], &["feature = \"cfg_a\""]),
        inline_mod_src!(LIB_RS, &["crate", "inline_path_mod"]),
        decl_mod_src!(LIB_RS, &["crate", "logical_mod_1"]),
        decl_mod_src!(LIB_RS, &["crate", "nested_path_1"])
    ]
);

#[test]
#[ignore = "debug helper"]
fn debug_tests_super_glob_backlinks() {
    let graph = &BACKLINK_FIXTURE.0.graph;
    let tree = &BACKLINK_FIXTURE.1;

    let target = graph
        .use_statements()
        .iter()
        .find(|imp| {
            imp.visible_name == "super::*"
                && imp.source_path == vec!["super".to_string()]
                && imp.is_glob
        })
        .expect("tests::super::* import not found");

    eprintln!(
        "\nImportedBy sources for tests glob import {} (id={}):",
        target.visible_name, target.id
    );
    for relation in tree.tree_relations() {
        match relation.rel() {
            SyntacticRelation::ImportedBy {
                source,
                target: relation_target,
            } if *relation_target == target.id => {
                let source_any = (*source).into();
                let node = graph.find_any_node_checked(source_any).unwrap();
                eprintln!(
                    "  source id={} name={} kind={:?} cfgs={:?}",
                    source_any,
                    node.name(),
                    node.kind(),
                    node.cfgs()
                );
            }
            _ => {}
        }
    }
}

#[test]
#[ignore = "future canary: union-style cfg graph is not the current contract"]
fn root_glob_target_glob_import_union_cfg_canary() {
    crate::common::relation_paranoid::assert_imported_by_sources_exact(
        &PARSED_FIXTURE,
        &BACKLINK_FIXTURE.1,
        target_import!(
            LIB_RS,
            &["crate"],
            "glob_target::*",
            &["glob_target"],
            None,
            true
        ),
        &[
            item_src!(
                GLOB_TARGET_MOD_RS,
                &["crate", "glob_target"],
                "glob_public_item",
                ItemKind::Function
            ),
            item_src!(
                GLOB_TARGET_MOD_RS,
                &["crate", "glob_target"],
                "glob_crate_item",
                ItemKind::Function
            ),
            decl_mod_src!(
                GLOB_TARGET_MOD_RS,
                &["crate", "glob_target", "glob_sub_path"]
            ),
            item_src_cfg!(
                GLOB_TARGET_MOD_RS,
                &["crate", "glob_target"],
                "glob_item_cfg_a",
                ItemKind::Function,
                &["feature = \"glob_feat_a\""]
            ),
            item_src_cfg!(
                GLOB_TARGET_MOD_RS,
                &["crate", "glob_target"],
                "glob_item_cfg_not_a",
                ItemKind::Function,
                &["not (feature = \"glob_feat_a\")"]
            ),
            inline_mod_src!(
                GLOB_TARGET_MOD_RS,
                &["crate", "glob_target", "pub_sub_with_restricted"]
            ),
        ],
    )
    .unwrap();
}

#[test]
#[ignore = "future canary: union-style cfg graph is not the current contract"]
fn tests_super_glob_import_union_cfg_root_modules_canary() {
    crate::common::relation_paranoid::assert_imported_by_sources_exact(
        &PARSED_FIXTURE,
        &BACKLINK_FIXTURE.1,
        target_import!(
            LIB_RS,
            &["crate", "tests"],
            "super::*",
            &["super"],
            None,
            true
        ),
        &[
            item_src!(LIB_RS, &["crate"], "add", ItemKind::Function),
            import_src!(
                LIB_RS,
                &["crate"],
                "item_c",
                &["chain_c", "item_c"],
                None,
                false
            ),
            import_src!(
                LIB_RS,
                &["crate"],
                "item_alt_d",
                &["chain_alt_d", "item_alt_d"],
                None,
                false
            ),
            import_src!(
                LIB_RS,
                &["crate"],
                "glob_target::*",
                &["glob_target"],
                None,
                true
            ),
            import_src!(
                LIB_RS,
                &["crate"],
                "final_deep_item",
                &["deep11", "item11"],
                Some("item11"),
                false
            ),
            import_src!(
                LIB_RS,
                &["crate"],
                "item_via_a",
                &["branch_a", "branch_item"],
                Some("branch_item"),
                false
            ),
            import_src!(
                LIB_RS,
                &["crate"],
                "item_via_b",
                &["branch_b", "branch_item"],
                Some("branch_item"),
                false
            ),
            import_src!(
                LIB_RS,
                &["crate"],
                "final_renamed_item",
                &["rename_step2", "renamed2"],
                Some("renamed2"),
                false
            ),
            inline_mod_src_cfg!(LIB_RS, &["crate", "cfg_mod"], &["feature = \"cfg_a\""]),
            inline_mod_src_cfg!(
                LIB_RS,
                &["crate", "cfg_mod"],
                &["not (feature = \"cfg_a\")"]
            ),
            inline_mod_src_cfg!(
                LIB_RS,
                &["crate", "conflict_parent"],
                &["feature = \"cfg_conflict\""]
            ),
        ],
    )
    .unwrap();
}

#[test]
#[ignore = "debug helper"]
fn debug_cfg_root_modules_and_glob_backlinks() {
    let graph = &BACKLINK_FIXTURE.0.graph;
    let tree = &BACKLINK_FIXTURE.1;

    eprintln!("root-level non-declaration modules in merged graph:");
    for module in graph
        .modules()
        .iter()
        .filter(|m| !m.is_decl() && m.path == vec!["crate".to_string(), m.name.clone()])
    {
        eprintln!(
            "  module name={} path={:?} kind={} cfgs={:?} id={}",
            module.name,
            module.path,
            if module.is_inline() {
                "inline"
            } else if module.is_file_based() {
                "file"
            } else {
                "decl"
            },
            module.cfgs,
            module.id
        );
    }

    let target = graph
        .use_statements()
        .iter()
        .find(|imp| {
            imp.visible_name == "glob_target::*"
                && imp.source_path == vec!["glob_target".to_string()]
                && imp.is_glob
        })
        .expect("glob_target::* import not found");

    eprintln!(
        "\nImportedBy sources for root glob import {} (id={}):",
        target.visible_name, target.id
    );
    for relation in tree.tree_relations() {
        match relation.rel() {
            SyntacticRelation::ImportedBy {
                source,
                target: relation_target,
            } if *relation_target == target.id => {
                let source_any = (*source).into();
                let node = graph.find_any_node_checked(source_any).unwrap();
                eprintln!(
                    "  source id={} name={} cfgs={:?}",
                    source_any,
                    node.name(),
                    node.cfgs()
                );
            }
            _ => {}
        }
    }

    panic!("debug helper");
}
