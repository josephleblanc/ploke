use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;
use syn_parser::parser::graph::CodeGraph;
use syn_parser::parser::types::{GenericParamKind, GenericParamNode, TypeKind};
#[cfg(not(feature = "uuid_ids"))]
use syn_parser::parser::visitor::analyze_code;
use syn_parser::parser::{nodes::*, ExtractSpan};
use thiserror::Error;

mod paranoid;

#[cfg(not(feature = "uuid_ids"))]
use syn_parser::parser::nodes::NodeId;

#[cfg(feature = "uuid_ids")]
use {
    std::path::PathBuf, syn_parser::discovery::run_discovery_phase,
    syn_parser::parser::analyze_files_parallel, syn_parser::parser::visitor::ParsedCodeGraph,
};

use ploke_common::{fixtures_crates_dir, fixtures_dir, malformed_fixtures_dir};

pub mod uuid_ids_utils;

#[cfg(feature = "uuid_ids")]
pub fn run_phase1_phase2(fixture_name: &str) -> Vec<Result<ParsedCodeGraph, syn::Error>> {
    let crate_path = fixtures_crates_dir().join(fixture_name);
    let discovery_output = run_discovery_phase(&PathBuf::from("."), &[crate_path]) // Adjust project_root if needed
        .expect("Phase 1 Discovery failed");
    analyze_files_parallel(&discovery_output, 0) // num_workers often ignored by rayon bridge
}

#[test]
fn test_paths() {
    let fixture_path = fixtures_dir().join("my_fixture.rs");
    println!("Fixture path: {}", fixture_path.display());
}

pub fn print_typedef_names(code_graph: &CodeGraph) -> Vec<&str> {
    code_graph
        .defined_types
        .iter()
        .map(|t| match t {
            TypeDefNode::Struct(struct_node) => &struct_node.name,
            TypeDefNode::Enum(enum_node) => &enum_node.name,
            TypeDefNode::TypeAlias(type_alias_node) => &type_alias_node.name,
            TypeDefNode::Union(union_node) => union_node.name.as_str(),
        })
        .collect::<Vec<&str>>()
}

pub const FIXTURES_DIR: &str = "tests/fixtures";

#[derive(Error, Debug)]
pub enum FixtureError {
    #[error("IO error reading fixture: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error in fixture: {0}")]
    Parse(#[from] syn::Error),
    #[error("Fixture not found: {0}")]
    NotFound(String),
    #[error("Invalid UTF-8 in fixture: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("Test assertion failed: {0}")]
    Assertion(String),
}

/// Parse a fixture file and return the resulting CodeGraph or error  
#[cfg(not(feature = "uuid_ids"))]
pub fn parse_fixture(fixture_name: &str) -> Result<CodeGraph, FixtureError> {
    let path = fixtures_dir().join(fixture_name);
    if !path.exists() {
        return Err(FixtureError::NotFound(path.display().to_string()));
    }
    Ok(analyze_code(&path)?)
}

/// WARNING: Only use this for testing error handling!!!
/// Parse a malformed fixture file and return the resulting CodeGraph or error  
#[cfg(not(feature = "uuid_ids"))]
pub fn parse_fixture_malformed(malformed_fixture_name: &str) -> Result<CodeGraph, FixtureError> {
    let path = malformed_fixtures_dir().join(malformed_fixture_name);

    if !path.exists() {
        return Err(FixtureError::NotFound(path.display().to_string()));
    }
    Ok(analyze_code(&path)?) // Add more error handling here?
}

/// Parse multiple fixtures and collect results
#[cfg(not(feature = "uuid_ids"))]
pub fn parse_fixtures(fixture_names: &[&str]) -> Result<Vec<CodeGraph>, FixtureError> {
    fixture_names
        .iter()
        .map(|name| parse_fixture(name))
        .collect()
}

/// WARNING: Only use this for testing error handling!!!
/// Parse multiple malformed fixtures and collect results
#[cfg(not(feature = "uuid_ids"))]
pub fn parse_fixtures_malformed(
    malformed_fixture_names: &[&str],
) -> Result<Vec<CodeGraph>, FixtureError> {
    malformed_fixture_names
        .iter()
        .map(|name| parse_fixture_malformed(name))
        .collect()
}

/// Helper to assert a condition with a descriptive error             
pub fn assert_fixture<T>(condition: bool, message: &str, ok_value: T) -> Result<T, FixtureError> {
    if condition {
        Ok(ok_value)
    } else {
        Err(FixtureError::Assertion(message.to_string()))
    }
}

/// Find a struct by name in the code graph
pub fn find_struct_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a StructNode> {
    graph.defined_types.iter().find_map(|def| {
        if let TypeDefNode::Struct(s) = def {
            if s.name == name {
                return Some(s);
            }
        }
        None
    })
}

/// Find an enum by name in the code graph
pub fn find_enum_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a EnumNode> {
    graph.defined_types.iter().find_map(|def| {
        if let TypeDefNode::Enum(e) = def {
            if e.name == name {
                return Some(e);
            }
        }
        None
    })
}

/// Find a trait by name in the code graph
pub fn find_trait_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a TraitNode> {
    graph
        .traits
        .iter()
        .find(|t| t.name == name)
        .or_else(|| graph.private_traits.iter().find(|t| t.name == name))
}

/// Find a function by name in the code graph
pub fn find_function_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a FunctionNode> {
    if let Some(func) = graph.functions.iter().find(|f| f.name == name) {
        return Some(func);
    }

    if let Some(use_stmt) = graph
        .use_statements
        .iter()
        .find(|u| u.visible_name == name && !u.is_glob)
    {
        if let Some(original_name) = &use_stmt.original_name {
            // The last segment of the path should be the original name
            return graph.functions.iter().find(|f| &f.name == original_name);
        } else {
            // for non-renamed imports, use the last path segment
            let original_name = use_stmt.path.last()?;
            return graph.functions.iter().find(|f| &f.name == original_name);
        }
    }
    None
}

/// Reads bytes from a file at given positions
pub fn read_byte_range(path: &Path, start: usize, end: usize) -> String {
    let mut file = File::open(path).expect("Failed to open file");
    let mut buffer = vec![0; end - start];
    file.seek(std::io::SeekFrom::Start(start as u64))
        .expect("Failed to seek");
    file.read_exact(&mut buffer).expect("Failed to read bytes");
    String::from_utf8(buffer).expect("Invalid UTF-8 in span")
}

/// Verifies that a parsed item's span matches the expected text
pub fn verify_span(item: &impl ExtractSpan, path: &Path, expected: &str) {
    let (start, end) = item.extract_span_bytes();
    let actual = read_byte_range(path, start, end);

    assert_eq!(
        actual,
        expected,
        "\nSpan mismatch in {}:\nExpected:\n{}\nActual:\n{}\n",
        path.display(),
        expected,
        actual
    );
}

/// Find a module by name in the code graph
pub fn find_module_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a ModuleNode> {
    graph.modules.iter().find(|m| m.name == name)
}

pub fn find_generic_param_by_name<'a>(
    params: &'a [GenericParamNode],
    name: &str,
) -> Option<&'a GenericParamNode> {
    params.iter().find(|param| match &param.kind {
        GenericParamKind::Lifetime {
            name: param_name, ..
        } => param_name == name,
        _ => false,
    })
}

/// Helper to create module path for tests
pub fn test_module_path(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|s| s.to_string()).collect()
}

/// Find module by path segments
pub fn find_module_by_path<'a>(graph: &'a CodeGraph, path: &'a [String]) -> Option<&'a ModuleNode> {
    graph.modules.iter().find(|m| m.path == path)
}

/// Helper function for visibility testing of TypeDefNode
#[cfg(not(feature = "uuid_ids"))]
pub fn get_visibility_info<'a>(def: &'a TypeDefNode, _graph: &CodeGraph) -> (NodeId, &'a str) {
    match def {
        TypeDefNode::Struct(s) => (s.id, s.name.as_str()),
        TypeDefNode::Enum(e) => (e.id, e.name.as_str()),
        TypeDefNode::TypeAlias(a) => (a.id, a.name.as_str()),
        TypeDefNode::Union(u) => (u.id, u.name.as_str()),
    }
}

/// Find a value (const/static) by name in the code graph                
pub fn find_value_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a ValueNode> {
    graph.values.iter().find(|v| v.name == name)
}

/// Find a macro by name in the code graph
pub fn find_macro_by_name<'a>(graph: &'a CodeGraph, name: &str) -> Option<&'a MacroNode> {
    graph.macros.iter().find(|m| m.name == name)
}

pub fn find_impl_for_type<'a>(graph: &'a CodeGraph, type_name: &str) -> Option<&'a ImplNode> {
    graph.impls.iter().find(|i| {
        if let Some(self_type) = graph
            .type_graph
            .iter()
            .find(|t| t.id == i.self_type && i.trait_type.is_none())
        {
            if let TypeKind::Named { path, .. } = &self_type.kind {
                return path.last().map(|s| s == type_name).unwrap_or(false);
            }
        }
        false
    })
}

#[cfg(not(feature = "uuid_ids"))]
pub fn find_func_path_by_id(graph: &CodeGraph, fn_id: NodeId) -> Option<&Vec<String>> {
    graph
        .modules
        .iter()
        .find(|m| m.id == fn_id)
        .map(|m| &m.path)
}
