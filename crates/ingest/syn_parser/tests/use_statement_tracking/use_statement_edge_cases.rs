#![cfg(feature = "use_statement_tracking")]
use crate::common::{parse_fixture, parse_fixture_malformed, FixtureError};
// use syn_parser::parser::nodes::ImportNode;

#[test]
fn test_edge_case_imports() {
    let graph = parse_fixture("use_statement_edge_cases.rs")
        .expect("Test fixture should be valid Rust syntax");

    let uses = &graph.use_statements;

    // 1. Test deeply nested paths
    assert!(
        uses.iter()
            .any(|u| u.path == vec!["a", "b", "c", "d", "e", "f"] && u.visible_name == "f"),
        "Deeply nested path failed"
    );

    // 2. Test multiple renames in one statement
    assert!(
        uses.iter().any(|u| u.path == vec!["x", "y"]
            && u.visible_name == "z"
            && u.original_name == Some("y".to_string())),
        "Multiple renames failed"
    );

    // 3. Test repeated `self` path segments
    assert!(
        uses.iter()
            .any(|u| u.path == vec!["self", "self", "module"] && u.visible_name == "module"),
        "Empty path segments failed"
    );

    // 4. Test UTF-8 identifiers
    assert!(
        uses.iter()
            .any(|u| u.path == vec!["模块", "子模块"] && u.visible_name == "类型"),
        "UTF-8 path handling failed"
    );

    // 5. Test raw identifiers
    assert!(
        uses.iter()
            .any(|u| u.path == vec!["r#mod", "r#type"] && u.visible_name == "r#var"),
        "Raw identifiers failed"
    );
}

// WARNING: This parses a malformed rust file, and should ONLY be used to test error handling.
#[test]
fn test_invalid_use_statements() {
    let result = parse_fixture_malformed("invalid_use.rs");

    // First verify we got the right error type
    match result {
        Err(FixtureError::Parse(syn_err)) => {
            // Check for specific error kind if needed
            let msg = syn_err.to_string();
            assert!(
                msg.contains("expected one of") && msg.contains("identifier"),
                "Expected syntax error about identifiers, got: {}",
                msg
            );
        }
        Err(e) => panic!("Expected parse error, got: {:?}", e),
        Ok(_) => panic!("Expected error for invalid syntax"),
    }
}
