use std::path::{Path, PathBuf};

use ploke_common::workspace_root;
use syn_parser::{discovery::run_discovery_phase, error::SynParserError, parser::analyze_files_parallel, ParsedCodeGraph};

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
                // .format_file(true)
                // .format_line_number(true)
                .try_init();

            let project_root = workspace_root();
            let crate_path = workspace_root()
                .join("crates")
                .join($crate_name);

            let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;

            if $build_tree {
                let mut merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
                let _tree = merged.build_tree_and_prune()?;
            }

            Ok(())
        }
    };
}

crate_test!(new_parse_syn, "ingest/syn_parser", build_tree);
crate_test!(new_parse_transform, "ingest/ploke-transform", build_tree);
crate_test!(new_parse_embed, "ingest/ploke-embed", build_tree);
crate_test!(new_parse_core, "ploke-core", build_tree);
crate_test!(new_parse_db, "ploke-db", build_tree);
crate_test!(new_parse_error, "ploke-error", build_tree);
crate_test!(new_parse_io, "ploke-io", build_tree);
crate_test!(new_parse_rag, "ploke-rag", build_tree);
crate_test!(new_parse_tui, "ploke-tui", build_tree);
crate_test!(new_parse_ty_mcp, "ploke-ty-mcp", build_tree);
crate_test!(new_parse_test_utils, "test-utils", build_tree);


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

// NOTE: Leaving this duplicated test of the macro functionality above for later reference, in case
// #[test]
// // there are any issues with the macro.
// pub fn parse_syn() -> Result<(), ploke_error::Error> {
//     let _ = env_logger::builder()
//         .is_test(true)
//         .format_timestamp(None) // Disable timestamps
//         .format_file(true)
//         .format_line_number(true)
//         .try_init();
//     let project_root = workspace_root(); // Use workspace root for context
//     let crate_path = workspace_root().join("crates").join("ingest").join("syn_parser");
//     let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
//     let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
//     let _(tree, pruned_items) = merged.build_tree_and_prune()?;
//     Ok(())
// }

