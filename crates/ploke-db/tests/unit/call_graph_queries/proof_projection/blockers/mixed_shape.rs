use super::*;

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

    insert_resolved_graph(
        &db,
        ResolvedGraphSeed {
            owner,
            module,
            site: resolved,
            target,
            file: Some("src/lib.rs"),
            span: (10, 20),
            path: &["crate", "helper"],
        },
    )?;

    insert_targetless_status(
        &db,
        TargetlessStatusSeed::external(owner, module, external, None, &["String", "new"], 0),
    )?;
    insert_targetless_status(
        &db,
        TargetlessStatusSeed::dynamic(owner, module, dynamic, None),
    )?;

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
