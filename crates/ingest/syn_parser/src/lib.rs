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
pub mod compilation_unit;
pub mod discovery;
pub mod error;
pub mod parser;
pub mod resolve;
pub mod utils; // Don't re-export `LogStyle` to keep it clear its a utility trait.

use std::path::{Path, PathBuf};

use discovery::run_discovery_phase;
use discovery::run_discovery_phase_with_target;
use error::SynParserError;
// Re-export PartialSuccess for internal use or if needed
pub use error::PartialSuccess;
use itertools::Itertools;
use parser::analyze_files_parallel;
// Re-export key items for easier access
pub use discovery::CrateContext;
pub use discovery::ManifestCtx;
pub use discovery::ManifestKind;
pub use discovery::TargetSelector;
pub use discovery::WithDiscoveryManifestRead;
pub use discovery::WithDiscoveryManifestToml;
pub use discovery::try_parse_manifest;
pub use parser::visitor::{analyze_file_phase2, logical_module_path_for_file};
pub use parser::{CodeGraph, ParserMessage, create_parser_channel};
use ploke_common::fixtures_crates_dir;
pub use ploke_core::TypeId; // Re-export the enum/struct from ploke-core

// test ids
pub use parser::nodes::test_ids::TestIds;

// Main types for access in other crates
pub use parser::graph::{GraphAccess, ParsedCodeGraph};
pub use resolve::module_tree::ModuleTree;

use crate::compilation_unit::{CompilationUnitDimensionRequest, enumerate_compilation_unit_keys};
use crate::discovery::workspace::WorkspaceMetadataSection;
use tracing::{info_span, instrument};

/// Configuration for [`parse_workspace_with_config`].
///
/// `target_selector`, when `Some`, is passed to discovery for **each** workspace member so that
/// [`run_discovery_phase_with_target`](crate::discovery::run_discovery_phase_with_target) limits
/// Cargo targets consistently across members.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParseWorkspaceConfig<'a> {
    /// Which workspace members to parse; `None` means all members (same as [`parse_workspace`]).
    pub selected_crates: Option<&'a [&'a Path]>,
    /// When set, discovery uses this Cargo target selector for every member crate.
    pub target_selector: Option<&'a TargetSelector>,
}

#[instrument(skip_all, fields(workspace = %target_workspace_dir.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("No filename")
        .to_string()
))]
pub fn parse_workspace_with_config(
    target_workspace_dir: &Path,
    config: &ParseWorkspaceConfig<'_>,
) -> Result<ParsedWorkspace, SynParserError> {
    let path_buf = PathBuf::from(target_workspace_dir);
    let workspace_metadata = try_parse_manifest(&path_buf, discovery::ManifestKind::WorkspaceRoot)
        .map_err(SynParserError::from)?;

    let workspace_data =
        workspace_metadata
            .workspace
            .ok_or(SynParserError::WorkspaceSectionMissing {
                workspace_path: path_buf.display().to_string(),
            })?;
    tracing::info!(target: "dbg_serde", "{workspace_data:#?}");

    let normalized_selected_crates = config
        .selected_crates
        .map(|crates| normalize_selected_crates(&workspace_data.path, crates));

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
        .map(|member| {
            let rel_path = member
                .strip_prefix(target_workspace_dir)
                .unwrap_or(member)
                .to_str()
                .unwrap_or("name unknown");
            let _span = info_span!(
                "try_run_phases_and_merge",
                rel_path = %rel_path
            )
            .entered();
            try_run_phases_and_merge_with_target(member, config.target_selector)
        })
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

/// Parse a Cargo workspace (same as [`parse_workspace_with_config`] with no target selector).
pub fn parse_workspace(
    target_workspace_dir: &Path,
    selected_crates: Option<&[&Path]>,
) -> Result<ParsedWorkspace, SynParserError> {
    parse_workspace_with_config(
        target_workspace_dir,
        &ParseWorkspaceConfig {
            selected_crates,
            target_selector: None,
        },
    )
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
#[instrument()]
pub fn try_run_phases_and_resolve(
    target_crate_dir: &Path,
) -> Result<Vec<ParsedCodeGraph>, SynParserError> {
    try_run_phases_and_resolve_with_target(target_crate_dir, None)
}

/// Like [`try_run_phases_and_resolve`], but passes `selected_target` through to
/// [`run_discovery_phase_with_target`](crate::discovery::run_discovery_phase_with_target).
#[instrument(skip(selected_target))]
pub fn try_run_phases_and_resolve_with_target(
    target_crate_dir: &Path,
    selected_target: Option<&TargetSelector>,
) -> Result<Vec<ParsedCodeGraph>, SynParserError> {
    // NOTE: Although the `run_discovery_phase` fuction takes two arguments, one for root path to a
    // workspace dir and another for the paths of multiple crates, it currently does not use the
    // argument for the root path, and so is left empty below for convenience.
    let path_buf = PathBuf::from(target_crate_dir);

    let discovery_output = run_discovery_phase_with_target(None, &[path_buf], selected_target)
        .map_err(SynParserError::from)?;
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
    let discovery_output = run_discovery_phase(None, std::slice::from_ref(&crate_path))
        .map_err(SynParserError::from)?;
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
    let partition = ParsedCodeGraph::partition_by_selected_roots(parsed_graphs)?;
    let selected_root = partition.select_default_root_path()?.to_path_buf();
    let mut merged = partition.merge_for_root(&selected_root)?;
    let tree = merged
        .build_tree_and_prune_for_root_path(&selected_root)
        .map_err(|err| {
            SynParserError::InternalState(format!("Failed to build module tree: {err}"))
        })?;
    Ok(ParserOutput {
        merged_graph: Some(merged),
        module_tree: Some(tree),
        parsed_graphs_for_masks: None,
        compilation_units: None,
    })
}

/// Like [`try_run_phases_and_merge`], but merges **all** Cargo target roots into one graph and
/// retains [`ParserOutput::parsed_graphs_for_masks`] for structural compilation-unit slices.
pub fn try_run_phases_union_for_crate(target_crate: &Path) -> Result<ParserOutput, SynParserError> {
    try_run_phases_union_for_crate_with_dimensions(
        target_crate,
        &CompilationUnitDimensionRequest::from_env_or_default(),
    )
}

/// Like [`try_run_phases_union_for_crate`], but allows explicit CU dimensions for enumeration.
pub fn try_run_phases_union_for_crate_with_dimensions(
    target_crate: &Path,
    dimensions: &CompilationUnitDimensionRequest,
) -> Result<ParserOutput, SynParserError> {
    let parsed_graphs = try_run_phases_and_resolve_with_target(target_crate, None)?;
    let parsed_for_masks = parsed_graphs.clone();
    let compilation_units = parsed_for_masks
        .iter()
        .find_map(|g| g.crate_context.as_ref())
        .map(|ctx| enumerate_compilation_unit_keys(ctx.namespace, &ctx.targets, dimensions))
        .unwrap_or_default();
    let (merged, tree) = ParsedCodeGraph::merge_union_graph_and_prune_tree(parsed_graphs)?;
    Ok(ParserOutput {
        merged_graph: Some(merged),
        module_tree: Some(tree),
        parsed_graphs_for_masks: Some(parsed_for_masks),
        compilation_units: Some(compilation_units),
    })
}

pub fn try_run_phases_and_merge(target_crate: &Path) -> Result<ParserOutput, SynParserError> {
    try_run_phases_and_merge_with_target(target_crate, None)
}

/// Like [`try_run_phases_and_merge`], but passes `selected_target` through to discovery (see
/// [`try_run_phases_and_resolve_with_target`]).
#[instrument()]
pub fn try_run_phases_and_merge_with_target(
    target_crate: &Path,
    selected_target: Option<&TargetSelector>,
) -> Result<ParserOutput, SynParserError> {
    let parsed_graphs = try_run_phases_and_resolve_with_target(target_crate, selected_target)?;
    let _parsed_file_count = parsed_graphs.len();
    let partition = ParsedCodeGraph::partition_by_selected_roots(parsed_graphs)?;
    let selected_root = partition.select_default_root_path()?.to_path_buf();
    let mut merged = partition.merge_for_root(&selected_root)?;
    let tree = {
        let graph_relation_count = merged.relations().len();
        let graph_module_count = merged.modules().len();
        let _span = info_span!(
            "build_tree_and_prune",
            graph_relation_count,
            graph_module_count
        )
        .entered();
        merged.build_tree_and_prune_for_root_path(&selected_root)
    }
    .map_err(|err| SynParserError::InternalState(format!("Failed to build module tree: {err}")))?;
    Ok(ParserOutput {
        merged_graph: Some(merged),
        module_tree: Some(tree),
        parsed_graphs_for_masks: None,
        compilation_units: None,
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
    /// When set (e.g. [`try_run_phases_union_for_crate`]), original per-file graphs for CU masks.
    pub parsed_graphs_for_masks: Option<Vec<ParsedCodeGraph>>,
    /// When set (e.g. union parse), pre-enumerated compilation-unit keys for mask construction.
    pub compilation_units: Option<Vec<ploke_core::CompilationUnitKey>>,
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

    /// Extracts per-file graphs retained for structural CU masks.
    pub fn extract_parsed_graphs_for_masks(&mut self) -> Option<Vec<ParsedCodeGraph>> {
        self.parsed_graphs_for_masks.take()
    }

    /// Extracts pre-enumerated compilation-unit keys, leaving `None` in place.
    pub fn extract_compilation_units(&mut self) -> Option<Vec<ploke_core::CompilationUnitKey>> {
        self.compilation_units.take()
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
    // Test inventory for `parse_workspace` coverage in this module.
    //
    // Fixtures:
    // - `create_workspace_fixture()` builds a two-member workspace with valid Rust sources in
    //   `crate_a` and `crate_b`.
    // - `create_workspace_fixture_with_members(...)` builds a temporary workspace from explicit
    //   `(crate_name, lib_rs_source)` pairs, and is used when a test needs a malformed member.
    //
    // Tests:
    // - `parse_workspace_defaults_to_all_members`
    //   Fixture: `create_workspace_fixture()`.
    //   Verifies that `selected_crates == None` parses every workspace member and returns
    //   normalized workspace member paths.
    // - `parse_workspace_normalizes_relative_selected_crates`
    //   Fixture: `create_workspace_fixture()`.
    //   Verifies that a relative selected crate path is normalized against the workspace root and
    //   only that crate is parsed.
    // - `parse_workspace_reports_missing_selected_crates`
    //   Fixture: `create_workspace_fixture()`.
    //   Verifies that a relative selection containing a non-member returns
    //   `WorkspaceSelectionMismatch`.
    // - `parse_workspace_reports_workspace_section_missing`
    //   Fixture: ad hoc temp directory with a `[package]` manifest only.
    //   Verifies that a manifest without `[workspace]` returns `WorkspaceSectionMissing`.
    // - `parse_workspace_reports_missing_manifest_as_complex_discovery`
    //   Fixture: empty temp directory.
    //   Verifies that a missing workspace `Cargo.toml` is mapped to `ComplexDiscovery`.
    // - `parse_workspace_reports_invalid_manifest_as_complex_discovery`
    //   Fixture: temp directory with invalid workspace TOML.
    //   Verifies that an unreadable workspace manifest is mapped to `ComplexDiscovery`.
    // - `parse_workspace_accepts_absolute_selected_crates`
    //   Fixture: `create_workspace_fixture()`.
    //   Verifies that absolute selected crate paths are accepted and only the requested member is
    //   parsed.
    // - `parse_workspace_reports_mixed_selection_mismatch_details`
    //   Fixture: `create_workspace_fixture()`.
    //   Verifies mixed absolute/relative selection reporting, including `selected_crates`,
    //   `missing_selected_crates`, and `workspace_members`.
    // - `parse_workspace_aggregates_member_parse_failures`
    //   Fixture: `create_workspace_fixture_with_members(...)` with one malformed member source.
    //   Verifies that member parse failures are surfaced as `MultipleErrors` instead of being
    //   dropped.
    // - `parse_workspace_empty_selection_returns_no_crates`
    //   Fixture: `create_workspace_fixture()`.
    //   Verifies the current empty-selection contract: success with zero parsed crates and
    //   normalized workspace metadata.
    // - `parse_workspace_populates_public_dto_fields`
    //   Fixture: `create_workspace_fixture()`.
    //   Verifies that the returned `ParsedWorkspace` / `ParsedCrate` DTOs expose usable workspace
    //   identity, crate roots, merged graphs, module trees, and graph crate context.
    use std::fs;
    use std::path::Path;

    use ploke_common::{fixture_github_clones_dir, workspace_root};
    use ploke_error::DiagnosticInfo;
    use tempfile::tempdir;

    use super::*;
    use crate::discovery::{TargetKind, TargetSelector, run_discovery_phase_with_target};

    fn create_workspace_fixture_with_members(members: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempdir().unwrap();
        let workspace_root = tmp.path();
        let member_names = members
            .iter()
            .map(|(name, _)| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");

        fs::write(
            workspace_root.join("Cargo.toml"),
            format!("[workspace]\nmembers = [{member_names}]\n"),
        )
        .unwrap();

        for (crate_name, source) in members {
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
            fs::write(crate_root.join("src/lib.rs"), source).unwrap();
        }

        tmp
    }

    fn create_workspace_fixture() -> tempfile::TempDir {
        create_workspace_fixture_with_members(&[
            ("crate_a", "pub fn crate_a_fn() {}\n"),
            ("crate_b", "pub fn crate_b_fn() {}\n"),
        ])
    }

    fn committed_workspace_fixture_root(name: &str) -> PathBuf {
        workspace_root().join("tests/fixture_workspace").join(name)
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

    #[test]
    fn parse_workspace_reports_workspace_section_missing() {
        let tmp = tempdir().unwrap();

        fs::write(
            tmp.path().join("Cargo.toml"),
            r#"[package]
name = "not-a-workspace"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        let err = match parse_workspace(tmp.path(), None) {
            Ok(_) => panic!("expected workspace section error"),
            Err(err) => err,
        };

        match err {
            SynParserError::WorkspaceSectionMissing { workspace_path } => {
                assert_eq!(workspace_path, tmp.path().display().to_string());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_workspace_reports_missing_manifest_as_structured_discovery() {
        let tmp = tempdir().unwrap();

        let err = match parse_workspace(tmp.path(), None) {
            Ok(_) => panic!("expected missing manifest error"),
            Err(err) => err,
        };

        match err {
            SynParserError::Discovery(err) => match err.as_ref() {
                crate::discovery::error::DiscoveryError::ManifestRead {
                    crate_path,
                    manifest_path,
                    ..
                } => {
                    assert!(crate_path.is_none());
                    assert_eq!(manifest_path, &tmp.path().join("Cargo.toml"));
                }
                other => panic!("unexpected discovery error: {other:?}"),
            },
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_workspace_reports_invalid_manifest_as_structured_discovery() {
        let tmp = tempdir().unwrap();

        fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace\nmembers = [\"crate_a\"]\n",
        )
        .unwrap();

        let err = match parse_workspace(tmp.path(), None) {
            Ok(_) => panic!("expected invalid manifest error"),
            Err(err) => err,
        };

        match err {
            SynParserError::Discovery(err) => match err.as_ref() {
                crate::discovery::error::DiscoveryError::ManifestParse {
                    crate_path,
                    manifest_path,
                    ..
                } => {
                    assert!(crate_path.is_none());
                    assert_eq!(manifest_path, &tmp.path().join("Cargo.toml"));
                    assert_eq!(err.diagnostic_kind(), "manifest_parse");
                    assert_eq!(
                        err.diagnostic_source_path(),
                        Some(tmp.path().join("Cargo.toml").as_path())
                    );
                    let emission_site = err
                        .diagnostic_emission_site()
                        .expect("workspace manifest parse should capture emission site");
                    assert!(
                        emission_site
                            .file
                            .ends_with("crates/ingest/syn_parser/src/discovery/workspace.rs")
                    );
                    assert!(emission_site.line > 0);
                    assert!(emission_site.column > 0);
                    let backtrace = err
                        .diagnostic_backtrace()
                        .expect("workspace manifest parse should capture a backtrace");
                    assert!(!backtrace.to_string().is_empty());
                    let span = err
                        .diagnostic_span()
                        .expect("workspace manifest parse should preserve a source span");
                    assert!(span.start().is_some());
                    assert!(span.end().is_some());
                    assert_eq!(span.line(), Some(1));
                    assert_eq!(span.column(), Some(11));
                }
                other => panic!("unexpected discovery error: {other:?}"),
            },
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_workspace_accepts_absolute_selected_crates() {
        let tmp = create_workspace_fixture();
        let crate_a = tmp.path().join("crate_a");
        let selected = [crate_a.as_path()];

        let parsed_workspace = parse_workspace(tmp.path(), Some(&selected)).unwrap();

        assert_eq!(parsed_workspace.crates.len(), 1);
        assert_eq!(parsed_workspace.crates[0].crate_context.root_path, crate_a);
    }

    #[test]
    fn parse_workspace_reports_mixed_selection_mismatch_details() {
        let tmp = create_workspace_fixture();
        let crate_b = tmp.path().join("crate_b");
        let selected = [crate_b.as_path(), Path::new("crate_missing")];

        let err = match parse_workspace(tmp.path(), Some(&selected)) {
            Ok(_) => panic!("expected workspace selection mismatch"),
            Err(err) => err,
        };

        match err {
            SynParserError::WorkspaceSelectionMismatch {
                workspace_path,
                workspace_members,
                selected_crates,
                missing_selected_crates,
            } => {
                assert_eq!(workspace_path, tmp.path().display().to_string());
                assert_eq!(
                    workspace_members,
                    vec![
                        tmp.path().join("crate_a").display().to_string(),
                        tmp.path().join("crate_b").display().to_string(),
                    ]
                );
                assert_eq!(
                    selected_crates,
                    vec![
                        crate_b.display().to_string(),
                        tmp.path().join("crate_missing").display().to_string(),
                    ]
                );
                assert_eq!(
                    missing_selected_crates,
                    vec![tmp.path().join("crate_missing").display().to_string()]
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_workspace_aggregates_member_parse_failures() {
        let tmp = create_workspace_fixture_with_members(&[
            ("crate_a", "pub fn crate_a_fn() {}\n"),
            ("crate_b", "pub fn crate_b_fn( {}\n"),
        ]);

        let err = match parse_workspace(tmp.path(), None) {
            Ok(_) => panic!("expected member parse failure to bubble"),
            Err(err) => err,
        };

        match err {
            SynParserError::MultipleErrors(errors) => {
                assert!(!errors.is_empty());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_workspace_empty_selection_returns_no_crates() {
        let tmp = create_workspace_fixture();
        let selected: [&Path; 0] = [];

        let parsed_workspace = parse_workspace(tmp.path(), Some(&selected)).unwrap();

        assert_eq!(parsed_workspace.workspace.path, tmp.path());
        assert_eq!(
            parsed_workspace.workspace.members,
            vec![tmp.path().join("crate_a"), tmp.path().join("crate_b")]
        );
        assert!(parsed_workspace.crates.is_empty());
    }

    #[test]
    fn parse_workspace_populates_public_dto_fields() {
        let tmp = create_workspace_fixture();

        let parsed_workspace = parse_workspace(tmp.path(), None).unwrap();

        assert_eq!(parsed_workspace.workspace.path, tmp.path());

        let mut crate_roots = parsed_workspace
            .crates
            .iter()
            .map(|parsed_crate| {
                assert!(parsed_crate.parser_output.merged_graph.is_some());
                assert!(parsed_crate.parser_output.module_tree.is_some());
                assert_eq!(
                    parsed_crate
                        .parser_output
                        .merged_graph
                        .as_ref()
                        .and_then(|graph| graph.crate_context.as_ref())
                        .map(|ctx| ctx.root_path.clone()),
                    Some(parsed_crate.crate_context.root_path.clone())
                );
                parsed_crate.crate_context.root_path.clone()
            })
            .collect::<Vec<_>>();
        crate_roots.sort();

        assert_eq!(
            crate_roots,
            vec![tmp.path().join("crate_a"), tmp.path().join("crate_b")]
        );
    }

    #[test]
    fn parse_workspace_committed_fixture_uses_multi_member_workspace() {
        let fixture_root = committed_workspace_fixture_root("ws_fixture_01");
        let nested_member = fixture_root.join("nested/member_nested");
        let root_member = fixture_root.join("member_root");

        assert!(
            fixture_root.is_dir(),
            "fixture workspace must exist on disk"
        );

        let parsed_workspace = parse_workspace(&fixture_root, None).unwrap();

        assert_eq!(parsed_workspace.workspace.path, fixture_root);
        assert_eq!(
            parsed_workspace.workspace.members,
            vec![root_member.clone(), nested_member.clone()]
        );
        assert_eq!(parsed_workspace.crates.len(), 2);

        let mut crate_roots = parsed_workspace
            .crates
            .iter()
            .map(|parsed_crate| parsed_crate.crate_context.root_path.clone())
            .collect::<Vec<_>>();
        crate_roots.sort();

        let mut expected_roots = vec![root_member, nested_member];
        expected_roots.sort();

        assert_eq!(crate_roots, expected_roots);
    }

    #[test]
    fn parse_workspace_config_handles_non_standard_root_without_duplicate_crate_roots() {
        let workspace_root = fixture_github_clones_dir().join("serde");
        let selected_member = Path::new("serde_derive_internals");

        let parsed = parse_workspace_with_config(
            &workspace_root,
            &ParseWorkspaceConfig {
                selected_crates: Some(&[selected_member]),
                target_selector: None,
            },
        )
        .expect("workspace-config parse should succeed for serde_derive_internals");

        assert_eq!(parsed.crates.len(), 1, "expected selected member only");

        let merged_graph = parsed.crates[0]
            .parser_output
            .merged_graph
            .as_ref()
            .expect("merged graph should be available");
        let crate_root_modules: Vec<_> = merged_graph
            .modules()
            .iter()
            .filter(|module| module.path() == &vec!["crate".to_string()])
            .collect();
        assert_eq!(
            crate_root_modules.len(),
            1,
            "non-standard lib root must produce exactly one crate root module"
        );
        assert!(
            crate_root_modules[0]
                .file_path()
                .is_some_and(|path| path.ends_with("serde_derive_internals/lib.rs")),
            "crate root should resolve to serde_derive_internals/lib.rs"
        );
    }

    #[test]
    fn parse_workspace_with_config_threads_target_selector() {
        let tmp = tempdir().unwrap();
        let ws = tmp.path();
        fs::write(ws.join("Cargo.toml"), "[workspace]\nmembers = [\"solo\"]\n").unwrap();
        let solo = ws.join("solo");
        fs::create_dir_all(solo.join("src")).unwrap();
        fs::create_dir_all(solo.join("tests")).unwrap();
        fs::write(
            solo.join("Cargo.toml"),
            r#"[package]
name = "solo"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        fs::write(solo.join("src/lib.rs"), "pub fn keep() {}\n").unwrap();
        fs::write(solo.join("tests/integration.rs"), "fn it_works() {}\n").unwrap();

        let lib_sel = TargetSelector {
            kind: TargetKind::Lib,
            name: "solo".to_string(),
        };
        let parsed = parse_workspace_with_config(
            ws,
            &ParseWorkspaceConfig {
                selected_crates: None,
                target_selector: Some(&lib_sel),
            },
        )
        .expect("parse workspace with lib target selector");

        assert_eq!(parsed.crates.len(), 1);
        let mg = parsed.crates[0]
            .parser_output
            .merged_graph
            .as_ref()
            .expect("merged graph");
        let fn_names: Vec<&str> = mg.functions().iter().map(|f| f.name.as_str()).collect();
        assert!(
            fn_names.iter().any(|n| *n == "keep"),
            "expected lib function in merged graph, got {fn_names:?}"
        );
        assert!(
            !fn_names.iter().any(|n| *n == "it_works"),
            "integration test fn should not appear when lib target is selected, got {fn_names:?}"
        );
    }

    #[test]
    fn selector_limits_phase2_to_selected_target_roots() {
        let tmp = tempdir().unwrap();
        let crate_root = tmp.path().join("targeted_pkg");
        fs::create_dir_all(crate_root.join("src")).unwrap();
        fs::create_dir_all(crate_root.join("tests")).unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            r#"[package]
name = "targeted_pkg"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        fs::write(crate_root.join("src/lib.rs"), "pub fn keep() {}\n").unwrap();
        fs::write(
            crate_root.join("tests/integration.rs"),
            "fn selected_only() {}\n",
        )
        .unwrap();

        let selector = TargetSelector {
            kind: TargetKind::Test,
            name: "integration".to_string(),
        };
        let discovery = run_discovery_phase_with_target(
            None,
            std::slice::from_ref(&crate_root),
            Some(&selector),
        )
        .unwrap();
        let parsed = analyze_files_parallel(&discovery, 0);

        let parsed_files = parsed
            .into_iter()
            .map(|res| res.expect("selected target should parse"))
            .map(|graph| graph.file_path)
            .collect::<Vec<_>>();

        assert_eq!(parsed_files.len(), 1, "expected only selected target root");
        assert!(
            parsed_files[0].ends_with("tests/integration.rs"),
            "phase2 should parse selected test target root"
        );
    }

    #[test]
    fn phase2_skips_non_primary_targets_when_primary_exists() {
        let tmp = tempdir().unwrap();
        let crate_root = tmp.path().join("mixed_targets_pkg");
        fs::create_dir_all(crate_root.join("src")).unwrap();
        fs::create_dir_all(crate_root.join("benches")).unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            r#"[package]
name = "mixed_targets_pkg"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        fs::write(crate_root.join("src/lib.rs"), "pub fn keep() {}\n").unwrap();
        fs::write(crate_root.join("benches/bench_a.rs"), "fn bench_a() {}\n").unwrap();

        let discovery = run_discovery_phase(None, std::slice::from_ref(&crate_root)).unwrap();
        let parsed = analyze_files_parallel(&discovery, 0);
        let parsed_files = parsed
            .into_iter()
            .map(|res| res.expect("primary targets should parse"))
            .map(|graph| graph.file_path)
            .collect::<Vec<_>>();

        assert!(
            parsed_files.iter().any(|p| p.ends_with("src/lib.rs")),
            "expected lib source to be parsed"
        );
        assert!(
            !parsed_files
                .iter()
                .any(|p| p.ends_with("benches/bench_a.rs")),
            "legacy-safe parse mode should skip benches when lib/bin exists"
        );
    }
}
