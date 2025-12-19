#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    reason = "Stubs for later helper functions."
)]

pub mod nodes;

use std::path::{Path, PathBuf};

use chrono::Local;
use cozo::MemStorage;
pub use ploke_common::{fixtures_crates_dir, fixtures_dir, workspace_root};
use ploke_core::embeddings::EmbeddingSet;
pub use ploke_core::NodeId;
use syn_parser::discovery::run_discovery_phase;
use syn_parser::error::SynParserError;
use syn_parser::parser::nodes::TypeDefNode;
use syn_parser::parser::{analyze_files_parallel, ParsedCodeGraph};
// TODO: Change import path of `CodeGraph` and `NodeId`, probably better organized to use `ploke-core`
use syn_parser::CodeGraph;

/// Guard to keep both the subscriber default and any file appender worker alive.
pub struct TestTracingGuard {
    _default: tracing::subscriber::DefaultGuard,
    _file: Option<tracing_appender::non_blocking::WorkerGuard>,
}

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
    crate_path: PathBuf,
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
pub fn parse_and_build_tree(
    crate_name: &str,
) -> Result<(ParsedCodeGraph, ModuleTree), ploke_error::Error> {
    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join(crate_name);
    let parsed_graphs = try_run_phases_and_collect_path(&project_root, crate_path)?;
    let mut merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let tree = merged.build_tree_and_prune()?;
    Ok((merged, tree))
}

#[cfg(feature = "test_setup")]
// Available fixture crates may be selected by using the directory name as input to setup_db_full:
//
// tests/fixture_crates/duplicate_name_fixture_1
// tests/fixture_crates/duplicate_name_fixture_2
// tests/fixture_crates/example_crate
// tests/fixture_crates/file_dir_detection
// tests/fixture_crates/fixture_attributes
// tests/fixture_crates/fixture_conflation
// tests/fixture_crates/fixture_cyclic_types
// tests/fixture_crates/fixture_edge_cases
// tests/fixture_crates/fixture_generics
// tests/fixture_crates/fixture_macros
// tests/fixture_crates/fixture_nodes
// tests/fixture_crates/fixture_path_resolution
// tests/fixture_crates/fixture_spp_edge_cases
// tests/fixture_crates/fixture_spp_edge_cases_no_cfg
// tests/fixture_crates/fixture_tracking_hash
// tests/fixture_crates/fixture_types
// tests/fixture_crates/fixture_update_embed
// tests/fixture_crates/simple_crate
// tests/fixture_crates/subdir
pub fn setup_db_full(fixture: &'static str) -> Result<cozo::Db<MemStorage>, ploke_error::Error> {
    use syn_parser::utils::LogStyle;

    tracing::info!("Settup up database with setup_db_full");
    // initialize db
    let db = cozo::Db::new(MemStorage::default()).expect("Failed to create database");
    tracing::info!("{}: Initialize", "Database".log_step());
    db.initialize().expect("Failed to initialize database");
    // create and insert schema for all nodes
    tracing::info!(
        "{}: Create and Insert Schema",
        "Transform/Database".log_step()
    );
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
    tracing::info!(
        "{}: Parsing and Database Transform Complete",
        "Setup".log_step()
    );
    Ok(db)
}

fn setup_db_create_multi_embeddings(
    db: cozo::Db<cozo::MemStorage>,
) -> Result<cozo::Db<cozo::MemStorage>, ploke_error::Error> {
    use ploke_db::multi_embedding::schema::{
        CozoEmbeddingSetExt, EmbeddingSetExt, EmbeddingVector,
    };
    use std::collections::BTreeMap;

    use cozo::ScriptMutability;
    use ploke_core::embeddings::{
        EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
    };
    use ploke_db::DbError;
    use syn_parser::utils::LogStyle;

    tracing::info!("{}: create embedding set", "Db".log_step());
    let create_rel_script = EmbeddingSet::script_create();
    let relation_name = EmbeddingSet::embedding_set_relation_name();
    let db_result = db
        .run_script(
            create_rel_script,
            BTreeMap::new(),
            ScriptMutability::Mutable,
        )
        .map_err(DbError::from)?;
    tracing::info!(?db_result.rows);

    tracing::info!(
        "{}: New multi_embedding relations created in the database
(both embedding_set and default embeddings vector for sentence-transformers)",
        "Setup".log_step()
    );

    tracing::info!("{}: put default embedding set", "Db".log_step());
    let embedding_set = EmbeddingSet::default();

    let script_put = embedding_set.script_put();
    let db_result = db
        .run_script(&script_put, BTreeMap::new(), ScriptMutability::Mutable)
        .map_err(DbError::from)?;
    tracing::info!(put_embedding_set = ?db_result.rows);

    tracing::info!(
        "{}: create default vector embedding relation",
        "Db".log_step()
    );
    let create_vector_script = EmbeddingVector::script_create_from_set(&embedding_set);
    let step_msg = format!("create {} relation", embedding_set.rel_name());
    let db_result = db
        .run_script(
            &create_vector_script,
            BTreeMap::new(),
            ScriptMutability::Mutable,
        )
        .map_err(DbError::from)?;
    tracing::info!(create_embedding_vector = ?db_result.rows);

    Ok(db)
}

pub fn setup_db_create_multi_embeddings_with_hnsw(
    fixture: &'static str,
) -> Result<cozo::Db<cozo::MemStorage>, ploke_error::Error> {
    use ploke_db::multi_embedding::hnsw_ext::HnswExt;

    let embedding_set = EmbeddingSet::default();
    let db = setup_db_full_multi_embedding(fixture)?;
    db.create_embedding_index(&embedding_set)?;
    Ok(db)
}

pub fn setup_db_full_multi_embedding(
    fixture: &'static str,
) -> Result<cozo::Db<MemStorage>, ploke_error::Error> {
    use ploke_db::multi_embedding::schema::{
        CozoEmbeddingSetExt, EmbeddingSetExt, EmbeddingVector,
    };
    use std::collections::BTreeMap;

    use cozo::ScriptMutability;
    use ploke_core::embeddings::{
        EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
    };
    use ploke_db::DbError;
    use syn_parser::utils::LogStyle;

    tracing::info!("Settup up database with setup_db_full");
    // initialize db
    let db = cozo::Db::new(MemStorage::default()).expect("Failed to create database");
    tracing::info!("{}: Initialize", "Database".log_step());
    db.initialize().expect("Failed to initialize database");
    // create and insert schema for all nodes
    tracing::info!(
        "{}: Create and Insert Schema",
        "Transform/Database".log_step()
    );
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
    tracing::info!(
        "{}: Parsing and Database Transform Complete",
        "Setup".log_step()
    );

    setup_db_create_multi_embeddings(db)
}

#[cfg(feature = "test_setup")]
/// Uses the crates in the `ploke` workspace itself as the target.
/// As such, cannot rely on stable inputs over time, but is a more robust example to test against
/// than the fixtures, which usually have various examples but may not have many nodes in total.
pub fn setup_db_full_crate(
    crate_name: &'static str,
) -> Result<cozo::Db<MemStorage>, ploke_error::Error> {
    use syn_parser::utils::LogStyle;

    tracing::info!("Setup up database with setup_db_full_crate");
    // initialize db
    let db = cozo::Db::new(MemStorage::default()).expect("Failed to create database");
    tracing::info!("{}: Initialize", "Database".log_step());
    db.initialize().expect("Failed to initialize database");
    // create and insert schema for all nodes
    tracing::info!(
        "{}: Create and Insert Schema",
        "Transform/Database".log_step()
    );
    ploke_transform::schema::create_schema_all(&db)?;

    // run the parse
    tracing::info!(
        "{}: run the parser, merge graphs, build tree",
        "Parse".log_step()
    );
    let (merged, tree) = parse_and_build_tree(crate_name)?;

    tracing::info!("{}: transform graph into db", "Transform".log_step());
    ploke_transform::transform::transform_parsed_graph(&db, merged, &tree)?;
    tracing::info!(
        "{}: Parsing and Database Transform Complete",
        "Setup".log_step()
    );
    setup_db_create_multi_embeddings(db)
}

#[cfg(feature = "test_setup")]
pub fn setup_db_full_embeddings(
    fixture: &'static str,
) -> std::result::Result<std::vec::Vec<ploke_db::TypedEmbedData>, ploke_error::Error> {
    use ploke_core::EmbeddingData;

    let db = ploke_db::Database::new(setup_db_full_multi_embedding(fixture)?);

    let limit = 100;
    let cursor = 0;
    // let embedding_data = db.get_nodes_for_embedding(100, None)?;
    db.get_unembedded_node_data(limit, cursor)
}

use fmt::format::FmtSpan;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::util::TryInitError;
use tracing_subscriber::{filter, fmt, layer::SubscriberExt, prelude::*, EnvFilter};

fn should_write_test_log_file() -> bool {
    match std::env::var("PLOKE_TEST_LOG") {
        Ok(v) => matches!(v.as_str(), "1" | "true" | "TRUE"),
        Err(_) => false,
    }
}

fn test_log_dir() -> PathBuf {
    workspace_root().join("target").join("test-logs")
}

fn make_test_log_writer(
    prefix: &str,
) -> (
    tracing_appender::non_blocking::NonBlocking,
    Option<WorkerGuard>,
) {
    if !should_write_test_log_file() {
        let (writer, guard) = tracing_appender::non_blocking(std::io::sink());
        return (writer, Some(guard));
    }

    let log_dir = test_log_dir();
    std::fs::create_dir_all(&log_dir).expect("Failed to create test log directory");
    let run_id = format!(
        "{}_{}",
        Local::now().format("%Y%m%d_%H%M%S"),
        std::process::id()
    );
    let file_appender = tracing_appender::rolling::never(log_dir, format!("{prefix}_{run_id}.log"));
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    (writer, Some(guard))
}

/// Legacy helper; now writes optional per-run logs under target/test-logs when `PLOKE_TEST_LOG=1`.
pub fn init_tracing_v2() -> TestTracingGuard {
    init_test_tracing(Level::INFO)
}

#[cfg(feature = "test_setup")]
pub fn init_test_tracing_with_target(
    target: &'static str,
    level: tracing::Level,
) -> TestTracingGuard {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("")); // opt-in via RUST_LOG

    let targets = filter::Targets::new()
        .with_target("ploke", level)
        .with_target("ploke_tui", level)
        .with_target("ploke_db", level)
        .with_target("ploke-embed", level)
        .with_target("ploke-io", level)
        .with_target("ploke-transform", level)
        .with_target("ploke_rag", level)
        .with_target("cozo", Level::ERROR)
        .with_target(target, level);

    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_file(true)
        .with_line_number(true)
        .with_target(true)
        .with_level(true)
        .without_time()
        .with_ansi(true)
        .compact();

    let (file_writer, file_guard) = make_test_log_writer("ploke_test");
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_line_number(true)
        .without_time()
        .with_ansi(false);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(targets)
        .with(console_layer)
        .with(file_layer);

    let default_guard = subscriber.set_default();
    TestTracingGuard {
        _default: default_guard,
        _file: file_guard,
    }
}

#[cfg(feature = "test_setup")]
pub fn init_test_tracing(level: tracing::Level) -> TestTracingGuard {
    init_test_tracing_with_target("", level)
}

pub fn init_tracing_tests(
    target_name: &str,
    target_level: Level,
    base: Option<Level>,
) -> TestTracingGuard {
    let base = base.unwrap_or(Level::ERROR);

    let base_filter = filter::Targets::new()
        .with_target("ploke", base)
        .with_target("ploke_tui", base)
        .with_target("ploke_embed", base)
        .with_target("ploke-db", base)
        .with_target("ploke-embed", base)
        .with_target("ploke-io", base)
        .with_target("ploke-transform", base)
        .with_target("cozo", Level::ERROR)
        .with_target(target_name, target_level);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::from("")); // opt-in via RUST_LOG

    let console_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .without_time()
        .with_line_number(true)
        .with_thread_ids(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(true)
        .with_writer(std::io::stderr);

    let (file_writer, file_guard) = make_test_log_writer("ploke_test");
    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_target(true)
        .with_level(true)
        .with_line_number(true)
        .without_time()
        .with_ansi(false);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(base_filter)
        .with(console_layer)
        .with(file_layer);

    let default_guard = subscriber.set_default();
    TestTracingGuard {
        _default: default_guard,
        _file: file_guard,
    }
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
