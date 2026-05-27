#![cfg(test)]
//! Exhaustive ImportedBy tests for every `use` statement in `fixture_spp_edge_cases_no_cfg`.
//!
//! The point of this suite is not to guess at a couple of representative cases. It is to pin the
//! exact relation surface for the entire fixture so failures show us the boundary of current import
//! backlink correctness.

use lazy_static::lazy_static;
use ploke_core::ItemKind;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::resolve::module_tree::ModuleTree;

use crate::common::build_tree_for_tests;
use crate::common::relation_paranoid::{
    ImportParanoidArgs, ModuleParanoidArgs, ModuleParanoidKind, assert_imported_by_sources_exact,
    paranoid_import, paranoid_item, paranoid_module,
};
use crate::common::{ParanoidArgs, run_phases_and_collect};
use crate::paranoid_imported_by_case;

const FIXTURE_NAME: &str = "fixture_spp_edge_cases_no_cfg";
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

macro_rules! spp_import_case {
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

// --- Scenario 1: Multi-step re-export chains ---
spp_import_case!(
    chain_b_item_b_imported_by_chain_a_item_a,
    target_import!(
        LIB_RS,
        &["crate", "chain_b"],
        "item_b",
        &["crate", "chain_a", "item_a"],
        Some("item_a"),
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "chain_a"],
        "item_a",
        ItemKind::Function
    )]
);

spp_import_case!(
    chain_c_item_c_imported_by_chain_b_item_b,
    target_import!(
        LIB_RS,
        &["crate", "chain_c"],
        "item_c",
        &["crate", "chain_b", "item_b"],
        Some("item_b"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "chain_b"],
        "item_b",
        &["crate", "chain_a", "item_a"],
        Some("item_a"),
        false
    )]
);

spp_import_case!(
    root_item_c_imported_by_chain_c_item_c,
    target_import!(
        LIB_RS,
        &["crate"],
        "item_c",
        &["chain_c", "item_c"],
        None,
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "chain_c"],
        "item_c",
        &["crate", "chain_b", "item_b"],
        Some("item_b"),
        false
    )]
);

spp_import_case!(
    chain_alt_b_item_a_imported_by_chain_a_item_a,
    target_import!(
        LIB_RS,
        &["crate", "chain_alt_b"],
        "item_a",
        &["crate", "chain_a", "item_a"],
        None,
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "chain_a"],
        "item_a",
        ItemKind::Function
    )]
);

spp_import_case!(
    chain_alt_c_item_a_imported_by_chain_alt_b_item_a,
    target_import!(
        LIB_RS,
        &["crate", "chain_alt_c"],
        "item_a",
        &["crate", "chain_alt_b", "item_a"],
        None,
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "chain_alt_b"],
        "item_a",
        &["crate", "chain_a", "item_a"],
        None,
        false
    )]
);

spp_import_case!(
    chain_alt_d_item_alt_d_imported_by_chain_alt_c_item_a,
    target_import!(
        LIB_RS,
        &["crate", "chain_alt_d"],
        "item_alt_d",
        &["crate", "chain_alt_c", "item_a"],
        Some("item_a"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "chain_alt_c"],
        "item_a",
        &["crate", "chain_alt_b", "item_a"],
        None,
        false
    )]
);

spp_import_case!(
    root_item_alt_d_imported_by_chain_alt_d_item_alt_d,
    target_import!(
        LIB_RS,
        &["crate"],
        "item_alt_d",
        &["chain_alt_d", "item_alt_d"],
        None,
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "chain_alt_d"],
        "item_alt_d",
        &["crate", "chain_alt_c", "item_a"],
        Some("item_a"),
        false
    )]
);

// --- Scenario 4: glob re-export at root ---
spp_import_case!(
    root_glob_target_glob_import_has_exact_visible_children,
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
        inline_mod_src!(
            GLOB_TARGET_MOD_RS,
            &["crate", "glob_target", "pub_sub_with_restricted"]
        )
    ]
);

// --- Scenario 7: relative re-exports ---
spp_import_case!(
    relative_inner_reexport_super_imported_by_parent_item,
    target_import!(
        LIB_RS,
        &["crate", "relative", "inner"],
        "reexport_super",
        &["super", "item_in_relative"],
        Some("item_in_relative"),
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "relative"],
        "item_in_relative",
        ItemKind::Function
    )]
);

spp_import_case!(
    relative_reexport_self_imported_by_inner_item,
    target_import!(
        LIB_RS,
        &["crate", "relative"],
        "reexport_self",
        &["self", "inner", "item_in_inner"],
        Some("item_in_inner"),
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "relative", "inner"],
        "item_in_inner",
        ItemKind::Function
    )]
);

spp_import_case!(
    relative_pub_item_in_private_inner_imported_by_private_inner_item,
    target_import!(
        LIB_RS,
        &["crate", "relative"],
        "pub_item_in_private_inner",
        &["self", "inner_private", "pub_item_in_private_inner"],
        None,
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "relative", "inner_private"],
        "pub_item_in_private_inner",
        ItemKind::Function
    )]
);

// --- Scenario 8: deep re-export chain ---
spp_import_case!(
    deep2_item2_imported_by_deep1_item,
    target_import!(
        LIB_RS,
        &["crate", "deep2"],
        "item2",
        &["crate", "deep1", "deep_item"],
        Some("deep_item"),
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "deep1"],
        "deep_item",
        ItemKind::Function
    )]
);
spp_import_case!(
    deep3_item3_imported_by_deep2_item2,
    target_import!(
        LIB_RS,
        &["crate", "deep3"],
        "item3",
        &["crate", "deep2", "item2"],
        Some("item2"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "deep2"],
        "item2",
        &["crate", "deep1", "deep_item"],
        Some("deep_item"),
        false
    )]
);
spp_import_case!(
    deep4_item4_imported_by_deep3_item3,
    target_import!(
        LIB_RS,
        &["crate", "deep4"],
        "item4",
        &["crate", "deep3", "item3"],
        Some("item3"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "deep3"],
        "item3",
        &["crate", "deep2", "item2"],
        Some("item2"),
        false
    )]
);
spp_import_case!(
    deep5_item5_imported_by_deep4_item4,
    target_import!(
        LIB_RS,
        &["crate", "deep5"],
        "item5",
        &["crate", "deep4", "item4"],
        Some("item4"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "deep4"],
        "item4",
        &["crate", "deep3", "item3"],
        Some("item3"),
        false
    )]
);
spp_import_case!(
    deep6_item6_imported_by_deep5_item5,
    target_import!(
        LIB_RS,
        &["crate", "deep6"],
        "item6",
        &["crate", "deep5", "item5"],
        Some("item5"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "deep5"],
        "item5",
        &["crate", "deep4", "item4"],
        Some("item4"),
        false
    )]
);
spp_import_case!(
    deep7_item7_imported_by_deep6_item6,
    target_import!(
        LIB_RS,
        &["crate", "deep7"],
        "item7",
        &["crate", "deep6", "item6"],
        Some("item6"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "deep6"],
        "item6",
        &["crate", "deep5", "item5"],
        Some("item5"),
        false
    )]
);
spp_import_case!(
    deep8_item8_imported_by_deep7_item7,
    target_import!(
        LIB_RS,
        &["crate", "deep8"],
        "item8",
        &["crate", "deep7", "item7"],
        Some("item7"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "deep7"],
        "item7",
        &["crate", "deep6", "item6"],
        Some("item6"),
        false
    )]
);
spp_import_case!(
    deep9_item9_imported_by_deep8_item8,
    target_import!(
        LIB_RS,
        &["crate", "deep9"],
        "item9",
        &["crate", "deep8", "item8"],
        Some("item8"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "deep8"],
        "item8",
        &["crate", "deep7", "item7"],
        Some("item7"),
        false
    )]
);
spp_import_case!(
    deep10_item10_imported_by_deep9_item9,
    target_import!(
        LIB_RS,
        &["crate", "deep10"],
        "item10",
        &["crate", "deep9", "item9"],
        Some("item9"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "deep9"],
        "item9",
        &["crate", "deep8", "item8"],
        Some("item8"),
        false
    )]
);
spp_import_case!(
    deep11_item11_imported_by_deep10_item10,
    target_import!(
        LIB_RS,
        &["crate", "deep11"],
        "item11",
        &["crate", "deep10", "item10"],
        Some("item10"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "deep10"],
        "item10",
        &["crate", "deep9", "item9"],
        Some("item9"),
        false
    )]
);

// --- Visibility/import chain in module a ---
spp_import_case!(
    root_func_c_imported_by_a_b_c_func,
    target_import!(
        LIB_RS,
        &["crate"],
        "func_c",
        &["a", "b", "c", "func"],
        Some("func"),
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "a", "b", "c"],
        "func",
        ItemKind::Function
    )]
);

spp_import_case!(
    root_pub_func_imported_by_a_pub_func_import,
    target_import!(
        LIB_RS,
        &["crate"],
        "pub_func",
        &["a", "pub_func"],
        None,
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "a"],
        "pub_func",
        &["b", "c", "func"],
        Some("func"),
        false
    )]
);

spp_import_case!(
    root_priv_func_imported_by_a_priv_func_import,
    target_import!(
        LIB_RS,
        &["crate"],
        "priv_func",
        &["a", "priv_func"],
        None,
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "a"],
        "priv_func",
        &["b_private", "c", "func"],
        Some("func"),
        false
    )]
);

spp_import_case!(
    root_very_public_imported_by_root_priv_func_import,
    target_import!(
        LIB_RS,
        &["crate"],
        "very_public",
        &["priv_func"],
        Some("priv_func"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate"],
        "priv_func",
        &["a", "priv_func"],
        None,
        false
    )]
);

spp_import_case!(
    root_crate_func_imported_by_a_priv_func_import,
    target_import!(
        LIB_RS,
        &["crate"],
        "crate_func",
        &["a", "priv_func"],
        Some("priv_func"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "a"],
        "priv_func",
        &["b_private", "c", "func"],
        Some("func"),
        false
    )]
);

spp_import_case!(
    a_pub_func_imported_by_a_b_c_func,
    target_import!(
        LIB_RS,
        &["crate", "a"],
        "pub_func",
        &["b", "c", "func"],
        Some("func"),
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "a", "b", "c"],
        "func",
        ItemKind::Function
    )]
);

spp_import_case!(
    a_crate_func_named_func_imported_by_a_b_c_func,
    target_import!(
        LIB_RS,
        &["crate", "a"],
        "func",
        &["b", "c", "func"],
        None,
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "a", "b", "c"],
        "func",
        ItemKind::Function
    )]
);

spp_import_case!(
    a_b_func_imported_by_a_b_c_func,
    target_import!(
        LIB_RS,
        &["crate", "a", "b"],
        "func",
        &["c", "func"],
        None,
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "a", "b", "c"],
        "func",
        ItemKind::Function
    )]
);

spp_import_case!(
    a_priv_func_imported_by_a_b_private_c_func,
    target_import!(
        LIB_RS,
        &["crate", "a"],
        "priv_func",
        &["b_private", "c", "func"],
        Some("func"),
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "a", "b_private", "c"],
        "func",
        ItemKind::Function
    )]
);

spp_import_case!(
    a_b_private_func_imported_by_a_b_private_c_func,
    target_import!(
        LIB_RS,
        &["crate", "a", "b_private"],
        "func",
        &["c", "func"],
        None,
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "a", "b_private", "c"],
        "func",
        ItemKind::Function
    )]
);

// --- Remaining chains at root ---
spp_import_case!(
    root_final_deep_item_imported_by_deep11_item11_import,
    target_import!(
        LIB_RS,
        &["crate"],
        "final_deep_item",
        &["deep11", "item11"],
        Some("item11"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "deep11"],
        "item11",
        &["crate", "deep10", "item10"],
        Some("item10"),
        false
    )]
);

spp_import_case!(
    branch_a_branch_item_imported_by_branch_source_item,
    target_import!(
        LIB_RS,
        &["crate", "branch_a"],
        "branch_item",
        &["crate", "branch_source", "branch_item"],
        None,
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "branch_source"],
        "branch_item",
        ItemKind::Function
    )]
);

spp_import_case!(
    branch_b_branch_item_imported_by_branch_source_item,
    target_import!(
        LIB_RS,
        &["crate", "branch_b"],
        "branch_item",
        &["crate", "branch_source", "branch_item"],
        None,
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "branch_source"],
        "branch_item",
        ItemKind::Function
    )]
);

spp_import_case!(
    private_intermediate_branch_item_imported_by_branch_a_import,
    target_import!(
        LIB_RS,
        &["crate", "private_intermediate"],
        "branch_item",
        &["crate", "branch_a", "branch_item"],
        None,
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "branch_a"],
        "branch_item",
        &["crate", "branch_source", "branch_item"],
        None,
        false
    )]
);

spp_import_case!(
    branch_c_item_c_imported_by_private_intermediate_import,
    target_import!(
        LIB_RS,
        &["crate", "branch_c"],
        "item_c",
        &["crate", "private_intermediate", "branch_item"],
        Some("branch_item"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "private_intermediate"],
        "branch_item",
        &["crate", "branch_a", "branch_item"],
        None,
        false
    )]
);

spp_import_case!(
    root_item_via_a_imported_by_branch_a_import,
    target_import!(
        LIB_RS,
        &["crate"],
        "item_via_a",
        &["branch_a", "branch_item"],
        Some("branch_item"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "branch_a"],
        "branch_item",
        &["crate", "branch_source", "branch_item"],
        None,
        false
    )]
);

spp_import_case!(
    root_item_via_b_imported_by_branch_b_import,
    target_import!(
        LIB_RS,
        &["crate"],
        "item_via_b",
        &["branch_b", "branch_item"],
        Some("branch_item"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "branch_b"],
        "branch_item",
        &["crate", "branch_source", "branch_item"],
        None,
        false
    )]
);

spp_import_case!(
    rename_step1_renamed1_imported_by_rename_source_item,
    target_import!(
        LIB_RS,
        &["crate", "rename_step1"],
        "renamed1",
        &["crate", "rename_source", "multi_rename_item"],
        Some("multi_rename_item"),
        false
    ),
    [item_src!(
        LIB_RS,
        &["crate", "rename_source"],
        "multi_rename_item",
        ItemKind::Function
    )]
);

spp_import_case!(
    rename_step2_renamed2_imported_by_rename_step1_import,
    target_import!(
        LIB_RS,
        &["crate", "rename_step2"],
        "renamed2",
        &["crate", "rename_step1", "renamed1"],
        Some("renamed1"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "rename_step1"],
        "renamed1",
        &["crate", "rename_source", "multi_rename_item"],
        Some("multi_rename_item"),
        false
    )]
);

spp_import_case!(
    root_final_renamed_item_imported_by_rename_step2_import,
    target_import!(
        LIB_RS,
        &["crate"],
        "final_renamed_item",
        &["rename_step2", "renamed2"],
        Some("renamed2"),
        false
    ),
    [import_src!(
        LIB_RS,
        &["crate", "rename_step2"],
        "renamed2",
        &["crate", "rename_step1", "renamed1"],
        Some("renamed1"),
        false
    )]
);

// --- Exhaustive `use super::*` cases in cfg(test) modules ---
spp_import_case!(
    vis_tests_super_glob_imports_all_visible_root_children,
    target_import!(
        LIB_RS,
        &["crate", "vis_tests"],
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
            "func_c",
            &["a", "b", "c", "func"],
            Some("func"),
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "pub_func",
            &["a", "pub_func"],
            None,
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "priv_func",
            &["a", "priv_func"],
            None,
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "very_public",
            &["priv_func"],
            Some("priv_func"),
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "crate_func",
            &["a", "priv_func"],
            Some("priv_func"),
            false
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
        inline_mod_src!(LIB_RS, &["crate", "inline_path_mod"]),
        decl_mod_src!(LIB_RS, &["crate", "logical_mod_1"]),
        decl_mod_src!(LIB_RS, &["crate", "nested_path_1"])
    ]
);

spp_import_case!(
    tests_super_glob_imports_all_visible_root_children,
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
            "func_c",
            &["a", "b", "c", "func"],
            Some("func"),
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "pub_func",
            &["a", "pub_func"],
            None,
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "priv_func",
            &["a", "priv_func"],
            None,
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "very_public",
            &["priv_func"],
            Some("priv_func"),
            false
        ),
        import_src!(
            LIB_RS,
            &["crate"],
            "crate_func",
            &["a", "priv_func"],
            Some("priv_func"),
            false
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
        inline_mod_src!(LIB_RS, &["crate", "inline_path_mod"]),
        decl_mod_src!(LIB_RS, &["crate", "logical_mod_1"]),
        decl_mod_src!(LIB_RS, &["crate", "nested_path_1"])
    ]
);

#[test]
#[should_panic]
fn fails_when_target_import_node_is_not_present() {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None)
        .try_init();

    assert_imported_by_sources_exact(
        &PARSED_FIXTURE,
        &BACKLINK_FIXTURE.1,
        target_import!(
            LIB_RS,
            &["crate", "chain_b"],
            "missing_item",
            &["crate", "chain_a", "item_a"],
            Some("item_a"),
            false
        ),
        &[item_src!(
            LIB_RS,
            &["crate", "chain_a"],
            "item_a",
            ItemKind::Function
        )],
    )
    .expect("missing target import should panic before returning Ok");
}

#[test]
#[should_panic]
fn fails_when_expected_source_node_is_not_present() {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None)
        .try_init();

    assert_imported_by_sources_exact(
        &PARSED_FIXTURE,
        &BACKLINK_FIXTURE.1,
        target_import!(
            LIB_RS,
            &["crate", "chain_b"],
            "item_b",
            &["crate", "chain_a", "item_a"],
            Some("item_a"),
            false
        ),
        &[item_src!(
            LIB_RS,
            &["crate", "chain_a"],
            "missing_item",
            ItemKind::Function
        )],
    )
    .expect("missing expected source should panic before returning Ok");
}
