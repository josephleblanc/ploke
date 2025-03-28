#![cfg(feature = "use_statement_tracking")]
use crate::common::parse_fixture;
use syn_parser::parser::nodes::UseStatement;

#[test]
fn test_empty_file() {
    let graph = parse_fixture("empty.rs");
    assert!(graph.use_statements.is_empty());
}

#[test]
fn test_simple_imports() {
    let graph = parse_fixture("use_statements.rs");

    let has_map = graph
        .use_statements
        .iter()
        .find(|u| u.path == vec!["std", "collections", "HashMap"]);
    assert!(
        has_map.is_some(),
        "Should find std::collections::HashMap import"
    );
}

#[test]
fn test_aliases() {
    let graph = parse_fixture("use_statements.rs");

    let fmt_alias = graph
        .use_statements
        .iter()
        .find(|u| u.path == vec!["std", "fmt"] && u.alias.as_deref() == Some("formatting"));
    assert!(fmt_alias.is_some(), "Should find fmt as formatting alias");
}

#[test]
fn test_nested_groups() {
    let graph = parse_fixture("use_statements.rs");

    let atomic_bool = graph
        .use_statements
        .iter()
        .find(|u| u.path == vec!["std", "sync", "atomic", "AtomicBool"]);
    assert!(atomic_bool.is_some(), "Should find nested atomic import");
}

#[test]
fn test_glob_imports() {
    let graph = parse_fixture("use_statements.rs");

    let glob = graph
        .use_statements
        .iter()
        .find(|u| u.path.last() == Some(&"*".to_string()));
    assert!(glob.is_some(), "Should find glob import");
}

#[test]
fn test_relative_paths() {
    let graph = parse_fixture("use_statements.rs");

    let rel_super = graph
        .use_statements
        .iter()
        .find(|u| u.path.first() == Some(&"super".to_string()));
    assert!(rel_super.is_some(), "Should find super relative import");

    let rel_crate = graph
        .use_statements
        .iter()
        .find(|u| u.path.first() == Some(&"crate".to_string()));
    assert!(rel_crate.is_some(), "Should find crate relative import");
}
