use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;
use syn_parser::parser::graph::CodeGraph;
use syn_parser::parser::types::{GenericParamKind, GenericParamNode};
use syn_parser::parser::visitor::analyze_code;
use syn_parser::parser::{nodes::*, ExtractSpan};
use thiserror::Error;

use ploke_common::{fixtures_dir, workspace_root};

#[test]
fn test_paths() {
    let fixture_path = fixtures_dir().join("my_fixture.rs");
    println!("Fixture path: {}", fixture_path.display());
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
pub fn parse_fixture(fixture_name: &str) -> Result<CodeGraph, FixtureError> {
    // let path = Path::new("tests/fixtures").join(fixture_name);
    let path = workspace_root().join("tests/fixtures").join(fixture_name);
    if !path.exists() {
        return Err(FixtureError::NotFound(path.display().to_string()));
    }
    Ok(analyze_code(&path)?)
}

/// Parse multiple fixtures and collect results                       
pub fn parse_fixtures(fixture_names: &[&str]) -> Result<Vec<CodeGraph>, FixtureError> {
    fixture_names
        .iter()
        .map(|name| parse_fixture(name))
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
    graph.functions.iter().find(|f| f.name == name)
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

/// Find an impl block for a specific type
pub fn find_impl_for_type<'a>(graph: &'a CodeGraph, type_name: &str) -> Option<&'a ImplNode> {
    // This is a simplified implementation - in a real scenario, you'd need to match
    // the type_id with the actual type name from the type graph
    graph.impls.iter().find(|impl_node| {
        if let Some(type_node) = graph
            .type_graph
            .iter()
            .find(|t| t.id == impl_node.self_type)
        {
            // This is a simplification - you'd need to extract the type name from the TypeKind
            format!("{:?}", type_node.kind).contains(type_name)
        } else {
            false
        }
    })
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
