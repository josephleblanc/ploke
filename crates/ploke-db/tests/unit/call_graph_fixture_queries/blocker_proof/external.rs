use ploke_db::ProofInvariantStatus;

use super::*;

#[test]
fn fixture_projection_marks_real_external_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_prelude_string_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;
    let span = context[0].site.span;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);
    assert_targetless_blocker_proofs(
        &db,
        "external",
        &[BlockerProofSite {
            site,
            span,
            blocker_reason: "external_dependency_summary_missing",
        }],
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projected_external_call_blocker_feeds_proof_invariants() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_prelude_string_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);

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
