use super::*;
use ploke_db::ProofGraphStore;

#[tokio::test]
async fn proof_context_disabled_safely_when_facts_absent() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    assert!(
        !db.has_proof_graph_facts().map_err(Error::from)?,
        "schema-less DB must not expose populated proof facts for this regression"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        rag.proof_context_degraded(),
        "RagService must record degraded proof context when proof facts are absent"
    );
    assert!(
        !rag.cfg.proof_context.enabled,
        "degraded proof context should disable downstream proof collection"
    );

    let seed = Uuid::from_u128(0xfeed);
    let context = rag.collect_proof_context(&[(seed, 1.0)])?;
    assert!(
        context.is_empty(),
        "degraded proof context must not attach proof rows from a DB without proof facts: {context:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn proof_context_sparse_get_context_attaches_projected_owner_facts() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_local_target"),
    )?;
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3, "single resolved path call should project 3 facts");

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
        "projected proof facts should enable RAG proof context"
    );

    let proof_context = rag.collect_proof_context(&[(owner, 1.0)])?;
    let rows = proof_context
        .get(&owner)
        .expect("owner seed should receive linked proof rows");
    assert_projected_owner_rows(rows, owner, target);

    let query = "call_crate_local_target";
    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with the caller owner only"
    );
    assert_eq!(
        sparse_hits[0].0, owner,
        "test query should seed get_context with the caller owner only"
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
        .expect("public get_context should preserve the caller owner seed");
    assert_projected_owner_rows(&owner_part.proof_context, owner, target);

    Ok(())
}

#[tokio::test]
async fn proof_context_target_seed_preserves_ambiguous_dynamic_candidates() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let sibling = unique_id_by_name(&db, "function", "local_target")?;
    let target = unique_id_by_name(&db, "function", "other_target")?;
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_if_ambiguous_function_item"),
    )?;
    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    assert_eq!(
        count, 4,
        "two ambiguous dynamic call sites should each project call_site + resolution facts"
    );

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
        "projected target-centered proof facts should enable RAG proof context"
    );

    let proof_context = rag.collect_proof_context(&[(target, 1.0)])?;
    let rows = proof_context
        .get(&target)
        .expect("target candidate seed should receive linked ambiguous dynamic proof rows");
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site" && row.caller_def_id.as_deref() == Some(&owner.to_string())
        }),
        "target candidate seed should link the ambiguous dynamic call site: {rows:#?}"
    );
    let mut expected = vec![target.to_string(), sibling.to_string()];
    expected.sort();
    let resolution = rows
        .iter()
        .find(|row| {
            row.kind == "call_resolution" && row.resolution_state.as_deref() == Some("ambiguous")
        })
        .unwrap_or_else(|| {
            panic!(
                "target candidate seed should link the ambiguous dynamic resolution row: {rows:#?}"
            )
        });
    let mut actual = resolution.candidate_def_ids.clone();
    actual.sort();
    assert_eq!(
        actual, expected,
        "RAG proof context should preserve all ambiguous sibling candidates"
    );

    Ok(())
}

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

fn assert_projected_owner_rows(rows: &[ProofContextInfo], owner: Uuid, target: Uuid) {
    let owner = owner.to_string();
    let target = target.to_string();
    assert_eq!(rows.len(), 3, "projected proof context rows: {rows:#?}");
    let site = rows
        .iter()
        .find(|row| row.kind == "call_site")
        .expect("proof context should include linked call_site fact");
    let site_id = site
        .call_site_id
        .as_deref()
        .expect("call_site proof fact should carry call_site_id");
    assert_eq!(
        site.build_domain_id.as_deref(),
        Some("bd:fixture-call-graph"),
        "call_site proof fact should preserve the build domain that scopes linked rows"
    );
    assert_eq!(site.caller_def_id.as_deref(), Some(owner.as_str()));

    let edge = rows
        .iter()
        .find(|row| row.kind == "call_edge")
        .expect("proof context should include linked call_edge fact");
    assert_eq!(edge.call_site_id.as_deref(), Some(site_id));
    assert_eq!(edge.caller_def_id.as_deref(), Some(owner.as_str()));
    assert_eq!(edge.callee_def_id.as_deref(), Some(target.as_str()));
    assert_eq!(edge.resolution_state.as_deref(), Some("resolved"));

    let resolution = rows
        .iter()
        .find(|row| row.kind == "call_resolution")
        .expect("proof context should include linked call_resolution fact");
    assert_eq!(resolution.call_site_id.as_deref(), Some(site_id));
    assert_eq!(resolution.resolution_state.as_deref(), Some("resolved"));
}
