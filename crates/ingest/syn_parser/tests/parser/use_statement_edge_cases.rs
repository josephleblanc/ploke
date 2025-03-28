#![cfg(feature = "use_statement_tracking")]
use crate::common::parse_fixture;
// use syn_parser::parser::nodes::UseStatement;

#[test]
fn test_edge_case_imports() {
    let graph = parse_fixture("use_statement_edge_cases.rs")
        .unwrap_or_else(|e| panic!(
            "Error parsing fixture 'use_statement_edge_cases.rs': {}\n\
            This likely indicates the test file itself has invalid syntax or the parser \
            doesn't support some valid syntax yet",
            e
        ));

    #[cfg(feature = "use_statement_tracking")]
    {
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
                && u.alias == Some("y".to_string())),
            "Multiple renames failed"
        );

        // 3. Test empty path segments
        assert!(
            uses.iter()
                .any(|u| u.path == vec!["", "", "module"] && u.visible_name == "module"),
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
}

#[test]
fn test_invalid_use_statements() {
    let graph = parse_fixture("invalid_uses.rs");
    assert!(
        graph.is_ok(), 
        "Should handle invalid syntax gracefully\nGot error: {:?}",
        graph.err()
    );

    #[cfg(feature = "use_statement_tracking")]
    {
        assert!(
            graph.unwrap().use_statements.is_empty(),
            "Invalid syntax should produce no use statements"
        );
    }
}
