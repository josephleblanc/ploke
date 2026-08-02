use super::super::*;
use ploke_db::ProofGraphStore;

fn incomplete_build_domain_record(build_domain_id: &str) -> serde_json::Value {
    serde_json::json!({
        "fact_kind": "build_domain",
        "schema_version": "ploke-proof-facts.v1",
        "build_domain_id": build_domain_id,
        "cargo_metadata_hash": "sha256:metadata",
        "cargo_lock_hash": "sha256:lock",
        "package_id": "ploke 0.1.0",
        "target_kind": "library",
        "target_name": "ploke",
        "target_root": "src/lib.rs",
        "target_triple": "x86_64-unknown-linux-gnu",
        "host_triple": "x86_64-unknown-linux-gnu",
        "profile": "dev",
        "features_hash": "sha256:features",
        "active_cfg_hash": "sha256:cfg",
        "rustc_version": "rustc 1.96.0",
        "extractor_version": "proof-graph-test",
        "proof_policy_version": "proof-policy-test",
        "evidence_use": "proof_only"
    })
}

fn assert_build_domain_blocker_reasons(rows: &[ProofContextInfo], build_domain_id: &str) {
    let mut reasons = rows
        .iter()
        .filter(|row| {
            row.kind == "build_domain"
                && row.fact_id == build_domain_id
                && row.build_domain_id.as_deref() == Some(build_domain_id)
        })
        .filter_map(|row| row.blocker_reason.as_deref())
        .collect::<Vec<_>>();
    reasons.sort_unstable();

    assert_eq!(
        reasons,
        vec![
            "cfg_domain_not_materialized",
            "rustc_invocation_evidence_missing",
        ],
        "RAG proof context should preserve every derived blocker for one proof fact: {rows:#?}"
    );
}

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

#[tokio::test]
async fn proof_context_seed_preserves_multiple_derived_blockers_for_one_fact() -> Result<(), Error>
{
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    db.ensure_proof_graph_schema().map_err(Error::from)?;
    let seed = Uuid::from_u128(0xbd0c);
    let build_domain_id = seed.to_string();
    db.upsert_proof_fact_values(&[incomplete_build_domain_record(&build_domain_id)])
        .map_err(Error::from)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let proof_context = rag.collect_proof_context(&[(seed, 1.0)])?;
    let rows = proof_context
        .get(&seed)
        .expect("build-domain seed should receive matching proof context rows");
    assert_build_domain_blocker_reasons(rows, &build_domain_id);

    Ok(())
}

#[tokio::test]
async fn proof_context_sparse_get_context_preserves_multiple_derived_blockers_for_one_fact()
-> Result<(), Error> {
    init_tracing_once();

    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_local_target"),
    )?;
    let build_domain_id = owner.to_string();
    db.ensure_proof_graph_schema().map_err(Error::from)?;
    db.upsert_proof_fact_values(&[incomplete_build_domain_record(&build_domain_id)])
        .map_err(Error::from)?;

    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.enabled = false;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.proof_context_degraded(),
        "seeded proof facts should enable RAG proof context"
    );

    let query = "call_crate_local_target";
    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.first().map(|(id, _)| *id),
        Some(owner),
        "test query should seed get_context with the owner that carries proof blockers"
    );

    let assembled = rag
        .get_context(
            query,
            1,
            &TokenBudget {
                max_total: 4096,
                per_file_max: 4096,
                per_part_max: 1024,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;
    let owner_part = assembled
        .parts
        .iter()
        .find(|part| part.id == owner)
        .expect("public get_context should preserve the proof-blocker owner seed");
    assert_build_domain_blocker_reasons(&owner_part.proof_context, &build_domain_id);

    Ok(())
}
