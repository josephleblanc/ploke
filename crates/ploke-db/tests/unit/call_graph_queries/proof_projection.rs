use super::*;

#[test]
fn proof_projection_stores_resolved_call_facts() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(31);
    let module = Uuid::from_u128(32);
    let site = Uuid::from_u128(33);
    let target = Uuid::from_u128(34);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:test")?;
    assert_eq!(count, 3);

    let edges = db.proof_checker_edges()?;
    assert_eq!(edges.len(), 1, "proof checker edges: {edges:#?}");
    assert_eq!(edges[0].call_site_id, site.to_string());
    assert_eq!(edges[0].caller_def_id, owner.to_string());
    assert_eq!(
        edges[0].callee_def_id.as_deref(),
        Some(target.to_string().as_str())
    );
    assert_eq!(edges[0].resolution_state, "resolved");
    assert!(edges[0].blocker_reason.is_none());

    let provenance = db
        .proof_source_provenance(&site.to_string())?
        .expect("projected call site source provenance");
    assert_eq!(provenance.source_file, "src/lib.rs");
    assert_eq!(provenance.start_byte, 10);
    assert_eq!(provenance.end_byte, 24);

    Ok(())
}

#[test]
fn owner_and_target_projection_share_stable_fact_identity() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x171);
    let module = Uuid::from_u128(0x172);
    let site = Uuid::from_u128(0x173);
    let target = Uuid::from_u128(0x174);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    assert_eq!(
        db.project_call_proof_facts_for_target(target, "bd:test")?,
        3
    );
    assert_eq!(
        db.proof_graphrag_context("")?.len(),
        3,
        "target-centered projection should store one call_site, one call_edge, and one call_resolution fact"
    );

    assert_eq!(db.project_call_proof_facts_for_owner(owner, "bd:test")?, 3);
    let rows = db.proof_graphrag_context("")?;
    assert_eq!(
        rows.len(),
        3,
        "owner projection of the same call site must upsert stable proof fact ids instead of duplicating rows: {rows:#?}"
    );

    let edges = db.proof_checker_edges()?;
    assert_eq!(edges.len(), 1, "proof checker edges: {edges:#?}");
    assert_eq!(edges[0].call_site_id, site.to_string());
    assert_eq!(
        edges[0].callee_def_id.as_deref(),
        Some(target.to_string().as_str())
    );

    let provenance = db
        .proof_source_provenance(&site.to_string())?
        .expect("stable projected call-site source provenance");
    assert_eq!(provenance.source_file, "src/lib.rs");
    assert_eq!(provenance.start_byte, 10);
    assert_eq!(provenance.end_byte, 24);

    Ok(())
}

#[test]
fn proof_projection_requires_non_empty_build_domain_id() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x1b1);
    let module = Uuid::from_u128(0x1b2);
    let site = Uuid::from_u128(0x1b3);
    let target = Uuid::from_u128(0x1b4);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    for error in [
        db.call_proof_facts_for_owner(owner, "")
            .expect_err("owner-scoped proof fact generation requires a build domain"),
        db.call_proof_facts_for_target(target, "")
            .expect_err("target-centered proof fact generation requires a build domain"),
        db.project_call_proof_facts_for_owner(owner, "")
            .expect_err("owner-scoped proof projection requires a build domain"),
        db.project_call_proof_facts_for_target(target, "")
            .expect_err("target-centered proof projection requires a build domain"),
    ] {
        assert!(
            error
                .to_string()
                .contains("requires non-empty build_domain_id"),
            "unexpected error: {error}"
        );
    }
    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "empty build-domain projection attempts must not store partial proof facts"
    );

    Ok(())
}

#[test]
fn proof_projection_requires_source_provenance_before_storage() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x1c1);
    let site = Uuid::from_u128(0x1c2);
    let target = Uuid::from_u128(0x1c3);

    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    let owner_error = db
        .project_call_proof_facts_for_owner(owner, "bd:test")
        .expect_err("owner-scoped projection requires source provenance");
    assert!(
        owner_error.to_string().contains("missing source file"),
        "unexpected owner projection error: {owner_error}"
    );

    let target_error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection requires caller source provenance");
    assert!(
        target_error.to_string().contains("missing source file"),
        "unexpected target projection error: {target_error}"
    );

    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "missing source provenance must not leave partial proof facts"
    );

    Ok(())
}

#[test]
fn target_centered_proof_projection_prevalidates_caller_sources_before_storage()
-> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let good_owner = Uuid::from_u128(0x1d1);
    let good_module = Uuid::from_u128(0x1d2);
    let good_site = Uuid::from_u128(0x1d3);
    let bad_owner = Uuid::from_u128(0x1d4);
    let bad_site = Uuid::from_u128(0x1d5);
    let target = Uuid::from_u128(0x1d6);

    insert_owner_source(&db, good_owner, good_module, "src/good.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: good_site,
            owner: good_owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, good_owner, good_site, "Path")?;
    insert_relation(&db, good_site, target, "Function", "Path", "Function")?;
    insert_status(&db, good_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: bad_site,
            owner: bad_owner,
            kind: "Path",
            span: (30, 44),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, bad_owner, bad_site, "Path")?;
    insert_relation(&db, bad_site, target, "Function", "Path", "Function")?;
    insert_status(&db, bad_site, "Path", "Resolved", Some("LocalExact"))?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 2, "target callers: {callers:#?}");

    let facts_error = db
        .call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered fact generation requires every caller source");
    assert!(
        facts_error.to_string().contains("missing source file"),
        "unexpected target-centered fact generation error: {facts_error}"
    );

    let projection_error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection requires every caller source");
    assert!(
        projection_error.to_string().contains("missing source file"),
        "unexpected target-centered projection error: {projection_error}"
    );

    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "target-centered source prevalidation must not store partial proof facts"
    );

    Ok(())
}

#[test]
fn proof_graphrag_context_links_generated_call_facts_by_call_site() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x181);
    let module = Uuid::from_u128(0x182);
    let site = Uuid::from_u128(0x183);
    let target = Uuid::from_u128(0x184);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (10, 24),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Resolved", Some("LocalExact"))?;

    assert_eq!(db.project_call_proof_facts_for_owner(owner, "bd:test")?, 3);

    let rows = db.proof_graphrag_context(&target.to_string())?;
    assert_eq!(
        rows.len(),
        3,
        "callee id query should return the matching edge plus linked call_site and call_resolution rows: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.caller_def_id.as_deref() == Some(owner.to_string().as_str())
        }),
        "linked call_site row missing: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.callee_def_id.as_deref() == Some(target.to_string().as_str())
        }),
        "matching call_edge row missing: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.to_string().as_str())
                && row.detail.is_none()
                && row.blocker_reason.is_none()
        }),
        "linked resolved call_resolution row missing: {rows:#?}"
    );

    Ok(())
}

#[test]
fn proof_projection_marks_unresolved_call_statuses() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(41);
    let module = Uuid::from_u128(42);
    let external = Uuid::from_u128(43);
    let dynamic = Uuid::from_u128(44);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: external,
            owner,
            kind: "Path",
            span: (30, 40),
            path: Some(vec!["std", "process", "Command", "new"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(1),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, external, "Path")?;
    insert_status(&db, external, "Path", "External", None)?;

    insert_call_site(
        &db,
        SiteSeed {
            id: dynamic,
            owner,
            kind: "Dynamic",
            span: (50, 60),
            path: None,
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: None,
        },
    )?;
    insert_edge(&db, owner, dynamic, "Dynamic")?;
    insert_status(&db, dynamic, "Dynamic", "Unsupported", None)?;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:test")?;
    assert_eq!(count, 4);
    assert!(db.proof_checker_edges()?.is_empty());

    let external_rows = db.proof_graphrag_context("external_dependency_summary_missing")?;
    assert!(
        external_rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(external.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
        }),
        "external proof rows: {external_rows:#?}"
    );

    let dynamic_rows = db.proof_graphrag_context("dynamic_dispatch_unbounded")?;
    assert!(
        dynamic_rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(dynamic.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
        }),
        "dynamic proof rows: {dynamic_rows:#?}"
    );

    Ok(())
}

#[test]
fn call_proof_facts_for_owner_preserves_mixed_resolution_shape() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(141);
    let module = Uuid::from_u128(142);
    let resolved = Uuid::from_u128(143);
    let target = Uuid::from_u128(144);
    let external = Uuid::from_u128(145);
    let dynamic = Uuid::from_u128(146);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: resolved,
            owner,
            kind: "Path",
            span: (10, 20),
            path: Some(vec!["crate", "helper"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, resolved, "Path")?;
    insert_relation(&db, resolved, target, "Function", "Path", "Function")?;
    insert_status(&db, resolved, "Path", "Resolved", Some("LocalExact"))?;

    insert_call_site(
        &db,
        SiteSeed {
            id: external,
            owner,
            kind: "Path",
            span: (30, 40),
            path: Some(vec!["String", "new"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, external, "Path")?;
    insert_status(&db, external, "Path", "External", None)?;

    insert_call_site(
        &db,
        SiteSeed {
            id: dynamic,
            owner,
            kind: "Dynamic",
            span: (50, 60),
            path: None,
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: None,
        },
    )?;
    insert_edge(&db, owner, dynamic, "Dynamic")?;
    insert_status(&db, dynamic, "Dynamic", "Unsupported", None)?;

    let facts = db.call_proof_facts_for_owner(owner, "bd:test")?;
    assert_eq!(facts.len(), 7, "proof facts: {facts:#?}");
    assert_eq!(fact_count(&facts, "call_site"), 3);
    assert_eq!(fact_count(&facts, "call_edge"), 1);
    assert_eq!(fact_count(&facts, "call_resolution"), 3);

    let edge = fact_for_call_site(&facts, "call_edge", resolved);
    assert_eq!(
        edge.get("callee_def_id")
            .and_then(serde_json::Value::as_str),
        Some(target.to_string().as_str())
    );
    assert_eq!(
        edge.get("resolution_state")
            .and_then(serde_json::Value::as_str),
        Some("resolved")
    );

    let resolved_status = fact_for_call_site(&facts, "call_resolution", resolved);
    assert_eq!(
        resolved_status
            .get("resolved_def_id")
            .and_then(serde_json::Value::as_str),
        Some(target.to_string().as_str())
    );
    assert!(
        resolved_status.get("blocking_reason").is_none(),
        "resolved status should not include a blocker: {resolved_status:#?}"
    );

    let external_status = fact_for_call_site(&facts, "call_resolution", external);
    assert_eq!(
        external_status
            .get("resolution_state")
            .and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    assert_eq!(
        external_status
            .get("blocking_reason")
            .and_then(serde_json::Value::as_str),
        Some("external_dependency_summary_missing")
    );

    let dynamic_status = fact_for_call_site(&facts, "call_resolution", dynamic);
    assert_eq!(
        dynamic_status
            .get("resolution_state")
            .and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    assert_eq!(
        dynamic_status
            .get("blocking_reason")
            .and_then(serde_json::Value::as_str),
        Some("dynamic_dispatch_unbounded")
    );

    Ok(())
}

#[test]
fn generated_call_resolution_blockers_feed_proof_invariants() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(151);
    let module = Uuid::from_u128(152);
    let external = Uuid::from_u128(153);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: external,
            owner,
            kind: "Path",
            span: (30, 40),
            path: Some(vec!["std", "process", "Command", "new"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(1),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, external, "Path")?;
    insert_status(&db, external, "Path", "External", None)?;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:test")?;
    assert_eq!(count, 2);

    db.upsert_proof_fact_values(&[serde_json::json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": "effect:external-command-new",
        "call_site_id": external.to_string(),
        "effect_class": "operating_system_process_create",
        "confidence": "synthetic-test",
        "blocker_if_unresolved": true,
        "evidence_use": "proof_only"
    })])?;

    let findings = db.proof_invariant_findings()?;
    let finding = findings
        .iter()
        .find(|finding| {
            finding.invariant == "detached_process_successor_handoff"
                && finding.call_site_id.as_deref() == Some(external.to_string().as_str())
        })
        .expect("detached process finding for generated external call proof");
    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(
        finding
            .reason
            .contains("external_dependency_summary_missing"),
        "finding should be blocked by generated external call resolution: {finding:#?}"
    );

    Ok(())
}

#[test]
fn proof_projection_rejects_non_resolved_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(51);
    let module = Uuid::from_u128(52);
    let site = Uuid::from_u128(53);
    let target = Uuid::from_u128(54);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Path",
            span: (70, 90),
            path: Some(vec!["unknown"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Path")?;
    insert_relation(&db, site, target, "Function", "Path", "Function")?;
    insert_status(&db, site, "Path", "Unresolved", None)?;

    let error = db
        .project_call_proof_facts_for_owner(owner, "bd:test")
        .expect_err("unresolved call site with local targets must reject");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(db.proof_graphrag_context("")?.is_empty());

    Ok(())
}

#[test]
fn proof_projection_rejects_ambiguous_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x151);
    let module = Uuid::from_u128(0x152);
    let site = Uuid::from_u128(0x153);
    let target = Uuid::from_u128(0x154);

    insert_owner_source(&db, owner, module, "src/lib.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Method",
            span: (100, 130),
            path: None,
            method: Some("overlap"),
            macro_name: None,
            receiver: Some(("LocalBinding", vec!["value"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Method")?;
    insert_relation(&db, site, target, "Method", "Method", "Method")?;
    insert_status(&db, site, "Method", "Ambiguous", None)?;

    let error = db
        .project_call_proof_facts_for_owner(owner, "bd:test")
        .expect_err("ambiguous call site with local targets must reject");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(db.proof_graphrag_context("")?.is_empty());

    Ok(())
}

#[test]
fn target_centered_proof_projection_rejects_non_resolved_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let resolved_owner = Uuid::from_u128(61);
    let resolved_module = Uuid::from_u128(62);
    let unresolved_owner = Uuid::from_u128(63);
    let unresolved_module = Uuid::from_u128(64);
    let target = Uuid::from_u128(65);
    let resolved_site = Uuid::from_u128(66);
    let unresolved_site = Uuid::from_u128(67);

    insert_owner_source(&db, resolved_owner, resolved_module, "src/resolved.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: resolved_site,
            owner: resolved_owner,
            kind: "Path",
            span: (10, 30),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, resolved_owner, resolved_site, "Path")?;
    insert_relation(&db, resolved_site, target, "Function", "Path", "Function")?;
    insert_status(&db, resolved_site, "Path", "Resolved", Some("LocalExact"))?;

    insert_owner_source(
        &db,
        unresolved_owner,
        unresolved_module,
        "src/unresolved.rs",
    )?;
    insert_call_site(
        &db,
        SiteSeed {
            id: unresolved_site,
            owner: unresolved_owner,
            kind: "Path",
            span: (40, 60),
            path: Some(vec!["crate", "target"]),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, unresolved_owner, unresolved_site, "Path")?;
    insert_relation(&db, unresolved_site, target, "Function", "Path", "Function")?;
    insert_status(&db, unresolved_site, "Path", "Unresolved", None)?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 2, "target callers: {callers:#?}");

    let error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection must reject inconsistent incoming rows");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "target-centered projection must not store partial proof facts after rejection"
    );

    Ok(())
}

#[test]
fn target_centered_proof_projection_rejects_ambiguous_local_targets() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x161);
    let module = Uuid::from_u128(0x162);
    let target = Uuid::from_u128(0x163);
    let site = Uuid::from_u128(0x164);

    insert_owner_source(&db, owner, module, "src/ambiguous.rs")?;
    insert_call_site(
        &db,
        SiteSeed {
            id: site,
            owner,
            kind: "Method",
            span: (140, 170),
            path: None,
            method: Some("overlap"),
            macro_name: None,
            receiver: Some(("LocalBinding", vec!["value"])),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(&db, owner, site, "Method")?;
    insert_relation(&db, site, target, "Method", "Method", "Method")?;
    insert_status(&db, site, "Method", "Ambiguous", None)?;

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), 1, "target callers: {callers:#?}");

    let error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection must reject ambiguous incoming rows");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "target-centered projection must not store partial proof facts after rejection"
    );

    Ok(())
}
