pub mod discovery;
pub mod error;
pub mod parser;
pub mod resolve;
pub mod utils; // Don't re-export `LogStyle` to keep it clear its a utility trait.

use discovery::run_discovery_phase;
use error::SynParserError;
use parser::analyze_files_parallel;
// Re-export key items for easier access
pub use parser::visitor::analyze_file_phase2;
pub use parser::{create_parser_channel, CodeGraph, ParserMessage};
use ploke_common::{fixtures_crates_dir, workspace_root};
pub use ploke_core::TypeId; // Re-export the enum/struct from ploke-core

// test ids
pub use parser::nodes::test_ids::TestIds;

// Main types for access in other crates
pub use parser::graph::ParsedCodeGraph;
pub use resolve::module_tree::ModuleTree;

pub fn run_phases_and_collect(fixture_name: &str) -> Result< Vec<ParsedCodeGraph>, SynParserError > {
    let crate_path = fixtures_crates_dir().join(fixture_name);
    let project_root = workspace_root(); // Use workspace root for context
    let discovery_output = run_discovery_phase(&project_root, &[crate_path.clone()])
        .unwrap_or_else(|e| panic!("Phase 1 Discovery failed for {}: {:?}", fixture_name, e));

    let results: Vec<Result<ParsedCodeGraph, SynParserError>> =
        analyze_files_parallel(&discovery_output, 0); // num_workers ignored by rayon bridge
    // Separate successes and errors
    let (successes, errors): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);

    if !errors.is_empty() {
        // Convert Vec<Result<T, E>> to Vec<E>
        let error_list: Vec<SynParserError> = errors.into_iter().map(Result::unwrap_err).collect();

        if successes.is_empty() {
            // All failed - return combined errors
            return Err(SynParserError::MultipleErrors(error_list));
        } else {
            // Some succeeded - log errors but continue
            eprintln!("{} files had errors:\n{}",
                error_list.len(),
                error_list.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
            );
        }
    }

    // Unwrap all successes (we know they're Ok)
    Ok(successes.into_iter().map(Result::unwrap).collect())   
}

pub fn run_phases_and_merge(fixture_name: &str) -> Result<ParserOutput, SynParserError> {
    let parsed_graphs = run_phases_and_collect(fixture_name)?;
    let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
    let tree = merged.build_module_tree()?;
    Ok(ParserOutput {
        merged_graph: Some(merged),
        module_tree: Some(tree),
    })
}

#[allow(dead_code, reason = "Primary output of this crate, not used locally")]
pub struct ParserOutput {
    merged_graph: Option<ParsedCodeGraph>,
    module_tree: Option<ModuleTree>,
}

impl ParserOutput {
    pub fn extract_merged_graph(&mut self) -> Option<ParsedCodeGraph> {
        self.merged_graph.take()
    }

    pub fn extract_module_tree(&mut self) -> Option<ModuleTree> {
        self.module_tree.take()
    }
}
