use super::super::*;

#[tokio::test]
async fn request_code_context_preserves_multiple_proof_blockers_for_one_fact()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_local_target"),
    )?;
    let build_domain_id = owner.to_string();
    db.ensure_proof_graph_schema()?;
    db.upsert_proof_fact_values(&[serde_json::json!({
        "fact_kind": "build_domain",
        "schema_version": "ploke-proof-facts.v1",
        "build_domain_id": build_domain_id.clone(),
        "cargo_metadata_hash": "sha256:metadata",
        "cargo_lock_hash": "sha256:lock",
        "package_id": "ploke 0.1.0",
        "target_kind": "library",
        "target_name": "ploke",
        "target_root": "src/lib.rs",
        "target_triple": "x86_64-unknown-linux-gnu",
        "host_triple": "x86_64-unknown-linux-gnu",
        "profile": "dev",
        "features_hash": "sha256:features",
        "active_cfg_hash": "sha256:cfg",
        "rustc_version": "rustc 1.96.0",
        "extractor_version": "proof-graph-test",
        "proof_policy_version": "proof-policy-test",
        "evidence_use": "proof_only"
    })])?;

    let tool_result = execute_fixture_tool_request(
        &db,
        "call_crate_local_target",
        1,
        "multi_proof_blocker_context",
    )
    .await?;
    let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
    assert_result_ok(&result, "call_crate_local_target", 1, "fixture_call_graph");
    assert!(
        result
            .note
            .as_deref()
            .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
        "seeded proof facts should avoid degraded proof-context note: {result:#?}"
    );

    let owner_part = result
        .context
        .iter()
        .find(|part| part.id == owner)
        .expect("request_code_context should materialize the seeded owner");
    let mut reasons = owner_part
        .proof_context
        .iter()
        .filter(|row| {
            row.kind == "build_domain"
                && row.fact_id == build_domain_id
                && row.build_domain_id.as_deref() == Some(build_domain_id.as_str())
        })
        .filter_map(|row| row.blocker_reason.as_deref())
        .collect::<Vec<_>>();
    reasons.sort_unstable();
    assert_eq!(
        reasons,
        vec![
            "cfg_domain_not_materialized",
            "rustc_invocation_evidence_missing",
        ],
        "request_code_context should preserve every derived blocker for one proof fact: {owner_part:#?}"
    );

    let payload = tool_result
        .ui_payload
        .as_ref()
        .expect("request_code_context should emit a UI payload");
    let expected_proof_blockers = result
        .context
        .iter()
        .flat_map(|part| part.proof_context.iter())
        .filter(|proof| proof.blocker_reason.is_some())
        .count();
    assert_eq!(
        ui_field(payload, "proof_blockers"),
        expected_proof_blockers.to_string()
    );

    Ok(())
}
