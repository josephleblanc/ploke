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
            "blocking_reason": "cfg_domain_not_materialized"
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
