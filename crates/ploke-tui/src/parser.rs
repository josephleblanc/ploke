use std::{
    env,
    path::PathBuf,
    sync::Arc,
};

use ploke_db::Database;
use ploke_io::path_policy::{normalize_target_path, PathPolicy};
use syn_parser::{
    ModuleTree, ParsedCodeGraph, ParserOutput, discovery::run_discovery_phase,
    error::SynParserError, parser::analyze_files_parallel, try_run_phases_and_merge,
};
use tracing::instrument;

/// Returns the directory to process, resolving relative paths and canonicalizing them.
/// Priority:
/// 1. Path supplied by the caller
/// 2. `$PWD` (current working directory)
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// use std::env;
/// use std::fs;
///
/// // When user provides a path, it should be returned
/// let user_path = env::current_dir().unwrap();
/// let result = ploke_tui::parser::resolve_target_dir(Some(user_path.clone()));
/// assert_eq!(result.unwrap(), fs::canonicalize(user_path).unwrap());
///
/// // When no path is provided, it should return current directory
/// let current_dir = env::current_dir().unwrap();
/// let result = ploke_tui::parser::resolve_target_dir(None);
/// assert_eq!(result.unwrap(), fs::canonicalize(current_dir).unwrap());
/// ```
#[instrument(fields(user_dir), err)]
pub fn resolve_target_dir(user_dir: Option<PathBuf>) -> Result<PathBuf, ploke_error::Error> {
    let target_dir = match user_dir {
        Some(p) => p,
        None => env::current_dir().map_err(SynParserError::from)?,
    };
    let abs_target = if target_dir.is_absolute() {
        target_dir
    } else {
        env::current_dir()
            .map_err(SynParserError::from)?
            .join(target_dir)
    };
    let policy = PathPolicy::new(vec![abs_target.clone()]);
    let normalized = normalize_target_path(&abs_target, &policy)?;
    Ok(normalized)
}

pub fn run_parse(db: Arc<Database>, target_dir: Option<PathBuf>) -> Result<(), ploke_error::Error> {
    use syn_parser::utils::LogStyle;

    let target = resolve_target_dir(target_dir)?;
    tracing::info!(
        "{}: run the parser on {}",
        "Parse".log_step(),
        target.display()
    );

    // let discovery_output = run_discovery_phase(&target, &[target.clone()])
    //     .map_err(ploke_error::Error::from)
    //     .inspect_err(|e| {
    //         tracing::error!("discovery error: {e:?}");
    //     })?;

    let parser_output = try_run_phases_and_merge(&target)?;
    // let graphs: Vec<_> = results
    //     .into_iter()
    //     .collect::<Result<_, _>>()
    //     .inspect_err(|e| {
    //         tracing::error!("error during parse: {e:?}");
    //     })
    //     .map_err(ploke_error::Error::from)?;

    tracing::info!(
        "{}: Parsing and Database Transform Complete",
        "Setup".log_step()
    );
    Ok(())
}

#[instrument(err, fields(target_dir), skip(db))]
pub fn run_parse_no_transform(
    db: Arc<Database>,
    target_dir: Option<PathBuf>,
) -> Result<ParserOutput, ploke_error::Error> {
    use syn_parser::utils::LogStyle;

    let target = resolve_target_dir(target_dir)?;
    tracing::info!(
        "{}: run the parser on {}",
        "Parse".log_step(),
        target.display()
    );

    let discovery_output =
        run_discovery_phase(&target, &[target.clone()]).map_err(ploke_error::Error::from)?;

    let results: Vec<Result<ParsedCodeGraph, SynParserError>> =
        analyze_files_parallel(&discovery_output, 0);

    let graphs: Vec<_> = results
        .into_iter()
        .collect::<Result<_, _>>()
        .map_err(ploke_error::Error::from)?;

    let mut merged = ParsedCodeGraph::merge_new(graphs)?;
    let tree = merged.build_tree_and_prune()?;
    Ok(ParserOutput {
        merged_graph: Some(merged),
        module_tree: Some(tree),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_target_dir_with_user_path() {
        let dir = tempdir().unwrap();
        let expected_path = std::fs::canonicalize(dir.path()).unwrap();
        let result = resolve_target_dir(Some(dir.path().to_path_buf()));
        assert_eq!(result.unwrap(), expected_path);
    }

    #[test]
    fn test_resolve_target_dir_without_user_path() {
        let expected_path = std::fs::canonicalize(env::current_dir().unwrap()).unwrap();
        let result = resolve_target_dir(None);
        assert_eq!(result.unwrap(), expected_path);
    }

    #[test]
    fn test_resolve_target_dir_with_empty_path() {
        let expected_path = std::fs::canonicalize(env::current_dir().unwrap()).unwrap();
        let result = resolve_target_dir(Some(PathBuf::new()));
        assert_eq!(result.unwrap(), expected_path);
    }
}
