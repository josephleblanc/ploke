use std::{env, fmt, path::PathBuf, sync::Arc};

use ploke_db::Database;
use ploke_io::path_policy::{PathPolicy, normalize_target_path};
use ploke_transform::transform::{transform_parsed_graph, transform_parsed_workspace};
use syn_parser::{
    ModuleTree, ParsedCodeGraph, ParserOutput,
    discovery::run_discovery_phase,
    discovery::workspace::{locate_workspace_manifest, try_parse_manifest},
    error::SynParserError,
    parse_workspace,
    parser::analyze_files_parallel,
    try_run_phases_and_merge,
};
use tracing::instrument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexTargetKind {
    Crate,
    Workspace,
}

#[derive(Debug, Clone)]
pub struct ResolvedIndexTarget {
    pub requested_path: PathBuf,
    pub focused_root: PathBuf,
    pub workspace_root: PathBuf,
    pub member_roots: Vec<PathBuf>,
    pub kind: IndexTargetKind,
}

#[derive(Debug)]
pub enum IndexTargetResolveError {
    Resolution(SynParserError),
    WorkspaceDiscovery(String),
    NoCrateOrWorkspace { requested_path: PathBuf },
}

impl fmt::Display for IndexTargetResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolution(err) => write!(f, "{err}"),
            Self::WorkspaceDiscovery(message) => write!(f, "{message}"),
            Self::NoCrateOrWorkspace { requested_path } => write!(
                f,
                "No crate root or workspace root was found for '{}'. Run `/index start <path>` from a crate root containing `Cargo.toml`, or from inside a Cargo workspace so Ploke can find the nearest ancestor workspace manifest.",
                requested_path.display()
            ),
        }
    }
}

pub fn resolve_index_target(
    user_dir: Option<PathBuf>,
) -> Result<ResolvedIndexTarget, IndexTargetResolveError> {
    let requested_path =
        resolve_target_dir(user_dir).map_err(IndexTargetResolveError::Resolution)?;
    let local_manifest = requested_path.join("Cargo.toml");

    if local_manifest.is_file() {
        let manifest = try_parse_manifest(&requested_path).map_err(|err| {
            IndexTargetResolveError::WorkspaceDiscovery(format!(
                "Failed to read Cargo manifest at '{}': {}",
                local_manifest.display(),
                err
            ))
        })?;

        if let Some(workspace) = manifest.workspace {
            let focused_root = workspace
                .members
                .first()
                .cloned()
                .unwrap_or_else(|| workspace.path.clone());
            return Ok(ResolvedIndexTarget {
                requested_path,
                focused_root,
                workspace_root: workspace.path,
                member_roots: workspace.members,
                kind: IndexTargetKind::Workspace,
            });
        }

        return Ok(ResolvedIndexTarget {
            requested_path: requested_path.clone(),
            focused_root: requested_path.clone(),
            workspace_root: requested_path.clone(),
            member_roots: vec![requested_path.clone()],
            kind: IndexTargetKind::Crate,
        });
    }

    match locate_workspace_manifest(&requested_path) {
        Ok((_manifest_path, metadata)) => {
            let workspace = metadata.workspace.ok_or_else(|| {
                IndexTargetResolveError::WorkspaceDiscovery(format!(
                    "Workspace discovery for '{}' succeeded but did not yield a `[workspace]` section.",
                    requested_path.display()
                ))
            })?;
            let focused_root = workspace
                .members
                .first()
                .cloned()
                .unwrap_or_else(|| workspace.path.clone());
            Ok(ResolvedIndexTarget {
                requested_path,
                focused_root,
                workspace_root: workspace.path,
                member_roots: workspace.members,
                kind: IndexTargetKind::Workspace,
            })
        }
        Err(syn_parser::discovery::DiscoveryError::WorkspaceManifestNotFound { .. }) => {
            Err(IndexTargetResolveError::NoCrateOrWorkspace { requested_path })
        }
        Err(err) => Err(IndexTargetResolveError::WorkspaceDiscovery(format!(
            "Failed to locate a workspace manifest above '{}': {}",
            requested_path.display(),
            err
        ))),
    }
}

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
pub fn resolve_target_dir(user_dir: Option<PathBuf>) -> Result<PathBuf, SynParserError> {
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
    let normalized = normalize_target_path(&abs_target, &policy).map_err(|err| {
        SynParserError::InternalState(format!("Failed to normalize target path: {err}"))
    })?;
    Ok(normalized)
}

pub fn run_parse(db: Arc<Database>, target_dir: Option<PathBuf>) -> Result<(), SynParserError> {
    let resolved = resolve_index_target(target_dir)
        .map_err(|err| SynParserError::InternalState(err.to_string()))?;
    run_parse_resolved(db, &resolved)
}

pub fn run_parse_resolved(
    db: Arc<Database>,
    resolved: &ResolvedIndexTarget,
) -> Result<(), SynParserError> {
    use syn_parser::utils::LogStyle;

    tracing::info!(
        "{}: run the parser on {} ({:?})",
        "Parse".log_step(),
        resolved.requested_path.display(),
        resolved.kind
    );

    match resolved.kind {
        IndexTargetKind::Crate => {
            let mut parser_output = try_run_phases_and_merge(&resolved.focused_root)?;
            let merged = parser_output.extract_merged_graph().ok_or_else(|| {
                SynParserError::InternalState("Missing parsed code graph".to_string())
            })?;
            let tree = parser_output
                .extract_module_tree()
                .ok_or_else(|| SynParserError::InternalState("Missing module tree".to_string()))?;
            transform_parsed_graph(&db, merged, &tree).map_err(|err| {
                SynParserError::InternalState(format!("Failed to transform parsed graph: {err}"))
            })?;
        }
        IndexTargetKind::Workspace => {
            let parsed_workspace = parse_workspace(&resolved.workspace_root, None)?;
            transform_parsed_workspace(&db, parsed_workspace).map_err(|err| {
                SynParserError::InternalState(format!(
                    "Failed to transform parsed workspace: {err}"
                ))
            })?;
        }
    }

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
) -> Result<ParserOutput, SynParserError> {
    use syn_parser::utils::LogStyle;

    let target = resolve_target_dir(target_dir)?;
    tracing::info!(
        "{}: run the parser on {}",
        "Parse".log_step(),
        target.display()
    );

    let discovery_output = run_discovery_phase(None, &[target.clone()])
        .map_err(|err| SynParserError::try_from(err).unwrap_or_else(|err| err))?;

    let results: Vec<Result<ParsedCodeGraph, SynParserError>> =
        analyze_files_parallel(&discovery_output, 0);

    let graphs: Vec<_> = results.into_iter().collect::<Result<_, _>>()?;

    let mut merged = ParsedCodeGraph::merge_new(graphs)?;
    let tree = merged.build_tree_and_prune().map_err(|err| {
        SynParserError::InternalState(format!("Failed to build module tree: {err}"))
    })?;
    Ok(ParserOutput {
        merged_graph: Some(merged),
        module_tree: Some(tree),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_test_utils::workspace_root;
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

    #[test]
    fn resolve_index_target_prefers_crate_root_when_pwd_is_crate_root() {
        let crate_root = workspace_root().join("tests/fixture_workspace/ws_fixture_01/member_root");

        let resolved = resolve_index_target(Some(crate_root.clone())).expect("resolve crate root");

        assert_eq!(resolved.kind, IndexTargetKind::Crate);
        assert_eq!(resolved.focused_root, crate_root);
        assert_eq!(resolved.workspace_root, resolved.focused_root);
        assert_eq!(resolved.member_roots, vec![resolved.focused_root]);
    }

    #[test]
    fn resolve_index_target_finds_workspace_when_pwd_is_not_crate_root() {
        let nested_src =
            workspace_root().join("tests/fixture_workspace/ws_fixture_01/nested/member_nested/src");
        let fixture_workspace_root = workspace_root().join("tests/fixture_workspace/ws_fixture_01");

        let resolved =
            resolve_index_target(Some(nested_src)).expect("resolve ancestor workspace from src");

        assert_eq!(resolved.kind, IndexTargetKind::Workspace);
        assert_eq!(resolved.workspace_root, fixture_workspace_root);
        assert_eq!(
            resolved.member_roots,
            vec![
                fixture_workspace_root.join("member_root"),
                fixture_workspace_root.join("nested/member_nested"),
            ]
        );
    }

    #[test]
    fn resolve_index_target_reports_missing_crate_or_workspace() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("not_a_cargo_target");
        std::fs::create_dir_all(&target).unwrap();

        let err = resolve_index_target(Some(target.clone())).expect_err("missing target error");

        match err {
            IndexTargetResolveError::NoCrateOrWorkspace { requested_path } => {
                assert_eq!(requested_path, std::fs::canonicalize(target).unwrap());
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
