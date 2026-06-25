use super::*;

#[test]
fn generated_call_resolution_blockers_feed_proof_invariants() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let owner = Uuid::from_u128(151);
    let module = Uuid::from_u128(152);
    let external = Uuid::from_u128(153);

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
