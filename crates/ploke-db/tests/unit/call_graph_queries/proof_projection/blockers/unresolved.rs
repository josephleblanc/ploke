use super::*;

#[test]
fn proof_projection_marks_unresolved_call_statuses() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(41);
    let module = Uuid::from_u128(42);
    let external = Uuid::from_u128(43);
    let dynamic = Uuid::from_u128(44);

    insert_targetless_status(
        &db,
        TargetlessStatusSeed::external(
            owner,
            module,
            external,
            Some("src/lib.rs"),
            &["std", "process", "Command", "new"],
            1,
        ),
    )?;
    insert_targetless_status(
        &db,
        TargetlessStatusSeed::dynamic(owner, module, dynamic, None),
    )?;

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
