use std::{path::{Path, PathBuf}, sync::Arc};

use ploke_db::Database;
use syn_parser::{discovery::run_discovery_phase, error::SynParserError, parser::analyze_files_parallel, ParsedCodeGraph};

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Error parsing workspace directory from crate `common`") // crates/
        .parent() // workspace root
        .expect("Failed to get workspace root")
        .to_path_buf()
}
pub fn fixtures_crates_dir() -> PathBuf {
    workspace_root().join("tests/fixture_crates")
}


pub fn run_parse(db: Arc< Database >,  fixture: &'static str) -> Result<(), ploke_error::Error> {
    use syn_parser::utils::LogStyle;

    // run the parse
    tracing::info!("{}: run the parser", "Parse".log_step());
    let successful_graphs = test_run_phases_and_collect(fixture);
    // merge results from all files
    tracing::info!("{}: merge the graphs", "Parse".log_step());
    let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

    // build module tree
    tracing::info!("{}: build module tree", "Parse".log_step());
    let tree = merged.build_module_tree().unwrap_or_else(|e| {
        log::error!(target: "transform_function",
            "Error building tree: {}",
            e
        );
        panic!()
    });

    tracing::info!("{}: transform graph into db", "Transform".log_step());
    ploke_transform::transform::transform_parsed_graph(&db, merged, &tree)?;
    tracing::info!("{}: Parsing and Database Transform Complete", "Setup".log_step());
    Ok(())
}

pub fn test_run_phases_and_collect(fixture_name: &str) -> Vec<ParsedCodeGraph> {
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
