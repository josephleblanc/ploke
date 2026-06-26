use super::super::*;
use ploke_db::ProofGraphStore;

#[tokio::test]
async fn proof_context_seed_exposes_build_domain_metadata() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    db.ensure_proof_graph_schema().map_err(Error::from)?;
    let seed = Uuid::from_u128(0xbd0);
    let build_domain_id = seed.to_string();
    db.upsert_proof_fact_values(&[serde_json::json!({
        "fact_kind": "build_domain",
        "schema_version": "ploke-proof-facts.v1",
        "build_domain_id": build_domain_id.clone(),
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
    })])
    .map_err(Error::from)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let proof_context = rag.collect_proof_context(&[(seed, 1.0)])?;
    let rows = proof_context
        .get(&seed)
        .expect("build-domain seed should receive matching proof context rows");
    assert!(
        rows.iter().any(|row| {
            row.kind == "build_domain"
                && row.build_domain_id.as_deref() == Some(build_domain_id.as_str())
                && row.target_kind.as_deref() == Some("library")
                && row.target_name.as_deref() == Some("ploke")
                && row.target_root.as_deref() == Some("src/lib.rs")
                && row.profile.as_deref() == Some("dev")
                && row.rustc_version.as_deref() == Some("rustc 1.96.0")
                && row.proof_policy_version.as_deref() == Some("proof-policy-test")
        }),
        "RAG proof context should expose build-domain metadata: {rows:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_seed_exposes_cfg_and_rustc_metadata() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    db.ensure_proof_graph_schema().map_err(Error::from)?;
    let seed = Uuid::from_u128(0xc96);
    let build_domain_id = seed.to_string();
    db.upsert_proof_fact_values(&[
        serde_json::json!({
            "fact_kind": "cfg_domain",
            "schema_version": "ploke-proof-facts.v1",
            "cfg_domain_id": "cfg:rag",
            "build_domain_id": build_domain_id.clone(),
            "active_cfg_hash": "sha256:cfg",
            "status": "blocked",
            "blocking_reason": "cfg_domain_not_materialized",
            "evidence_use": "proof_only"
        }),
        serde_json::json!({
            "fact_kind": "rustc_invocation",
            "schema_version": "ploke-proof-facts.v1",
            "invocation_id": "rustc:rag",
            "build_domain_id": build_domain_id.clone(),
            "rustc_program": "rustc",
            "rustc_version": "rustc 1.96.0",
            "working_directory": "/workspace/ploke",
            "argument_vector_hash": "sha256:argv",
            "environment_hash": "sha256:env",
            "status": "blocked",
            "blocking_reason": "rustc_invocation_evidence_missing",
            "evidence_use": "proof_only"
        }),
    ])
    .map_err(Error::from)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let proof_context = rag.collect_proof_context(&[(seed, 1.0)])?;
    let rows = proof_context
        .get(&seed)
        .expect("cfg/rustc seed should receive matching proof context rows");
    assert!(
        rows.iter().any(|row| {
            row.kind == "cfg_domain"
                && row.build_domain_id.as_deref() == Some(build_domain_id.as_str())
                && row.cfg_domain_id.as_deref() == Some("cfg:rag")
                && row.active_cfg_hash.as_deref() == Some("sha256:cfg")
                && row.blocker_reason.as_deref() == Some("cfg_domain_not_materialized")
        }),
        "RAG proof context should expose cfg-domain metadata: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "rustc_invocation"
                && row.build_domain_id.as_deref() == Some(build_domain_id.as_str())
                && row.invocation_id.as_deref() == Some("rustc:rag")
                && row.rustc_program.as_deref() == Some("rustc")
                && row.working_directory.as_deref() == Some("/workspace/ploke")
                && row.argument_vector_hash.as_deref() == Some("sha256:argv")
                && row.environment_hash.as_deref() == Some("sha256:env")
                && row.rustc_version.as_deref() == Some("rustc 1.96.0")
                && row.blocker_reason.as_deref() == Some("rustc_invocation_evidence_missing")
        }),
        "RAG proof context should expose rustc-invocation metadata: {rows:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_seed_exposes_effect_seed_metadata() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    db.ensure_proof_graph_schema().map_err(Error::from)?;
    let seed = Uuid::from_u128(0xefc);
    let call_site_id = seed.to_string();
    db.upsert_proof_fact_values(&[
        serde_json::json!({
            "fact_kind": "call_site",
            "schema_version": "ploke-proof-facts.v1",
            "call_site_id": call_site_id,
            "build_domain_id": "bd:rag",
            "caller_def_id": "def:effect",
            "source_span": {
                "file": "src/lib.rs",
                "start_byte": 40,
                "end_byte": 64
            },
            "evidence_use": "proof_only"
        }),
        serde_json::json!({
            "fact_kind": "effect_seed",
            "schema_version": "ploke-proof-facts.v1",
            "effect_seed_id": "effect:rag",
            "call_site_id": seed.to_string(),
            "effect_class": "operating_system_process_create",
            "confidence": "command-spawn",
            "blocker_if_unresolved": true,
            "evidence_use": "proof_only"
        }),
    ])
    .map_err(Error::from)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let proof_context = rag.collect_proof_context(&[(seed, 1.0)])?;
    let rows = proof_context
        .get(&seed)
        .expect("effect seed should receive linked proof context rows");
    assert!(
        rows.iter().any(|row| {
            row.kind == "effect_seed"
                && row.call_site_id.as_deref() == Some(call_site_id.as_str())
                && row.effect_seed_id.as_deref() == Some("effect:rag")
                && row.effect_class.as_deref() == Some("operating_system_process_create")
                && row.confidence.as_deref() == Some("command-spawn")
                && row.blocker_if_unresolved == Some(true)
        }),
        "RAG proof context should expose effect-seed metadata: {rows:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_seed_exposes_authority_terms() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    db.ensure_proof_graph_schema().map_err(Error::from)?;
    let seed = Uuid::from_u128(0xa17);
    db.upsert_proof_fact_values(&[serde_json::json!({
        "fact_kind": "authority",
        "schema_version": "ploke-proof-facts.v1",
        "authority_fact_id": seed.to_string(),
        "build_domain_id": "bd:rag",
        "authority_term": "successor",
        "status": "admitted",
        "evidence_use": "proof_only",
        "source_span": {
            "file": "src/lib.rs",
            "start_byte": 30,
            "end_byte": 40
        }
    })])
    .map_err(Error::from)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let proof_context = rag.collect_proof_context(&[(seed, 1.0)])?;
    let rows = proof_context
        .get(&seed)
        .expect("authority seed should receive matching proof context rows");
    assert!(
        rows.iter().any(|row| {
            row.kind == "authority"
                && row.authority_term.as_deref() == Some("successor")
                && row.status.as_deref() == Some("admitted")
        }),
        "RAG proof context should expose authority terms explicitly: {rows:#?}"
    );

    Ok(())
}
