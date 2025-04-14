#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    reason = "Stubs for later helper functions."
)]

use ploke_common::fixtures_dir;
use syn_parser::parser::nodes::TypeDefNode;
// TODO: Change import path of `CodeGraph` and `NodeId`, probably better organized to use `ploke-core`
use syn_parser::CodeGraph;
use syn_parser::NodeId;

// Should return result
pub fn parse_fixture(fixture_name: &str) {
    todo!()
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
