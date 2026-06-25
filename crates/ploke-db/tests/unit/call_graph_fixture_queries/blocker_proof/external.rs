use ploke_db::ProofInvariantStatus;

use super::*;

#[test]
fn fixture_projection_marks_real_external_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = string_new_blockers(&db)?;

    assert_targetless_blocker_proofs(&db, "external", &expected, "fixture_call_graph/src/lib.rs")?;

    Ok(())
}

#[test]
fn fixture_projected_external_call_blocker_feeds_proof_invariants() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = string_new_blockers(&db)?;
    let site = expected[0].site;

    db.upsert_proof_fact_values(&[serde_json::json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": "effect:fixture-external-call",
        "call_site_id": site.to_string(),
        "effect_class": "operating_system_process_create",
        "confidence": "fixture-test",
        "blocker_if_unresolved": true,
        "evidence_use": "proof_only"
    })])?;

    let findings = db.proof_invariant_findings()?;
    let finding = findings
        .iter()
        .find(|finding| {
            finding.invariant == "detached_process_successor_handoff"
                && finding.call_site_id.as_deref() == Some(site.to_string().as_str())
        })
        .expect("detached process finding for fixture external call proof");
    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(
        finding
            .reason
            .contains("external_dependency_summary_missing"),
        "fixture external call blocker should feed proof invariants: {finding:#?}"
    );

    Ok(())
}

fn string_new_blockers(db: &Database) -> Result<Vec<BlockerProofSite>, DbError> {
    let mut expected = Vec::new();
    assert_projected_blockers(
        db,
        &mut expected,
        "call_prelude_string_new",
        &[TargetlessBlockerCase {
            row: TargetlessRowCase::path(
                &["String", "new"],
                0,
                CallStatusKind::External,
                "String::new external",
            ),
            blocker_reason: "external_dependency_summary_missing",
        }],
    )?;
    Ok(expected)
}
