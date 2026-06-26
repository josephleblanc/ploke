use super::super::*;
use ploke_db::ProofGraphStore;

#[tokio::test]
async fn proof_context_seed_exposes_derived_blocker_reasons() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    db.ensure_proof_graph_schema().map_err(Error::from)?;
    let seed = Uuid::from_u128(0xdeed);
    db.upsert_proof_fact_values(&[
        serde_json::json!({
            "fact_kind": "call_site",
            "schema_version": "ploke-proof-facts.v1",
            "call_site_id": "call:derived",
            "build_domain_id": "bd:rag",
            "caller_def_id": seed.to_string(),
            "source_span": {
                "file": "src/lib.rs",
                "start_byte": 10,
                "end_byte": 20
            },
            "evidence_use": "proof_only"
        }),
        serde_json::json!({
            "fact_kind": "call_edge",
            "schema_version": "ploke-proof-facts.v1",
            "call_edge_id": "edge:derived",
            "call_site_id": "call:derived",
            "caller_def_id": seed.to_string(),
            "resolution_state": "unresolved",
            "evidence_use": "proof_only"
        }),
    ])
    .map_err(Error::from)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let proof_context = rag.collect_proof_context(&[(seed, 1.0)])?;
    let rows = proof_context
        .get(&seed)
        .expect("seed should receive linked proof context rows");
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some("call:derived")
                && row.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "RAG proof context should expose derived blocker reasons: {rows:#?}"
    );

    Ok(())
}
