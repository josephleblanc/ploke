use super::*;
use ploke_db::CallPathOptions;

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

    let effects = db.call_effects_reachable_from_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    let effect = effects
        .iter()
        .find(|effect| effect.effect_seed_id == "effect:external-command-new")
        .unwrap_or_else(|| {
            panic!(
                "reachable-effect query should report the generated external command sink: {effects:#?}"
            )
        });
    assert_eq!(effect.effect_class, "operating_system_process_create");
    assert_eq!(effect.confidence.as_deref(), Some("synthetic-test"));
    assert_eq!(effect.blocker_if_unresolved, Some(true));
    assert_eq!(effect.call_site.site.id, external);
    assert_eq!(effect.call_site.status.status, CallStatusKind::External);
    assert!(
        effect.paths_to_owner.is_empty(),
        "direct external-command frontier should not invent a self path: {effect:#?}"
    );
    assert!(
        effect
            .blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "reachable process effect should preserve the projected external-summary blocker: {effect:#?}"
    );

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
