use std::path::{Path, PathBuf};

use log::error;
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
        eprintln!("result is ok? | {}", result.is_ok());
        results.push(result?);
    }
    Ok(results)
}

// TODO: Add specific tests to handle known limitation #11 from
// docs/plans/uuid_refactor/02c_phase2_known_limitations.md

#[test]
pub fn basic_test_parse_self() -> Result<(), ploke_error::Error> {
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

#[test]
pub fn basic_test_parse_transform() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ingest").join("ploke-transform");
    try_run_phases_and_collect_path(&project_root, crate_path).inspect_err(|e| error!("error running try_run_phases and collect: {e}"))?;
    Ok(())
}

#[test]
pub fn basic_test_parse_embed() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ingest").join("ploke-embed");
    try_run_phases_and_collect_path(&project_root, crate_path).inspect_err(|e| error!("error running try_run_phases and collect: {e}"))?;
    Ok(())
}

#[test]
pub fn basic_test_parse_core() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-core");
    try_run_phases_and_collect_path(&project_root, crate_path).inspect_err(|e| error!("error running try_run_phases and collect: {e}"))?;
    Ok(())
}

#[test]
pub fn basic_test_parse_db() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-db");
    try_run_phases_and_collect_path(&project_root, crate_path).inspect_err(|e| error!("error running try_run_phases and collect: {e}"))?;
    Ok(())
}

#[test]
pub fn basic_test_parse_error() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-error");
    try_run_phases_and_collect_path(&project_root, crate_path).inspect_err(|e| error!("error running try_run_phases and collect: {e}"))?;
    Ok(())
}

#[test]
pub fn basic_test_parse_io() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-io");
    try_run_phases_and_collect_path(&project_root, crate_path).inspect_err(|e| error!("error running try_run_phases and collect: {e}"))?;
    Ok(())
}

#[test]
pub fn basic_test_parse_rag() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-rag");
    try_run_phases_and_collect_path(&project_root, crate_path).inspect_err(|e| error!("error running try_run_phases and collect: {e}"))?;
    Ok(())
}

#[test]
pub fn basic_test_parse_tui() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-tui");
    try_run_phases_and_collect_path(&project_root, crate_path).inspect_err(|e| error!("error running try_run_phases and collect: {e}"))?;
    Ok(())
}

#[test]
pub fn basic_test_parse_ty_mcp() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-ty-mcp");
    try_run_phases_and_collect_path(&project_root, crate_path).inspect_err(|e| error!("error running try_run_phases and collect: {e}"))?;
    Ok(())
}

#[test]
pub fn basic_test_parse_test_utils() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("test-utils");
    try_run_phases_and_collect_path(&project_root, crate_path).inspect_err(|e| error!("error running try_run_phases and collect: {e}"))?;
    Ok(())
}
