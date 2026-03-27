use std::collections::HashSet;

use crate::common::run_phases_and_collect;
use syn_parser::compilation_unit::{
    CompilationUnitTargetKind, build_structural_compilation_unit_slices,
    compilation_unit_keys_for_targets, default_target_triple,
};
use syn_parser::discovery::TargetKind;
use syn_parser::parser::ParsedCodeGraph;
use syn_parser::parser::graph::{GraphAccess, GraphNode};
fn enabled_function_names(
    graph: &ParsedCodeGraph,
    enabled_ids: &HashSet<uuid::Uuid>,
) -> Vec<String> {
    let mut names = graph
        .functions()
        .iter()
        .filter(|f| enabled_ids.contains(&f.any_id().uuid()))
        .map(|f| f.name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn enabled_import_debug(graph: &ParsedCodeGraph, enabled_ids: &HashSet<uuid::Uuid>) -> Vec<String> {
    let mut imports = graph
        .use_statements()
        .iter()
        .filter(|i| enabled_ids.contains(&i.any_id().uuid()))
        .map(|i| format!("{} :: {:?}", i.name(), i.source_path))
        .collect::<Vec<_>>();
    imports.sort();
    imports.dedup();
    imports
}

#[test]
fn structural_slices_diverge_for_lib_and_bin_targets() {
    let parsed_graphs = run_phases_and_collect("fixture_multi_target_cu");
    let ctx = parsed_graphs
        .iter()
        .find_map(|g| g.crate_context.as_ref())
        .expect("crate context");

    let keys = compilation_unit_keys_for_targets(
        ctx.namespace,
        &ctx.targets,
        default_target_triple(),
        "dev".to_string(),
        vec![],
    );
    let lib_key = keys
        .iter()
        .find(|k| k.target_kind == CompilationUnitTargetKind::Lib)
        .expect("lib key")
        .clone();
    let bin_key = keys
        .iter()
        .find(|k| k.target_kind == CompilationUnitTargetKind::Bin)
        .expect("bin key")
        .clone();

    let slices = build_structural_compilation_unit_slices(
        parsed_graphs.clone(),
        &[lib_key.clone(), bin_key.clone()],
    )
    .expect("build structural slices");
    let lib_slice = slices
        .iter()
        .find(|s| s.key.target_kind == CompilationUnitTargetKind::Lib)
        .expect("lib slice");
    let bin_slice = slices
        .iter()
        .find(|s| s.key.target_kind == CompilationUnitTargetKind::Bin)
        .expect("bin slice");

    let partition = ParsedCodeGraph::partition_by_selected_roots(parsed_graphs.clone())
        .expect("partition by roots");
    let lib_merged = partition
        .merge_for_root(&lib_key.target_root)
        .expect("merge lib");
    let bin_merged = partition
        .merge_for_root(&bin_key.target_root)
        .expect("merge bin");

    let lib_only_id = lib_merged
        .functions()
        .iter()
        .find_map(|f| (f.name == "lib_only").then_some(f.any_id().uuid()))
        .expect("lib_only id");
    let bin_only_id = bin_merged
        .functions()
        .iter()
        .find_map(|f| (f.name == "bin_only").then_some(f.any_id().uuid()))
        .expect("bin_only id");

    assert!(lib_slice.enabled_node_ids.contains(&lib_only_id));
    assert!(!lib_slice.enabled_node_ids.contains(&bin_only_id));
    assert!(bin_slice.enabled_node_ids.contains(&bin_only_id));
    assert!(!bin_slice.enabled_node_ids.contains(&lib_only_id));

    let has_lib_target = ctx
        .targets
        .iter()
        .any(|t| matches!(t.kind, TargetKind::Lib));
    let has_bin_target = ctx
        .targets
        .iter()
        .any(|t| matches!(t.kind, TargetKind::Bin));
    assert!(has_lib_target && has_bin_target);

    let lib_enabled_fns = enabled_function_names(&lib_merged, &lib_slice.enabled_node_ids);
    let bin_enabled_fns = enabled_function_names(&bin_merged, &bin_slice.enabled_node_ids);
    let lib_enabled_imports = enabled_import_debug(&lib_merged, &lib_slice.enabled_node_ids);
    let bin_enabled_imports = enabled_import_debug(&bin_merged, &bin_slice.enabled_node_ids);

    println!("lib enabled functions: {lib_enabled_fns:#?}");
    println!("bin enabled functions: {bin_enabled_fns:#?}");
    println!("lib enabled imports: {lib_enabled_imports:#?}");
    println!("bin enabled imports: {bin_enabled_imports:#?}");
}

#[test]
fn structural_slices_support_custom_lib_path_targets() {
    let parsed_graphs = run_phases_and_collect("fixture_unusual_lib");
    let ctx = parsed_graphs
        .iter()
        .find_map(|g| g.crate_context.as_ref())
        .expect("crate context");

    let keys = compilation_unit_keys_for_targets(
        ctx.namespace,
        &ctx.targets,
        default_target_triple(),
        "dev".to_string(),
        vec![],
    );
    let lib_key = keys
        .iter()
        .find(|k| k.target_kind == CompilationUnitTargetKind::Lib)
        .expect("lib key")
        .clone();
    let bin_key = keys
        .iter()
        .find(|k| k.target_kind == CompilationUnitTargetKind::Bin)
        .expect("bin key")
        .clone();

    assert!(
        lib_key.target_root.ends_with("fixture_unusual_lib/lib.rs"),
        "expected custom lib target root outside src, got {:?}",
        lib_key.target_root
    );
    assert!(
        bin_key
            .target_root
            .ends_with("fixture_unusual_lib/src/main.rs"),
        "expected bin target root in src/main.rs, got {:?}",
        bin_key.target_root
    );

    let slices = build_structural_compilation_unit_slices(
        parsed_graphs.clone(),
        &[lib_key.clone(), bin_key.clone()],
    )
    .expect("build structural slices");
    let lib_slice = slices
        .iter()
        .find(|s| s.key.target_kind == CompilationUnitTargetKind::Lib)
        .expect("lib slice");
    let bin_slice = slices
        .iter()
        .find(|s| s.key.target_kind == CompilationUnitTargetKind::Bin)
        .expect("bin slice");

    let partition = ParsedCodeGraph::partition_by_selected_roots(parsed_graphs.clone())
        .expect("partition by roots");
    let lib_merged = partition
        .merge_for_root(&lib_key.target_root)
        .expect("merge lib");
    let bin_merged = partition
        .merge_for_root(&bin_key.target_root)
        .expect("merge bin");

    let in_lib_id = lib_merged
        .functions()
        .iter()
        .find_map(|f| (f.name == "in_lib").then_some(f.any_id().uuid()))
        .expect("in_lib id");
    let in_bin_id = bin_merged
        .functions()
        .iter()
        .find_map(|f| (f.name == "in_bin").then_some(f.any_id().uuid()))
        .expect("in_bin id");

    assert!(lib_slice.enabled_node_ids.contains(&in_lib_id));
    assert!(!lib_slice.enabled_node_ids.contains(&in_bin_id));
    assert!(bin_slice.enabled_node_ids.contains(&in_bin_id));
    assert!(!bin_slice.enabled_node_ids.contains(&in_lib_id));

    let lib_enabled_fns = enabled_function_names(&lib_merged, &lib_slice.enabled_node_ids);
    let bin_enabled_fns = enabled_function_names(&bin_merged, &bin_slice.enabled_node_ids);
    let lib_enabled_imports = enabled_import_debug(&lib_merged, &lib_slice.enabled_node_ids);
    let bin_enabled_imports = enabled_import_debug(&bin_merged, &bin_slice.enabled_node_ids);

    println!("custom lib path lib enabled functions: {lib_enabled_fns:#?}");
    println!("custom lib path bin enabled functions: {bin_enabled_fns:#?}");
    println!("custom lib path lib enabled imports: {lib_enabled_imports:#?}");
    println!("custom lib path bin enabled imports: {bin_enabled_imports:#?}");
}
