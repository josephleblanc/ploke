use ploke_common::{fixtures_crates_dir, workspace_root};

use crate::{
    discovery::run_discovery_phase,
    error::SynParserError,
    parser::{analyze_files_parallel, graph::ParsedGraphError, ParsedCodeGraph},
    resolve::module_tree::ModuleTree,
};
pub fn run_phases_and_collect(fixture_name: &str) -> Vec<ParsedCodeGraph> {
    let crate_path = fixtures_crates_dir().join(fixture_name);
    let project_root = workspace_root(); // Use workspace root for context
    let discovery_output = run_discovery_phase(&project_root, &[crate_path.clone()])
        .unwrap_or_else(|e| panic!("Phase 1 Discovery failed for {}: {:?}", fixture_name, e));

    let results_with_errors: Vec<Result<ParsedCodeGraph, SynParserError>> =
        analyze_files_parallel(&discovery_output, 0); // num_workers ignored by rayon bridge

    // Collect successful results, panicking if any file failed to parse in Phase 2
    results_with_errors
        .into_iter()
        .map(|res| {
            res.unwrap_or_else(|e| {
                panic!(
                    "Phase 2 parsing failed for a file in fixture {}: {:?}",
                    fixture_name, e
                )
            })
        })
        .collect()
}

pub fn build_tree_for_tests(fixture_name: &str) -> (ParsedCodeGraph, ModuleTree) {
    let results = run_phases_and_collect(fixture_name);
    let merged_graph = ParsedCodeGraph::merge_new(results).expect("Failed to merge graphs");
    let tree = merged_graph
        .build_module_tree() // dirty, placeholder
        .expect("Failed to build module tree for fixture");
    (merged_graph, tree)
}
