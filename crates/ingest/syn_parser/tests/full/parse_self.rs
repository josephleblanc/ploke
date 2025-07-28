use std::path::{Path, PathBuf};

use ploke_common::workspace_root;
use syn_parser::{discovery::run_discovery_phase, error::SynParserError, parser::analyze_files_parallel, ParsedCodeGraph};

// NOTE: If these tests need to be refactored at some point, consider using/improving the following
// macro, suggested by the AI.
// Also consider adding a lazy constructor for each of these module trees for further testing in
// end-to-end testing later.
// - See tests in: ploke/crates/ingest/ploke-transform/src/tests.rs
//
//                                 The Macro Solution
//
// Here are two versions - a simple one that just reduces repetition, and a more
// advanced one that gives you full control:
//
//                         Version 1: Simple Declarative Macro
//
//
// macro_rules! crate_test {
//     ($test_name:ident, $crate_name:expr) => {
//         #[test]
//         pub fn $test_name() -> Result<(), ploke_error::Error> {
//             let _ = env_logger::builder()
//                 .is_test(true)
//                 .format_timestamp(None)
//                 .format_file(true)
//                 .format_line_number(true)
//                 .try_init();
//
//             let project_root = workspace_root();
//             let crate_path = workspace_root()
//                 .join("crates")
//                 .join($crate_name);
//
//             try_run_phases_and_collect_path(&project_root, crate_path)
//                 .inspect_err(|e| error!("error running try_run_phases and collect
// {e}"))?;
//             Ok(())
//         }
//     };
// }
//
//
//                  Version 2: Advanced Macro with Optional Features
//
//
// Use this macro to duplicate the following tests. DO NOT DELETE THE TESTS. I want to make sure
// the tests run correctly both with the macro version and the non-macro version before removing
// the current tests!
macro_rules! crate_test {
    // Basic case - just name and crate path
    ($test_name:ident, $crate_name:expr) => {
        crate_test!($test_name, $crate_name, false);
    };

    // With module tree building
    ($test_name:ident, $crate_name:expr, build_tree) => {
        crate_test!($test_name, $crate_name, true);
    };

    // Internal implementation
    ($test_name:ident, $crate_name:expr, $build_tree:expr) => {
        #[test]
        pub fn $test_name() -> Result<(), ploke_error::Error> {
            let _ = env_logger::builder()
                .is_test(true)
                .format_timestamp(None)
                .format_file(true)
                .format_line_number(true)
                .try_init();

            let project_root = workspace_root();
            let crate_path = workspace_root()
                .join("crates")
                .join($crate_name);

            let parsed_graphs = try_run_phases_and_collect_path(&project_root,
crate_path)?;

            if $build_tree {
                let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
                let _tree = merged.build_module_tree()?;
            }

            Ok(())
        }
    };
}


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
pub fn parse_syn() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ingest").join("syn_parser");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

#[test]
pub fn parse_transform() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ingest").join("ploke-transform");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

#[test]
pub fn parse_embed() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ingest").join("ploke-embed");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

#[test]
pub fn parse_core() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-core");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

#[test]
pub fn parse_db() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-db");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

#[test]
pub fn parse_error() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-error");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

#[test]
pub fn parse_io() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-io");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

#[test]
pub fn parse_rag() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-rag");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

#[test]
pub fn parse_tui() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-tui");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

#[test]
pub fn parse_ty_mcp() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ploke-ty-mcp");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

#[test]
pub fn parse_test_utils() -> Result<(), ploke_error::Error> {
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("test-utils");
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let _tree = merged.build_module_tree()?;
    Ok(())
}

