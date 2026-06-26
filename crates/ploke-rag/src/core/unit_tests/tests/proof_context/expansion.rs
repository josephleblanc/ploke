use super::super::*;
use ploke_db::ProofGraphStore;

#[tokio::test]
async fn proof_context_seed_exposes_expansion_metadata() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    db.ensure_proof_graph_schema().map_err(Error::from)?;
    let seed = Uuid::from_u128(0xe11);
    let boundary_id = seed.to_string();
    db.upsert_proof_fact_values(&[
        serde_json::json!({
            "fact_kind": "expansion_boundary",
            "schema_version": "ploke-proof-facts.v1",
            "boundary_id": boundary_id.clone(),
            "build_domain_id": "bd:rag",
            "boundary_kind": "macro_rules_invocation",
            "expansion_state": "unresolved",
            "blocking_reason": "macro_expansion_not_available",
            "source_span": {
                "file": "src/lib.rs",
                "start_byte": 90,
                "end_byte": 100
            },
            "evidence_use": "proof_only"
        }),
        serde_json::json!({
            "fact_kind": "expanded_item",
            "schema_version": "ploke-proof-facts.v1",
            "expanded_item_id": "expanded:item:rag",
            "boundary_id": seed.to_string(),
            "build_domain_id": "bd:rag",
            "definition_id": seed.to_string(),
            "source_span": {
                "file": "src/lib.rs",
                "start_byte": 100,
                "end_byte": 130
            },
            "evidence_use": "proof_only"
        }),
    ])
    .map_err(Error::from)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let proof_context = rag.collect_proof_context(&[(seed, 1.0)])?;
    let rows = proof_context
        .get(&seed)
        .expect("expansion seed should receive matching proof context rows");
    assert!(
        rows.iter().any(|row| {
            row.kind == "expansion_boundary"
                && row.boundary_id.as_deref() == Some(boundary_id.as_str())
                && row.boundary_kind.as_deref() == Some("macro_rules_invocation")
                && row.blocker_reason.as_deref() == Some("macro_expansion_not_available")
        }),
        "RAG proof context should expose expansion-boundary metadata: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "expanded_item"
                && row.expanded_item_id.as_deref() == Some("expanded:item:rag")
                && row.boundary_id.as_deref() == Some(boundary_id.as_str())
                && row.definition_id.as_deref() == Some(boundary_id.as_str())
        }),
        "RAG proof context should expose expanded-item linkage metadata: {rows:#?}"
    );

    Ok(())
}
