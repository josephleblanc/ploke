use super::super::*;

#[test]
fn proof_projection_stores_resolved_call_facts() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(31);
    let module = Uuid::from_u128(32);
    let site = Uuid::from_u128(33);
    let target = Uuid::from_u128(34);

    insert_resolved_graph(
        &db,
        ResolvedGraphSeed::path_call(owner, module, site, target, Some("src/lib.rs")),
    )?;

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

    insert_resolved_graph(
        &db,
        ResolvedGraphSeed::path_call(owner, module, site, target, Some("src/lib.rs")),
    )?;

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
