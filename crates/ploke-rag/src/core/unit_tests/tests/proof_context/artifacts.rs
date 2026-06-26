use super::super::*;
use ploke_db::ProofGraphStore;

#[tokio::test]
async fn proof_context_seed_exposes_external_summary_ids() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    db.ensure_proof_graph_schema().map_err(Error::from)?;
    let seed = Uuid::from_u128(0x5eed);
    let summary_id = seed.to_string();
    db.upsert_proof_fact_values(&[serde_json::json!({
        "fact_kind": "call_resolution",
        "schema_version": "ploke-proof-facts.v1",
        "call_site_id": "call:external",
        "resolution_state": "externally_summarized",
        "external_summary_id": summary_id,
        "blocking_reason": "external_dependency_summary_missing"
    })])
    .map_err(Error::from)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "stored proof facts should enable RAG proof context"
    );

    let proof_context = rag.collect_proof_context(&[(seed, 1.0)])?;
    let rows = proof_context
        .get(&seed)
        .expect("external summary seed should receive matching proof rows");
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some("call:external")
                && row.resolution_state.as_deref() == Some("externally_summarized")
                && row.external_summary_id.as_deref() == Some(summary_id.as_str())
        }),
        "RAG proof context should expose external_summary_id: {rows:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_seed_exposes_external_summary_artifact_detail() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    db.ensure_proof_graph_schema().map_err(Error::from)?;
    let seed = Uuid::from_u128(0x5efe);
    let summary_id = seed.to_string();
    db.upsert_proof_fact_values(&[serde_json::json!({
        "fact_kind": "external_summary",
        "schema_version": "ploke-proof-facts.v1",
        "external_summary_id": summary_id,
        "build_domain_id": "bd:rag",
        "summary_class": "opaque_blocked",
        "artifact_hash": "sha256:external-artifact",
        "version": "external 1.0.0",
        "review_method": "manual-review",
        "scope_of_validity": "rag test fixture",
        "allowed_effects": ["external_summary_boundary"],
        "required_containment": "none",
        "invalidation_conditions": "artifact hash or proof policy changes",
        "status": "blocked",
        "evidence_use": "proof_only"
    })])
    .map_err(Error::from)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let proof_context = rag.collect_proof_context(&[(seed, 1.0)])?;
    let rows = proof_context
        .get(&seed)
        .expect("external summary seed should receive matching proof rows");
    assert!(
        rows.iter().any(|row| {
            row.kind == "external_summary"
                && row.fact_id == summary_id
                && row.build_domain_id.as_deref() == Some("bd:rag")
                && row.summary_class.as_deref() == Some("opaque_blocked")
                && row.artifact_hash.as_deref() == Some("sha256:external-artifact")
                && row.summary_version.as_deref() == Some("external 1.0.0")
                && row.review_method.as_deref() == Some("manual-review")
                && row.scope_of_validity.as_deref() == Some("rag test fixture")
                && row.allowed_effects == vec!["external_summary_boundary".to_string()]
                && row.required_containment.as_deref() == Some("none")
                && row.invalidation_conditions.as_deref()
                    == Some("artifact hash or proof policy changes")
                && row.status.as_deref() == Some("blocked")
                && row.detail.as_deref() == Some("opaque_blocked")
        }),
        "RAG proof context should expose external summary artifact details: {rows:#?}"
    );

    Ok(())
}
