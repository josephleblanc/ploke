use super::super::super::*;
use super::super::helpers::assert_blocked_resolution;

#[tokio::test]
async fn proof_context_collection_preserves_axum_await_result_receiver_blocker() -> Result<(), Error>
{
    init_tracing_once();
    let db = Arc::new(fresh_backup_fixture_db(
        &ploke_test_utils::CORPUS_AXUM_CALL_GRAPH,
    )?);
    assert!(
        db.has_call_graph_relations()?,
        "corpus_axum_call_graph must include call graph relations for proof-context tests"
    );

    let owner = axum_conn_limiter_accept_owner(&db)?;
    let projected = db.project_call_proof_facts_for_owner(owner, "bd:corpus-axum-call-graph")?;
    assert!(
        projected >= 2,
        "ConnLimiter::accept should project targetless call-site proof rows"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum call graph facts should enable RAG proof context"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let calls = call_context
        .get(&owner)
        .expect("ConnLimiter::accept should receive outgoing call context");
    let site_id = await_result_unwrap_site(calls, owner);

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .expect("ConnLimiter::accept should receive projected proof rows");

    // Matrix: awaited-result receiver row.
    // Source chain:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/serve/listener.rs:142 owns `ConnLimiter<T>::accept`.
    //   axum/src/serve/listener.rs:143 calls
    //   `self.sem.clone().acquire_owned().await.unwrap()`.
    // Expected proof traversal: owner-seeded proof context must include the
    // call_site plus blocked call_resolution facts for the exact unsupported,
    // targetless AwaitResult `unwrap` site. There are zero callee edges for
    // this row until awaited-result receiver resolution is implemented.
    assert_blocked_resolution(rows, owner, "type_resolution_missing");
    assert_blocked_resolution_for_site(rows, owner, site_id, "type_resolution_missing");

    Ok(())
}

fn axum_conn_limiter_accept_owner(db: &Database) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from("accept"));

    let rows = db.raw_query_params(
        r#"?[id, body] :=
            *method { id, name: $name, body @ 'NOW' }"#,
        params,
    )?;
    let marker = body_key("self.sem.clone().acquire_owned().await.unwrap()");
    let matching = rows
        .rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            body_key(body).contains(&marker).then(|| row[0].clone())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one axum ConnLimiter::accept owner; rows: {:#?}",
        rows.rows
    );

    to_uuid(&matching[0]).map_err(Error::from)
}

fn await_result_unwrap_site(calls: &[CallContextInfo], owner: Uuid) -> Uuid {
    let matching = calls
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Method
                && call.callee
                    == (CallCalleeInfo::Method {
                        name: "unwrap".to_string(),
                        receiver: Some(CallReceiverInfo::AwaitResult),
                    })
                && call.status == CallStatusKind::Unsupported
                && call.resolution.is_none()
                && call.targets.is_empty()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected one targetless AwaitResult unwrap call row: {calls:#?}"
    );
    matching[0].site_id
}

fn assert_blocked_resolution_for_site(
    rows: &[ProofContextInfo],
    owner: Uuid,
    site_id: Uuid,
    reason: &str,
) {
    let owner = owner.to_string();
    let site_id = site_id.to_string();
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.call_site_id.as_deref() == Some(site_id.as_str())
                && row.build_domain_id.as_deref() == Some("bd:corpus-axum-call-graph")
        }),
        "proof context should include the AwaitResult unwrap call_site fact: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site_id.as_str())
                && row.resolution_state.as_deref() == Some("blocked")
                && row.blocker_reason.as_deref() == Some(reason)
        }),
        "proof context should include the AwaitResult unwrap blocked resolution: {rows:#?}"
    );
}

fn body_key(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}
