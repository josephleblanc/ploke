#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    reason = "Stubs for later helper functions."
)]

pub mod nodes;

use std::path::{Path, PathBuf};

use cozo::MemStorage;
pub use ploke_common::{fixtures_crates_dir, fixtures_dir, workspace_root};
use ploke_core::NodeId;
use syn_parser::discovery::run_discovery_phase;
use syn_parser::error::SynParserError;
use syn_parser::parser::nodes::TypeDefNode;
use syn_parser::parser::{analyze_files_parallel, ParsedCodeGraph};
// TODO: Change import path of `CodeGraph` and `NodeId`, probably better organized to use `ploke-core`
use syn_parser::CodeGraph;

// Should return result
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


#[cfg(feature = "test_setup")]
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
#[cfg(feature = "test_setup")]
use syn_parser::ModuleTree;

#[cfg(feature = "test_setup")]
pub fn parse_and_build_tree(crate_name: &str) -> Result<(ParsedCodeGraph, ModuleTree), ploke_error::Error> {

    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join(crate_name);
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let mut merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let tree = merged.build_tree_and_prune()?;
    Ok((merged, tree ))
}

#[cfg(feature = "test_setup")]
pub fn setup_db_full(fixture: &'static str) -> Result<cozo::Db<MemStorage>, ploke_error::Error> {
    use syn_parser::utils::LogStyle;

    tracing::info!("Settup up database with setup_db_full");
    // initialize db
    let db = cozo::Db::new(MemStorage::default()).expect("Failed to create database");
    tracing::info!("{}: Initialize", "Database".log_step());
    db.initialize().expect("Failed to initialize database");
    // create and insert schema for all nodes
    tracing::info!("{}: Create and Insert Schema", "Transform/Database".log_step());
    ploke_transform::schema::create_schema_all(&db)?;

    // run the parse
    tracing::info!("{}: run the parser", "Parse".log_step());
    let successful_graphs = test_run_phases_and_collect(fixture);
    // merge results from all files
    tracing::info!("{}: merge the graphs", "Parse".log_step());
    let mut merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

    // build module tree
    tracing::info!("{}: build module tree", "Parse".log_step());
    let tree = merged.build_tree_and_prune().unwrap_or_else(|e| {
        log::error!(target: "transform_function",
            "Error building tree: {}",
            e
        );
        panic!()
    });

    tracing::info!("{}: transform graph into db", "Transform".log_step());
    ploke_transform::transform::transform_parsed_graph(&db, merged, &tree)?;
    tracing::info!("{}: Parsing and Database Transform Complete", "Setup".log_step());
    Ok(db)
}

#[cfg(feature = "test_setup")]
pub fn setup_db_full_embeddings(
    fixture: &'static str,
) -> std::result::Result<std::vec::Vec<ploke_db::TypedEmbedData>, ploke_error::Error> {
    use ploke_core::EmbeddingData;

    let db = ploke_db::Database::new(setup_db_full(fixture)?);

    let limit = 100;
    let cursor = 0;
    // let embedding_data = db.get_nodes_for_embedding(100, None)?;
    db.get_unembedded_node_data(limit, cursor)
}

use fmt::format::FmtSpan;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{filter, fmt, prelude::*, EnvFilter};

pub fn init_tracing_v2() -> WorkerGuard {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")); // Default to 'info' level

    // File appender with custom timestamp format
    let log_dir = "logs";
    std::fs::create_dir_all(log_dir).expect("Failed to create logs directory");
    let file_appender = tracing_appender::rolling::daily(log_dir, "ploke.log");
    let (non_blocking_file, file_guard) = tracing_appender::non_blocking(file_appender);

    // Common log format builder
    let fmt_layer = fmt::layer()
        .pretty()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_span_events(FmtSpan::CLOSE); // Capture span durations

    let file_subscriber = fmt_layer
        .with_writer(std::io::stderr)
        // .with_writer(non_blocking_file)
        .with_ansi(false)
        .compact();

    tracing_subscriber::registry()
        .with(filter)
        .with(file_subscriber)
        .init();

    file_guard
}

#[cfg(feature = "test_setup")]
pub fn init_test_tracing(level: tracing::Level) {
    use tracing::Level;

    let filter = filter::Targets::new()
        .with_target("debug_dup", Level::ERROR)
        // .with_target("ploke", level)
        // .with_target("ploke-db", level)
        // .with_target("ploke-embed", level)
        // .with_target("ploke-io", level)
        // .with_target("ploke-transform", level)
        .with_target("cozo", Level::ERROR);

    let layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_file(true)
        .with_line_number(true)
        .with_target(false) // Show module path
        .with_level(true) // Show log level
        .without_time() // Remove timestamps
        .with_ansi(true)
        .pretty(); 
    tracing_subscriber::registry()
        .with(layer)
        .with(filter)
        .init();
}

// Should return result
pub fn parse_malformed_fixture(fixture_name: &str) {
    todo!()
}

/// Find a function node by name in a CodeGraph
// We have better funcitons for this now, still, not a bad idea to make them all available from
// here maybe, by re-exporting from `syn_parser`
pub fn find_function_by_name(graph: &CodeGraph, name: &str) -> Option<NodeId> {
    todo!()
}

/// Find a struct node by name in a CodeGraph  
// Again, we have better ways to do this in `syn_parser`
// Look for good helpers from test functions
pub fn find_struct_(graph: &CodeGraph, name: &str) -> Option<NodeId> {
    todo!()
}

/// Find a module node by path in a CodeGraph                          
// Again, we have better ways to do this in `syn_parser`
// Look for good helpers from test functions
pub fn find_module_by_(graph: &CodeGraph, path: &[String]) -> Option<NodeId> {
    todo!()
}

// Helper to create module path for testing
pub fn test_module_path(segments: &[&str]) /* return type tbd */
{
    todo!()
}
