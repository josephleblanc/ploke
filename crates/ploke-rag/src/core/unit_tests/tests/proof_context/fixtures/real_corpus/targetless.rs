use super::super::super::super::*;
use super::super::super::helpers::assert_blocked_resolution;
use super::helpers::{
    AXUM_DOMAIN, assert_site_blocker, await_result_unwrap_site, axum_db, conn_limiter_accept_owner,
};

#[tokio::test]
async fn proof_context_collection_preserves_axum_await_result_receiver_blocker() -> Result<(), Error>
{
    init_tracing_once();
    let db = axum_db()?;

    let owner = conn_limiter_accept_owner(&db)?;
    let projected = db.project_call_proof_facts_for_owner(owner, AXUM_DOMAIN)?;
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
    assert_site_blocker(rows, owner, site_id, "type_resolution_missing");

    Ok(())
}
