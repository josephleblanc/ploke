#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    reason = "Stubs for later helper functions."
)]

pub mod nodes;

use std::path::{Path, PathBuf};

#[cfg(feature = "multi_embedding_schema")]
use std::collections::BTreeMap;

use cozo::MemStorage;
#[cfg(feature = "multi_embedding_schema")]
use cozo::{DataValue, NamedRows, ScriptMutability, UuidWrapper};
pub use ploke_common::{
    fixtures_crates_dir, fixtures_dir, workspace_root, LEGACY_FIXTURE_BACKUP_REL_PATH,
    LEGACY_FIXTURE_METADATA_REL_PATH, MULTI_EMBED_FIXTURE_BACKUP_REL_PATH,
    MULTI_EMBED_FIXTURE_METADATA_REL_PATH, MULTI_EMBED_SCHEMA_TAG,
};
pub use ploke_core::NodeId;
#[cfg(feature = "multi_embedding_schema")]
use ploke_db::multi_embedding::{
    embedding_entry, experimental_node_relation_specs, vector_dimension_specs,
    ExperimentalEmbeddingDbExt, ExperimentalNodeRelationSpec, ExperimentalVectorRelation,
};
#[cfg(feature = "multi_embedding_schema")]
use ploke_db::DbError;
use syn_parser::discovery::run_discovery_phase;
use syn_parser::error::SynParserError;
use syn_parser::parser::nodes::TypeDefNode;
use syn_parser::parser::{analyze_files_parallel, ParsedCodeGraph};
// TODO: Change import path of `CodeGraph` and `NodeId`, probably better organized to use `ploke-core`
use syn_parser::CodeGraph;
use tokio::runtime::Runtime;
use uuid::Uuid;

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

#[cfg(feature = "test_setup")]
/// Uses the crates in the `ploke` workspace itself as the target.
/// As such, cannot rely on stable inputs over time, but is a more robust example to test against
/// than the fixtures, which usually have various examples but may not have many nodes in total.
pub fn setup_db_full_crate(
    crate_name: &'static str,
) -> Result<cozo::Db<MemStorage>, ploke_error::Error> {
    use syn_parser::utils::LogStyle;

    tracing::info!("Settup up database with setup_db_full_crate");
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
    Ok(db)
}

#[cfg(feature = "test_setup")]
pub fn setup_db_full_embeddings(
    fixture: &'static str,
) -> std::result::Result<std::vec::Vec<ploke_db::TypedEmbedData>, ploke_error::Error> {
    use ploke_core::EmbeddingData;

    let db = ploke_db::Database::new(setup_db_full(fixture)?);

    #[cfg(feature = "multi_embedding_schema")]
    seed_multi_embedding_schema(&db)?;

    let limit = 100;
    let cursor = 0;
    // let embedding_data = db.get_nodes_for_embedding(100, None)?;
    db.get_unembedded_node_data(limit, cursor)
}

const LEGACY_EMBEDDING_TARGET_ROWS: usize = 32;
const LEGACY_EMBEDDING_VECTOR_DIMS: usize = 384;
const LEGACY_EMBEDDING_FETCH_FLOOR: usize = 16;

pub fn seed_default_legacy_embeddings(
    db: &ploke_db::Database,
) -> Result<usize, ploke_error::Error> {
    seed_legacy_embeddings(
        db,
        LEGACY_EMBEDDING_TARGET_ROWS,
        LEGACY_EMBEDDING_VECTOR_DIMS,
    )
}

pub fn seed_legacy_embeddings(
    db: &ploke_db::Database,
    desired_rows: usize,
    dims: usize,
) -> Result<usize, ploke_error::Error> {
    if desired_rows == 0 || dims == 0 {
        return Ok(0);
    }
    let limit = desired_rows.max(LEGACY_EMBEDDING_FETCH_FLOOR);
    let typed_nodes = db.get_unembedded_node_data(limit, 0)?;
    let mut updates = Vec::new();
    let mut seeded = 0usize;
    for typed in typed_nodes {
        for entry in typed.v {
            updates.push((entry.id, build_legacy_embedding_vector(dims, seeded)));
            seeded += 1;
            if seeded >= desired_rows {
                break;
            }
        }
        if seeded >= desired_rows {
            break;
        }
    }
    apply_legacy_embedding_updates(db, updates)?;
    Ok(seeded)
}

fn build_legacy_embedding_vector(dims: usize, seed: usize) -> Vec<f32> {
    let mut vector = Vec::with_capacity(dims);
    for offset in 0..dims {
        let value = (((seed + offset) % 97) as f32 + 1.0) / 97.0;
        vector.push(value);
    }
    vector
}

fn apply_legacy_embedding_updates(
    db: &ploke_db::Database,
    updates: Vec<(Uuid, Vec<f32>)>,
) -> Result<(), ploke_error::Error> {
    if updates.is_empty() {
        return Ok(());
    }
    let runtime = Runtime::new().map_err(|err| {
        ploke_error::Error::TransformError(format!("failed to init tokio runtime: {err}"))
    })?;
    runtime.block_on(async {
        db.update_embeddings_batch(updates)
            .await
            .map_err(ploke_error::Error::from)
    })
}

#[cfg(feature = "multi_embedding_schema")]
pub fn seed_multi_embedding_schema(db: &ploke_db::Database) -> Result<(), ploke_error::Error> {
    for spec in experimental_node_relation_specs() {
        let node_ids = seed_metadata_rows(db, spec)?;
        seed_vector_rows(db, spec, &node_ids)?;
    }
    Ok(())
}

#[cfg(feature = "multi_embedding_schema")]
fn seed_metadata_rows(
    db: &ploke_db::Database,
    spec: &'static ExperimentalNodeRelationSpec,
) -> Result<Vec<Uuid>, ploke_error::Error> {
    ensure_metadata_relation(db, spec)?;
    let projection_fields = metadata_projection_fields(spec);
    if projection_fields.is_empty() {
        return Ok(Vec::new());
    }
    let columns_csv = projection_fields.join(", ");
    let base_relation = spec.node_type.relation_str();
    let script = format!(
        r#"?[{columns}] :=
    *{base} {{ {columns} @ 'NOW' }}"#,
        columns = columns_csv,
        base = base_relation,
    );
    let NamedRows { headers, rows, .. } =
        run_script(db, &script, BTreeMap::new(), ScriptMutability::Immutable)?;
    let embeddings_template: Vec<DataValue> = vector_dimension_specs()
        .iter()
        .map(|dim_spec| embedding_entry(dim_spec.embedding_model(), dim_spec.dims()))
        .collect();
    let mut node_ids = Vec::new();
    for row in rows {
        let mut params = BTreeMap::new();
        for (idx, header) in headers.iter().enumerate() {
            params.insert(header.clone(), row[idx].clone());
        }
        params.insert(
            "embeddings".into(),
            DataValue::List(embeddings_template.clone()),
        );
        let insert_script = spec.metadata_schema.script_put(&params);
        run_script(
            db,
            &insert_script,
            params.clone(),
            ScriptMutability::Mutable,
        )?;
        let node_id_value = params.get("id").ok_or_else(|| {
            ploke_error::Error::from(DbError::ExperimentalMetadataParse {
                reason: format!("metadata params missing id for {}", spec.name),
            })
        })?;
        node_ids.push(data_value_to_uuid(node_id_value)?);
    }
    Ok(node_ids)
}

#[cfg(feature = "multi_embedding_schema")]
fn seed_vector_rows(
    db: &ploke_db::Database,
    spec: &'static ExperimentalNodeRelationSpec,
    node_ids: &[Uuid],
) -> Result<(), ploke_error::Error> {
    if node_ids.is_empty() {
        return Ok(());
    }
    for dim_spec in vector_dimension_specs() {
        let relation = ExperimentalVectorRelation::new(dim_spec.dims(), spec.vector_relation_base);
        ensure_vector_relation(db, &relation)?;
        for node_id in node_ids {
            relation
                .insert_row(db, *node_id, dim_spec)
                .map_err(ploke_error::Error::from)?;
        }
    }
    Ok(())
}

#[cfg(feature = "multi_embedding_schema")]
fn ensure_metadata_relation(
    db: &ploke_db::Database,
    spec: &ExperimentalNodeRelationSpec,
) -> Result<(), ploke_error::Error> {
    match db.ensure_relation_registered(spec.metadata_schema.relation()) {
        Ok(()) => Ok(()),
        Err(DbError::ExperimentalRelationMissing { .. }) => {
            run_script(
                db,
                &spec.metadata_schema.script_create(),
                BTreeMap::new(),
                ScriptMutability::Mutable,
            )?;
            Ok(())
        }
        Err(err) => Err(ploke_error::Error::from(err)),
    }
}

#[cfg(feature = "multi_embedding_schema")]
fn ensure_vector_relation(
    db: &ploke_db::Database,
    relation: &ExperimentalVectorRelation,
) -> Result<(), ploke_error::Error> {
    match db.ensure_relation_registered(&relation.relation_name()) {
        Ok(()) => Ok(()),
        Err(DbError::ExperimentalRelationMissing { .. }) => {
            run_script(
                db,
                &relation.script_create(),
                BTreeMap::new(),
                ScriptMutability::Mutable,
            )?;
            Ok(())
        }
        Err(err) => Err(ploke_error::Error::from(err)),
    }
}

#[cfg(feature = "multi_embedding_schema")]
fn metadata_projection_fields(spec: &ExperimentalNodeRelationSpec) -> Vec<&'static str> {
    spec.metadata_schema
        .field_names()
        .filter(|field| *field != "embeddings")
        .collect()
}

#[cfg(feature = "multi_embedding_schema")]
fn run_script(
    db: &ploke_db::Database,
    script: &str,
    params: BTreeMap<String, DataValue>,
    mutability: ScriptMutability,
) -> Result<NamedRows, ploke_error::Error> {
    db.run_script(script, params, mutability).map_err(|err| {
        let mut msg = err.to_string();
        msg.push_str(" | script: ");
        msg.push_str(script);
        ploke_error::Error::from(DbError::Cozo(msg))
    })
}

#[cfg(feature = "multi_embedding_schema")]
fn data_value_to_uuid(value: &DataValue) -> Result<Uuid, ploke_error::Error> {
    if let DataValue::Uuid(UuidWrapper(uuid)) = value {
        Ok(*uuid)
    } else {
        Err(ploke_error::Error::from(
            DbError::ExperimentalMetadataParse {
                reason: "id column must be a uuid".into(),
            },
        ))
    }
}

#[cfg(all(test, feature = "test_setup"))]
mod tests {
    use super::*;

    #[cfg(feature = "multi_embedding_schema")]
    #[test]
    fn seeds_multi_embedding_relations_for_fixture_nodes() -> Result<(), ploke_error::Error> {
        let raw_db = setup_db_full("fixture_nodes")?;
        let database = ploke_db::Database::new(raw_db);
        seed_multi_embedding_schema(&database)?;
        assert!(
            relation_has_rows(&database, "function_multi_embedding", "id")?,
            "expected function metadata rows"
        );
        assert!(
            relation_has_rows(&database, "function_embedding_vectors_384", "node_id")?,
            "expected function vector rows for 384 dims"
        );
        let nodes = database.get_unembedded_node_data(16, 0)?;
        assert!(
            nodes.iter().any(|typed| !typed.v.is_empty()),
            "embedding batches should be available after seeding"
        );
        Ok(())
    }

    #[cfg(feature = "multi_embedding_schema")]
    fn relation_has_rows(
        db: &ploke_db::Database,
        relation_name: &str,
        column_name: &str,
    ) -> Result<bool, ploke_error::Error> {
        let script = format!(
            r#"?[{column}] :=
    *{rel} {{ {column} @ 'NOW' }}"#,
            column = column_name,
            rel = relation_name
        );
        let rows = db
            .run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| ploke_error::Error::from(DbError::Cozo(err.to_string())))?;
        Ok(!rows.rows.is_empty())
    }
}

use fmt::format::FmtSpan;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::util::TryInitError;
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
        // .with_target("debug_dup", Level::ERROR)
        .with_target("db", Level::ERROR)
        .with_target("ploke_tui::app_state", Level::INFO)
        .with_target("ploke_embed", Level::ERROR)
        .with_target("specific_target", Level::ERROR)
        .with_target("file_hashes", Level::ERROR)
        .with_target("ploke", level)
        .with_target("ploke-db", level)
        .with_target("ploke-embed", level)
        .with_target("ploke-io", level)
        .with_target("ploke-transform", level)
        .with_target("cozo", Level::ERROR);

    let layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_file(true)
        .with_line_number(true)
        .with_target(true) // Show module path
        .with_level(true) // Show log level
        .without_time() // Remove timestamps
        .with_ansi(true)
        .pretty();
    tracing_subscriber::registry()
        .with(layer)
        .with(filter)
        .init();
}

pub fn init_tracing_tests(target_name: &str, target_level: Level, base: Option<Level>) {
    let base = base.unwrap_or(Level::ERROR);
    let base_filter = filter::Targets::new()
        // .with_target("debug_dup", Level::ERROR)
        .with_target("ploke", base)
        .with_target("ploke_tui", base)
        .with_target("ploke_embed", base)
        .with_target("ploke-db", base)
        .with_target("ploke-embed", base)
        .with_target("ploke-io", base)
        .with_target("ploke-transform", base)
        // cozo is verbose, set to Error
        .with_target("cozo", Level::ERROR)
        .with_target(target_name, target_level);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::from("")); // Default to 'info' level

    // File appender with custom timestamp format
    let log_dir = "test-logs";
    std::fs::create_dir_all(log_dir).expect("Failed to create logs directory");
    let file_appender = tracing_appender::rolling::hourly(log_dir, "ploke.log");

    // Also log to stderr so test failures print captured diagnostics without requiring manual file inspection.
    let console_subscriber = fmt::layer()
        .with_target(true)
        .with_level(true)
        .without_time()
        .with_line_number(true)
        .with_thread_ids(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(true);

    // Use try_init to avoid panicking if a global subscriber is already set (e.g., across tests)
    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(base_filter)
        .with(console_subscriber.with_writer(std::io::stderr))
        .try_init();
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
