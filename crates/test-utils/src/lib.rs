use ploke_common::fixtures_dir;
use syn_parser::parser::visitor::analyze_code;

pub fn parse_fixture(
    fixture_name: &str,
) -> Result<syn_parser::parser::graph::CodeGraph, syn::Error> {
    let path = fixtures_dir().join(fixture_name);
    analyze_code(&path)
}

pub fn parse_malformed_fixture(
    fixture_name: &str,
) -> Result<syn_parser::parser::graph::CodeGraph, syn::Error> {
    let path = fixtures_dir().join(fixture_name);
    analyze_code(&path)
}

/// Find a function node by name in a CodeGraph
pub fn find_function_by_name(graph: &CodeGraph, name: &str) -> Option<NodeId> {
    graph.functions.iter()
        .find(|f| f.name == name)
        .map(|f| f.id)
}

/// Find a struct node by name in a CodeGraph  
pub fn find_struct_by_name(graph: &CodeGraph, name: &str) -> Option<NodeId> {
    graph.defined_types.iter()
        .find_map(|t| match t {
            TypeDefNode::Struct(s) if s.name == name => Some(s.id),
            _ => None
        })
}

/// Find a module node by path in a CodeGraph
#[cfg(feature = "module_path_tracking")]
pub fn find_module_by_path(graph: &CodeGraph, path: &[String]) -> Option<NodeId> {
    graph.modules.iter()
        .find(|m| m.path == path)
        .map(|m| m.id)
}

/// Helper to create module path for testing
pub fn test_module_path(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|s| s.to_string()).collect()
}
