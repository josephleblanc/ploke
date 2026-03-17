//! A Rust source code parser and analyzer.
//!
//! `syn_parser` provides tools to perform a discovery phase on a Rust crate,
//! parse all its source files into abstract syntax trees, and build a structured,

//! queryable representation of the code.
//!
//! The main entry points are [`run_phases_and_collect`] and [`run_phases_and_merge`].
//!
//! # Examples
//!
//! A typical use case involves running the parser on a fixture crate and
//! accessing the resulting code graph and module tree for analysis.
//!
//! ```no_run
//! use syn_parser::{run_phases_and_merge, GraphAccess, ParserOutput};
//!
//! fn main() -> Result<(), ploke_error::Error> {
//!     // The `fixture_name` should correspond to a directory in the `tests/fixtures`
//!     // directory of the `ploke` workspace.
//!     let fixture_name = "simple_crate";
//!
//!     // `run_phases_and_merge` handles discovery, parallel parsing, and tree construction.
//!     let mut parser_output: ParserOutput = run_phases_and_merge(fixture_name)?;
//!
//!     // Extract the code graph and module tree for analysis.
//!     if let (Some(graph), Some(tree)) = (parser_output.extract_merged_graph(), parser_output.extract_module_tree()) {
//!         println!("Successfully parsed and built module tree.");
//!         println!("Found {} functions in the graph.", graph.functions().len());
//!         println!("Module tree root: {:?}", tree.root());
//!     }
//!
//!     Ok(())
//! }
//! ```
pub mod discovery;
pub mod error;
pub mod parser;
pub mod resolve;
pub mod utils; // Don't re-export `LogStyle` to keep it clear its a utility trait.

use std::path::{Path, PathBuf};

use discovery::run_discovery_phase;
use error::SynParserError;
// Re-export PartialSuccess for internal use or if needed
pub use error::PartialSuccess;
use itertools::Itertools;
use parser::analyze_files_parallel;
// Re-export key items for easier access
pub use discovery::CrateContext;
pub use parser::visitor::analyze_file_phase2;
pub use parser::{create_parser_channel, CodeGraph, ParserMessage};
use ploke_common::fixtures_crates_dir;
pub use ploke_core::TypeId; // Re-export the enum/struct from ploke-core

// test ids
pub use parser::nodes::test_ids::TestIds;

// Main types for access in other crates
pub use parser::graph::{GraphAccess, ParsedCodeGraph};
pub use resolve::module_tree::ModuleTree;

use crate::discovery::workspace::{try_parse_manifest, WorkspaceMetadataSection};

pub fn parse_workspace(
    target_workspace_dir: &Path,
    selected_crates: Option<&[&Path]>,
) -> Result<ParsedWorkspace, SynParserError> {
    let path_buf = PathBuf::from(target_workspace_dir);
    let name = path_buf
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("No filename")
        .to_string();
    let path = path_buf.display().to_string();
    let workspace_metadata =
        try_parse_manifest(&path_buf).map_err(|e| SynParserError::ComplexDiscovery {
            name,
            path,
            source_string: e.to_string(),
        })?;

    let workspace_data =
        workspace_metadata
            .workspace
            .ok_or(SynParserError::WorkspaceSectionMissing {
                workspace_path: path_buf.display().to_string(),
            })?;

    let normalized_selected_crates =
        selected_crates.map(|crates| normalize_selected_crates(&workspace_data.path, crates));

    if let Some(selected_crates) = &normalized_selected_crates {
        let missing_selected = selected_crates
            .iter()
            .filter(|selected| !workspace_data.members.contains(selected))
            .cloned()
            .collect_vec();

        if !missing_selected.is_empty() {
            return Err(SynParserError::WorkspaceSelectionMismatch {
                workspace_path: workspace_data.path.display().to_string(),
                workspace_members: workspace_data
                    .members
                    .iter()
                    .map(|m| m.display().to_string())
                    .collect_vec(),
                selected_crates: selected_crates
                    .iter()
                    .map(|c| c.display().to_string())
                    .collect_vec(),
                missing_selected_crates: missing_selected
                    .iter()
                    .map(|c| c.display().to_string())
                    .collect_vec(),
            });
        }
    }

    let (successes, errors): (Vec<ParserOutput>, Vec<SynParserError>) = workspace_data
        .members
        .iter()
        .map(PathBuf::as_path)
        .filter(|member| {
            normalized_selected_crates
                .as_ref()
                .is_none_or(|selected| selected.contains(&member.to_path_buf()))
        })
        .map(try_run_phases_and_merge)
        .partition_result();

    if !errors.is_empty() {
        return Err(SynParserError::MultipleErrors(errors));
    }

    Ok(ParsedWorkspace {
        crates: successes
            .into_iter()
            .map(ParsedCrate::try_from)
            .collect::<Result<Vec<_>, _>>()?,
        workspace: workspace_data,
    })
}

fn normalize_selected_crates(workspace_root: &Path, selected_crates: &[&Path]) -> Vec<PathBuf> {
    selected_crates
        .iter()
        .map(|crate_path| {
            if crate_path.is_absolute() {
                crate_path.to_path_buf()
            } else {
                workspace_root.join(crate_path)
            }
        })
        .collect()
}

/// Try to run the full parsing process, returning the first error encountered. An error is
/// returned when the parse fails for any reason. The caller may determine whether to panic or
/// otherwise handle the error.
///
/// The target is assumed to be a single crate which may or may not be in a workspace. However, the
/// target dir itself must be the crate root, and is assumed to contain a crate-level (as opposed
/// to workspace-level) `Cargo.toml` file.
pub fn try_run_phases_and_resolve(
    target_crate_dir: &Path,
) -> Result<Vec<ParsedCodeGraph>, SynParserError> {
    // NOTE: Although the `run_discovery_phase` fuction takes two arguments, one for root path to a
    // workspace dir and another for the paths of multiple crates, it currently does not use the
    // argument for the root path, and so is left empty below for convenience.
    let path_buf = PathBuf::from(target_crate_dir);

    let name = path_buf
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("No filename")
        .to_string();
    let path = path_buf.display().to_string();
    let discovery_output =
        run_discovery_phase(None, &[path_buf]).map_err(|e| SynParserError::ComplexDiscovery {
            name,
            path,
            source_string: e.to_string(),
        })?;
    // NOTE:2025-12-26
    // commenting out the below so we don't panic on error in the target crate.
    // TODO: Determine whether or not the following error would be a result of an intenral error
    // (i.e. due to an error in the parsing program itself such as broken invariant) or an error in
    // the target crate's syntax (e.g. a `ub fn some_func() {}` instead of `pub fn some_func() {}`)
    // .unwrap_or_else(|e| panic!("Phase 1 Discovery failed for {}: {:?}", fixture_name, e));

    let results: Vec<Result<ParsedCodeGraph, SynParserError>> =
        analyze_files_parallel(&discovery_output, 0); // num_workers ignored by rayon bridge

    // Separate successes and errors
    let (successes_res, errors_res): (Vec<_>, Vec<_>) =
        results.into_iter().partition(Result::is_ok);

    let successes: Vec<ParsedCodeGraph> = successes_res.into_iter().map(Result::unwrap).collect();
    let error_list: Vec<SynParserError> = errors_res.into_iter().map(Result::unwrap_err).collect();

    // Check if at least one crate carries the context (critical for merging)
    if !successes.is_empty() && !successes.iter().any(|pr| pr.crate_context.is_some()) {
        return Err(SynParserError::ParsedGraphError(
            crate::parser::graph::ParsedGraphError::MissingCrateContext,
        ));
    }

    if !error_list.is_empty() {
        if successes.is_empty() {
            // All failed - return combined errors
            return Err(SynParserError::MultipleErrors(error_list));
        } else {
            // Partial success - return successes and errors
            // The caller can match on this error to retrieve the partial successes if needed.
            return Err(SynParserError::PartialParsing {
                successes: PartialSuccess(successes),
                errors: error_list,
            });
        }
    }

    Ok(successes)
}

/// Runs the discovery and parsing phases and collects the `ParsedCodeGraph`s.
///
/// This function is useful when you need to inspect the parsed data for each
/// file individually before merging.
///
/// # Arguments
///
/// * `fixture_name` - The name of the test fixture crate to run the phases on.
///
/// # Returns
///
/// A `Result` containing a `Vec` of `ParsedCodeGraph`s on success, or a
/// `SynParserError` if a critical error occurs during discovery, if all
/// files fail to parse, or if partial parsing occurs (returned as `SynParserError::PartialParsing`).
///
/// # Panics
///
/// This function will panic if the initial discovery phase fails (e.g., the
/// fixture directory or its `Cargo.toml` cannot be found). This is intended
/// for test environments where fixtures are expected to be present.
pub fn run_phases_and_collect(fixture_name: &str) -> Result<Vec<ParsedCodeGraph>, SynParserError> {
    let crate_path = fixtures_crates_dir().join(fixture_name);
    let discovery_output =
        run_discovery_phase(None, std::slice::from_ref(&crate_path)).map_err(|e| {
            SynParserError::ComplexDiscovery {
                name: fixture_name.to_string(),
                path: crate_path.display().to_string(),
                source_string: e.to_string(),
            }
        })?;
    // NOTE:2025-12-26
    // commenting out the below so we don't panic on error in the target crate.
    // TODO: Determine whether or not the following error would be a result of an intenral error
    // (i.e. due to an error in the parsing program itself such as broken invariant) or an error in
    // the target crate's syntax (e.g. a `ub fn some_func() {}` instead of `pub fn some_func() {}`)
    // .unwrap_or_else(|e| panic!("Phase 1 Discovery failed for {}: {:?}", fixture_name, e));

    let results: Vec<Result<ParsedCodeGraph, SynParserError>> =
        analyze_files_parallel(&discovery_output, 0); // num_workers ignored by rayon bridge

    // Separate successes and errors
    let (successes_res, errors_res): (Vec<_>, Vec<_>) =
        results.into_iter().partition(Result::is_ok);

    let successes: Vec<ParsedCodeGraph> = successes_res.into_iter().map(Result::unwrap).collect();
    let error_list: Vec<SynParserError> = errors_res.into_iter().map(Result::unwrap_err).collect();

    // Check if at least one crate carries the context (critical for merging)
    if !successes.is_empty() && !successes.iter().any(|pr| pr.crate_context.is_some()) {
        return Err(SynParserError::ParsedGraphError(
            crate::parser::graph::ParsedGraphError::MissingCrateContext,
        ));
    }

    if !error_list.is_empty() {
        if successes.is_empty() {
            // All failed - return combined errors
            return Err(SynParserError::MultipleErrors(error_list));
        } else {
            // Partial success - return successes and errors
            // The caller can match on this error to retrieve the partial successes if needed.
            return Err(SynParserError::PartialParsing {
                successes: PartialSuccess(successes),
                errors: error_list,
            });
        }
    }

    Ok(successes)
}

/// Runs the full parsing pipeline and returns a `ParserOutput`.
///
/// This is the primary entry point for parsing a crate. It performs the
/// discovery phase, parallel parsing of all files, merges the results into a
/// single `ParsedCodeGraph`, and finally constructs the `ModuleTree`.
///
/// # Arguments
///
/// * `fixture_name` - The name of the test fixture crate to parse.
///
/// # Returns
///
/// A `Result` containing a `ParserOutput` on success, or a `SynParserError`
/// on failure.
///
/// # Panics
///
/// This function will panic if the initial discovery phase fails, similar to
/// [`run_phases_and_collect`].
pub fn run_phases_and_merge(fixture_name: &str) -> Result<ParserOutput, ploke_error::Error> {
    let parsed_graphs = run_phases_and_collect(fixture_name)?;
    let mut merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let tree = merged.build_tree_and_prune().map_err(|err| {
        SynParserError::InternalState(format!("Failed to build module tree: {err}"))
    })?;
    Ok(ParserOutput {
        merged_graph: Some(merged),
        module_tree: Some(tree),
    })
}

#[tracing::instrument(fields(target_crate), err)]
pub fn try_run_phases_and_merge(target_crate: &Path) -> Result<ParserOutput, SynParserError> {
    let parsed_graphs = try_run_phases_and_resolve(target_crate)?;
    let mut merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let tree = merged.build_tree_and_prune().map_err(|err| {
        SynParserError::InternalState(format!("Failed to build module tree: {err}"))
    })?;
    Ok(ParserOutput {
        merged_graph: Some(merged),
        module_tree: Some(tree),
    })
}

/// Output of parsing a workspace.
pub struct ParsedWorkspace {
    pub workspace: WorkspaceMetadataSection,
    pub crates: Vec<ParsedCrate>,
}

/// Output of parsing a single crate within a workspace.
pub struct ParsedCrate {
    pub crate_context: CrateContext,
    pub parser_output: ParserOutput,
}

/// The output of the parser, containing the merged `ParsedCodeGraph` and `ModuleTree`.
pub struct ParserOutput {
    pub merged_graph: Option<ParsedCodeGraph>,
    pub module_tree: Option<ModuleTree>,
}

impl ParserOutput {
    /// Extracts the `ParsedCodeGraph` from the `ParserOutput`, leaving `None` in its place.
    pub fn extract_merged_graph(&mut self) -> Option<ParsedCodeGraph> {
        self.merged_graph.take()
    }

    /// Extracts the `ModuleTree` from the `ParserOutput`, leaving `None` in its place.
    pub fn extract_module_tree(&mut self) -> Option<ModuleTree> {
        self.module_tree.take()
    }
}

impl TryFrom<ParserOutput> for ParsedCrate {
    type Error = SynParserError;

    fn try_from(parser_output: ParserOutput) -> Result<Self, Self::Error> {
        let crate_context = parser_output
            .merged_graph
            .as_ref()
            .and_then(|graph| graph.crate_context.as_ref())
            .cloned()
            .ok_or(SynParserError::ParsedGraphError(
                crate::parser::graph::ParsedGraphError::MissingCrateContext,
            ))?;

        Ok(Self {
            crate_context,
            parser_output,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn create_workspace_fixture() -> tempfile::TempDir {
        let tmp = tempdir().unwrap();
        let workspace_root = tmp.path();

        fs::write(
            workspace_root.join("Cargo.toml"),
            r#"[workspace]
members = ["crate_a", "crate_b"]
"#,
        )
        .unwrap();

        for crate_name in ["crate_a", "crate_b"] {
            let crate_root = workspace_root.join(crate_name);
            fs::create_dir_all(crate_root.join("src")).unwrap();
            fs::write(
                crate_root.join("Cargo.toml"),
                format!(
                    r#"[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2021"
"#
                ),
            )
            .unwrap();
            fs::write(
                crate_root.join("src/lib.rs"),
                format!("pub fn {}_fn() {{}}\n", crate_name),
            )
            .unwrap();
        }

        tmp
    }

    #[test]
    fn parse_workspace_defaults_to_all_members() {
        let tmp = create_workspace_fixture();

        let parsed_workspace = parse_workspace(tmp.path(), None).unwrap();

        assert_eq!(parsed_workspace.workspace.path, tmp.path());
        assert_eq!(parsed_workspace.crates.len(), 2);
        assert_eq!(
            parsed_workspace.workspace.members,
            vec![tmp.path().join("crate_a"), tmp.path().join("crate_b")]
        );
    }

    #[test]
    fn parse_workspace_normalizes_relative_selected_crates() {
        let tmp = create_workspace_fixture();
        let selected = [Path::new("crate_b")];

        let parsed_workspace = parse_workspace(tmp.path(), Some(&selected)).unwrap();

        assert_eq!(parsed_workspace.crates.len(), 1);
        assert_eq!(
            parsed_workspace.crates[0].crate_context.root_path,
            tmp.path().join("crate_b")
        );
    }

    #[test]
    fn parse_workspace_reports_missing_selected_crates() {
        let tmp = create_workspace_fixture();
        let selected = [Path::new("crate_b"), Path::new("crate_missing")];

        let err = match parse_workspace(tmp.path(), Some(&selected)) {
            Ok(_) => panic!("expected workspace selection mismatch"),
            Err(err) => err,
        };

        match err {
            SynParserError::WorkspaceSelectionMismatch {
                workspace_path,
                missing_selected_crates,
                ..
            } => {
                assert_eq!(workspace_path, tmp.path().display().to_string());
                assert_eq!(
                    missing_selected_crates,
                    vec![tmp.path().join("crate_missing").display().to_string()]
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
