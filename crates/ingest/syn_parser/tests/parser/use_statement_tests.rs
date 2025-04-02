#![cfg(feature = "use_statement_tracking")]
use crate::common::parse_fixture;

#[test]
fn test_empty_file() {
    let graph = parse_fixture("empty.rs").expect("Error parsing fixture empty.rs");
    assert!(graph.use_statements.is_empty());
}

#[test]
fn test_simple_imports() {
    let graph =
        parse_fixture("use_statements.rs").expect("Error parsing fixture use_statements.rs");

    let has_map = graph
        .use_statements
        .iter()
        .find(|u| u.path == vec!["std", "collections", "HashMap"]);
    assert!(
        has_map.is_some(),
        "Should find std::collections::HashMap import\nFound use statements: {:#?}",
        graph.use_statements
    );
}

#[test]
fn test_aliases() {
    let graph =
        parse_fixture("use_statements.rs").expect("Error parsing fixture use_statements.rs");

    let fmt_alias = graph.use_statements.iter().find(|u| {
        u.path == vec!["std", "fmt"]
            && u.visible_name == "formatting"
            && u.original_name == Some("fmt".to_owned())
    });
    // #[cfg(features = "debug_print")]
    assert!(
        fmt_alias.is_some(),
        "Should find fmt as formatting alias\nFound use statements:\n{:#?}\nExpected path: {:?} with alias: 'formatting'",
        graph.use_statements,
        vec!["std", "fmt"]
    );
}

#[test]
fn test_nested_groups() {
    let graph =
        parse_fixture("use_statements.rs").expect("Error parsing fixture use_statements.rs");

    let atomic_bool = graph
        .use_statements
        .iter()
        .find(|u| u.path == vec!["std", "sync", "atomic", "AtomicBool"]);
    assert!(atomic_bool.is_some(), "Should find nested atomic import");
}

#[test]
fn test_glob_imports() {
    let graph =
        parse_fixture("use_statements.rs").expect("Error parsing fixture use_statements.rs");

    let expected_glob_path = ["std", "prelude", "v1"];

    let glob = graph
        .use_statements
        .iter()
        .find(|u| u.path == expected_glob_path && u.is_glob);
    assert!(
        glob.is_some(),
        "Should find glob import, instead found: {:#?}\nFound use statements:\n{:#?}\nExpected a glob import ending with '*'",
        glob,
        graph.use_statements
    );
}

#[test]
fn test_relative_paths() {
    let graph =
        parse_fixture("use_statements.rs").expect("Error parsing fixture use_statements.rs");

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
