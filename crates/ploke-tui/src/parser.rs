use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use ploke_db::Database;
use syn_parser::{
    ParsedCodeGraph, discovery::run_discovery_phase, error::SynParserError,
    parser::analyze_files_parallel,
};

/// Returns the directory to process.
/// Priority:
/// 1. Path supplied by the caller
/// 2. `$PWD` (current working directory)
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use std::env;
///
/// // When user provides a path, it should be returned
/// let user_path = PathBuf::from("/some/path");
/// let result = ploke_tui::parser::resolve_target_dir(Some(user_path.clone()));
/// assert_eq!(result.unwrap(), user_path);
///
/// // When no path is provided, it should return current directory
/// let current_dir = env::current_dir().unwrap();
/// let result = ploke_tui::parser::resolve_target_dir(None);
/// assert_eq!(result.unwrap(), current_dir);
/// ```
pub fn resolve_target_dir(user_dir: Option<PathBuf>) -> Result<PathBuf, ploke_error::Error> {
    let target_dir = match user_dir {
        Some(p) => p,
        None => env::current_dir().map_err(SynParserError::from)?,
    };
    Ok(target_dir)
}

pub fn run_parse(db: Arc<Database>, target_dir: Option<PathBuf>) -> Result<(), ploke_error::Error> {
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
    ploke_transform::transform::transform_parsed_graph(&db, merged, &tree)?;
    tracing::info!(
        "{}: Parsing and Database Transform Complete",
        "Setup".log_step()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_target_dir_with_user_path() {
        let expected_path = PathBuf::from("/tmp/test/path");
        let result = resolve_target_dir(Some(expected_path.clone()));
        assert_eq!(result.unwrap(), expected_path);
    }

    #[test]
    fn test_resolve_target_dir_without_user_path() {
        let expected_path = env::current_dir().unwrap();
        let result = resolve_target_dir(None);
        assert_eq!(result.unwrap(), expected_path);
    }

    #[test]
    fn test_resolve_target_dir_with_empty_path() {
        let empty_path = PathBuf::new();
        let result = resolve_target_dir(Some(empty_path.clone()));
        assert_eq!(result.unwrap(), empty_path);
    }
}
