use crate::common::run_phases_and_collect;
use syn_parser::compilation_unit::{
    CompilationUnitTargetKind, build_structural_compilation_unit_slices,
    compilation_unit_keys_for_targets, default_target_triple,
};
use syn_parser::discovery::TargetKind;
use syn_parser::parser::graph::{GraphAccess, GraphNode};
use syn_parser::parser::ParsedCodeGraph;

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

    let slices =
        build_structural_compilation_unit_slices(parsed_graphs.clone(), &[lib_key.clone(), bin_key.clone()])
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

    let has_lib_target = ctx.targets.iter().any(|t| matches!(t.kind, TargetKind::Lib));
    let has_bin_target = ctx.targets.iter().any(|t| matches!(t.kind, TargetKind::Bin));
    assert!(has_lib_target && has_bin_target);
}
