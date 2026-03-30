//! Tests for [`syn_parser::try_run_phases_union_for_crate`] and
//! [`syn_parser::try_run_phases_union_for_crate_with_dimensions`].
//!
//! `try_run_phases_union_for_crate` delegates to `_with_dimensions` with
//! [`syn_parser::compilation_unit::CompilationUnitDimensionRequest::from_env_or_default`].
//! Assertions that depend on an exact CU key list use `_with_dimensions` with
//! [`syn_parser::compilation_unit::CompilationUnitDimensionRequest::baseline_default`] so tests stay
//! independent of `PLOKE_CU_*` environment variables.
//!
//! **Fixture choice:** [`merge_union_graph_and_prune_tree`](syn_parser::parser::ParsedCodeGraph::merge_union_graph_and_prune_tree)
//! currently errors on crates with **both** a library and a binary root (`fixture_multi_target_cu`):
//! duplicate logical module path `crate` when merging multiple target roots. Until that is resolved,
//! these tests use **`fixture_cfg_cu`** (lib-only) so the union pipeline completes.

use std::collections::HashSet;

use ploke_common::fixtures_crates_dir;
use syn_parser::compilation_unit::{
    CompilationUnitDimensionRequest, compilation_unit_keys_for_targets, default_target_triple,
};
use syn_parser::parser::graph::GraphAccess;
use syn_parser::{
    try_run_phases_and_resolve, try_run_phases_union_for_crate,
    try_run_phases_union_for_crate_with_dimensions,
};

fn fixture_cfg_cu_root() -> std::path::PathBuf {
    fixtures_crates_dir().join("fixture_cfg_cu")
}

// `try_run_phases_union_for_crate_with_dimensions` + baseline dimensions: merged graph, tree,
// parsed_graphs_for_masks, and compilation_units are all present; merged graph includes lib symbols.
#[test]
fn union_baseline_fields() {
    let crate_root = fixture_cfg_cu_root();
    let mut out = try_run_phases_union_for_crate_with_dimensions(
        &crate_root,
        &CompilationUnitDimensionRequest::baseline_default(),
    )
    .expect("union parse with baseline dimensions");

    let merged = out.merged_graph.take().expect("merged graph");
    out.module_tree.take().expect("module tree");
    out.parsed_graphs_for_masks
        .take()
        .expect("parsed_graphs_for_masks");
    out.compilation_units.take().expect("compilation_units");

    let fn_names: HashSet<_> = merged.functions().iter().map(|f| f.name.as_str()).collect();
    assert!(
        fn_names.contains("always_present"),
        "merged union graph should include lib crate symbols, got {fn_names:?}"
    );
}

// `parsed_graphs_for_masks` is the same file-graph set as `try_run_phases_and_resolve` (length + paths).
#[test]
fn union_masks_resolve() {
    let crate_root = fixture_cfg_cu_root();
    let resolve = try_run_phases_and_resolve(&crate_root).expect("resolve phase");
    let mut union_out = try_run_phases_union_for_crate_with_dimensions(
        &crate_root,
        &CompilationUnitDimensionRequest::baseline_default(),
    )
    .expect("union parse");

    let masks = union_out
        .parsed_graphs_for_masks
        .take()
        .expect("masks should be retained for CU pipeline");

    assert_eq!(
        resolve.len(),
        masks.len(),
        "parsed_graphs_for_masks must be the same parse pass as try_run_phases_and_resolve"
    );
    let resolve_paths: HashSet<_> = resolve.iter().map(|g| g.file_path.clone()).collect();
    let mask_paths: HashSet<_> = masks.iter().map(|g| g.file_path.clone()).collect();
    assert_eq!(
        resolve_paths, mask_paths,
        "per-file graphs should match between resolve and union mask snapshot"
    );
}

// `compilation_units` matches `compilation_unit_keys_for_targets` for the same crate (baseline triple/profile/features).
#[test]
fn union_baseline_cu_keys() {
    let crate_root = fixture_cfg_cu_root();
    let resolve = try_run_phases_and_resolve(&crate_root).expect("resolve");
    let ctx = resolve
        .iter()
        .find_map(|g| g.crate_context.as_ref())
        .expect("crate context on at least one graph");

    let expected = compilation_unit_keys_for_targets(
        ctx.namespace,
        &ctx.targets,
        default_target_triple(),
        "dev".to_string(),
        vec![],
    );

    let mut out = try_run_phases_union_for_crate_with_dimensions(
        &crate_root,
        &CompilationUnitDimensionRequest::baseline_default(),
    )
    .expect("union parse");

    let cu = out.compilation_units.take().expect("compilation_units");
    assert_eq!(
        cu.len(),
        expected.len(),
        "CU enumeration should match targets × baseline dimensions"
    );

    let a: HashSet<_> = cu.into_iter().collect();
    let b: HashSet<_> = expected.into_iter().collect();
    assert_eq!(
        a, b,
        "compilation unit keys should match baseline enumeration"
    );
}

// `try_run_phases_union_for_crate` (dimensions from env): merged graph, masks, and non-empty CU list.
#[test]
fn union_crate() {
    let crate_root = fixture_cfg_cu_root();
    let mut out =
        try_run_phases_union_for_crate(&crate_root).expect("try_run_phases_union_for_crate");

    let merged = out.merged_graph.take().expect("merged graph");
    out.module_tree.take().expect("module tree");
    let masks = out
        .parsed_graphs_for_masks
        .take()
        .expect("parsed_graphs_for_masks");
    let cu = out.compilation_units.take().expect("compilation_units");

    let fn_names: HashSet<_> = merged.functions().iter().map(|f| f.name.as_str()).collect();
    assert!(fn_names.contains("always_present"));

    assert!(
        !masks.is_empty(),
        "mask snapshot should list every parsed file graph from resolve"
    );
    assert!(
        !cu.is_empty(),
        "compilation_units should list at least one CU for a crate with targets"
    );
}
