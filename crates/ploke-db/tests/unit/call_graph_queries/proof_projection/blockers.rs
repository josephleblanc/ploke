use super::super::*;

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
