use std::{env, path::{Path, PathBuf}, sync::Arc};

use ploke_db::Database;
use syn_parser::{discovery::run_discovery_phase, error::SynParserError, parser::analyze_files_parallel, ParsedCodeGraph};

/// Returns the directory to process.
/// Priority:
/// 1. Path supplied by the caller
/// 2. `$PWD` (current working directory)
pub fn resolve_target_dir(user_dir: Option<PathBuf>) -> Result<PathBuf, ploke_error::Error> {
    match user_dir {
        Some(p) => Ok(p),
        None => env::current_dir().map_err(Into::into),
    }
}


pub fn run_parse(db: Arc<Database>, target_dir: Option<PathBuf>) -> Result<(), ploke_error::Error> {
    use syn_parser::utils::LogStyle;

    let target = resolve_target_dir(target_dir)?;
    tracing::info!("{}: run the parser on {}", "Parse".log_step(), target.display());

    let discovery_output = run_discovery_phase(&target, &[target.clone()])
        .map_err(Into::into)?;

    let results: Vec<Result<ParsedCodeGraph, SynParserError>> =
        analyze_files_parallel(&discovery_output, 0);

    let graphs: Vec<_> = results.into_iter().collect::<Result<_, _>>()
        .map_err(Into::into)?;

    let merged = ParsedCodeGraph::merge_new(graphs)?;
    let tree = merged.build_module_tree()?;
    ploke_transform::transform::transform_parsed_graph(&db, merged, &tree)?;
    tracing::info!("{}: Parsing and Database Transform Complete", "Setup".log_step());
    Ok(())
}

