use ploke_db::{Database, DbError, PrettyDebug, QueryContext};

// fn pretty(err: &DbError) -> Option<String> {
//     err.cozo_query_fields()
//         .and_then(|fields| serde_json::to_string_pretty(&fields).ok())
// }
//
#[test]
fn raw_query_with_context_captures_callsite() {
    let _ = tracing_subscriber::fmt::try_init();
    let db = Database::new_init().expect("init db");

    let query = QueryContext::new("MissingRelationDemo", "?[x] := *nonexistent{x}");

    let err = db
        .raw_query_with_context(query)
        .expect_err("missing relation should error");

    let display = err.to_string();
    let structured = err
        .pretty_json()
        .expect("cozo fields available for pretty JSON");

    match &err {
        DbError::CozoQuery {
            query_name,
            file,
            line,
            column,
            message,
        } => {
            assert_eq!(query_name, &query.name);
            assert!(file.ends_with("callsite_logging_tests.rs"));
            assert!(*line > 0);
            assert!(*column > 0);
            assert!(
                message.contains("nonexistent")
                    || message.contains("stored relation")
                    || !message.is_empty()
            );
        }
        other => panic!("expected CozoQuery error, got {other:?}"),
    }

    tracing::error!(error = %err, ?err, "demonstrating callsite-aware Cozo error");
    tracing::error!(error = %structured, ?structured, "demonstrating structured callsite-aware Cozo error");
    assert!(
        display.contains("callsite_logging_tests.rs"),
        "display should surface the Rust callsite: {display}"
    );
    assert!(
        structured.contains("\"query_name\": \"MissingRelationDemo\"")
            && structured.contains("callsite_logging_tests.rs"),
        "structured view should include query name and file: {structured}"
    );
}
