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
//! use syn_parser::{run_phases_and_merge, ParserOutput, error::SynParserError};
//!
//! fn main() -> Result<(), SynParserError> {
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
use parser::analyze_files_parallel;
// Re-export key items for easier access
pub use parser::visitor::analyze_file_phase2;
pub use parser::{create_parser_channel, CodeGraph, ParserMessage};
use ploke_common::{fixtures_crates_dir, workspace_root};
pub use ploke_core::TypeId; // Re-export the enum/struct from ploke-core

// test ids
pub use parser::nodes::test_ids::TestIds;

// Main types for access in other crates
pub use parser::graph::{GraphAccess, ParsedCodeGraph};
pub use resolve::module_tree::ModuleTree;
use tracing::instrument;

/// Try to run the full parsing process, returning the first error encountered. An error is
/// returned when the parse fails for any reason. The caller may determine whether to panic or
/// otherwise handle the error.
///
/// The target is assumed to be a single crate which may or may not be in a workspace. However, the
/// target dir itself must be the crate root, and is assumed to contain a crate-level (as opposed
/// to workspace-level) `Cargo.toml` file.
#[instrument(err, fields(target_crate_dir))]
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
    let discovery_output = run_discovery_phase(&Path::new(""), std::slice::from_ref(&path_buf))
        .map_err(|e| SynParserError::ComplexDiscovery {
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
    let project_root = workspace_root(); // Use workspace root for context
    let discovery_output = run_discovery_phase(&project_root, std::slice::from_ref(&crate_path))
        .map_err(|e| SynParserError::ComplexDiscovery {
            name: fixture_name.to_string(),
            path: crate_path.display().to_string(),
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
    let tree = merged.build_tree_and_prune()?;
    Ok(ParserOutput {
        merged_graph: Some(merged),
        module_tree: Some(tree),
    })
}

#[tracing::instrument(fields(target_crate), err)]
pub fn try_run_phases_and_merge(target_crate: &Path) -> Result<ParserOutput, ploke_error::Error> {
    let parsed_graphs = try_run_phases_and_resolve(target_crate)?;
    let mut merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let tree = merged.build_tree_and_prune()?;
    Ok(ParserOutput {
        merged_graph: Some(merged),
        module_tree: Some(tree),
    })
}

/// The output of the parser, containing the merged `ParsedCodeGraph` and `ModuleTree`.
#[allow(dead_code, reason = "Primary output of this crate, not used locally")]
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
