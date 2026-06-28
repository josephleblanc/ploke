use std::collections::{BTreeMap, BTreeSet};

use cozo::DataValue;

use super::super::super::super::super::*;
use super::expected::path;

#[tokio::test]
async fn call_context_exact_reads_axum_body_empty_incoming_callers() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(fresh_backup_fixture_db(
        &ploke_test_utils::CORPUS_AXUM_CALL_GRAPH,
    )?);
    assert!(
        db.has_call_graph_relations()?,
        "corpus_axum_call_graph must include call graph relations for RAG call-context tests"
    );

    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "axum call graph backup should enable RAG call context"
    );

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        2,
        "current axum fixture should resolve exactly the two axum-core Body::empty callers: {callers:#?}"
    );

    let context = rag.exact_call_context(target)?;
    let expected_callee = CallCalleeInfo::Path {
        path: path(&["Body", "empty"]),
    };
    let incoming = context
        .iter()
        .filter(|call| {
            call.kind == CallSiteKind::Path
                && call.callee == expected_callee
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();

    // Matrix: `Body::empty` re-exported constructor row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/response/into_response.rs:128 calls `Body::empty()`.
    //   axum-core/src/response/into_response.rs:163 calls `Body::empty()`.
    // Expected traversal for the current fixture: the RAG exact call-context
    // path preserves the same two incoming caller-site edges exposed by
    // `Database::callers_for_target`. Broader axum re-export fanout remains a
    // separate import/re-export completeness gap tracked by the matrix.
    assert_eq!(
        incoming.len(),
        2,
        "RAG exact call context should expose both current Body::empty incoming edges: {context:#?}"
    );

    let expected_site_ids = callers
        .iter()
        .map(|caller| caller.site.id)
        .collect::<BTreeSet<_>>();
    let incoming_site_ids = incoming
        .iter()
        .map(|call| call.site_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        incoming_site_ids, expected_site_ids,
        "RAG call context should preserve the DB caller site identities"
    );

    for call in incoming {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::AssociatedFunction);
    }

    Ok(())
}

fn method_id_by_name_and_body_substring(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id, body] :=
            *method { id, name: $name, body @ 'NOW' }"#,
        params,
    )?;
    let normalized_marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            body_key(body)
                .contains(&normalized_marker)
                .then(|| row[0].clone())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} whose body contains {body_marker:?}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&matching[0]).map_err(Error::from)
}

fn body_key(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}
