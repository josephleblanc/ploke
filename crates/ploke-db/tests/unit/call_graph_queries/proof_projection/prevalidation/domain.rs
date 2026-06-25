use super::*;

#[test]
fn proof_projection_requires_non_empty_build_domain_id() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(0x1b1);
    let module = Uuid::from_u128(0x1b2);
    let site = Uuid::from_u128(0x1b3);
    let target = Uuid::from_u128(0x1b4);

    insert_resolved_graph(
        &db,
        ResolvedGraphSeed::path_call(owner, module, site, target, Some("src/lib.rs")),
    )?;

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
