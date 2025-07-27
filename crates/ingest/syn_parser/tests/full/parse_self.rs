use std::path::{Path, PathBuf};

use ploke_common::workspace_root;
use syn_parser::{discovery::run_discovery_phase, error::SynParserError, parser::analyze_files_parallel, ParsedCodeGraph};


pub fn try_run_phases_and_collect_path(
    project_root: &Path,
    crate_path: PathBuf
) -> Result<Vec<ParsedCodeGraph>, ploke_error::Error> {
    let discovery_output = run_discovery_phase(project_root, &[crate_path.clone()])?;

    let results_with_errors: Vec<Result<ParsedCodeGraph, SynParserError>> =
        analyze_files_parallel(&discovery_output, 0); // num_workers ignored by rayon bridge

    // Collect successful results, panicking if any file failed to parse in Phase 2
    let mut results = Vec::new();
    for result in results_with_errors {
        results.push(result?);
    }
    Ok(results)
}

#[test]
pub fn basic_test() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ingest").join("syn_parser");
    try_run_phases_and_collect_path(&project_root, crate_path)?;
    Ok(())
}
