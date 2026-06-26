use super::*;

#[tokio::test]
async fn request_code_context_returns_projected_proof_context() -> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_local_target"),
    )?;
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        3,
        "single resolved path call should project call_site, call_edge, and call_resolution facts"
    );

    let tool_result =
        execute_fixture_tool_request(&db, "call_crate_local_target", 1, "projected_proof_context")
            .await?;
    let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
    assert_result_ok(&result, "call_crate_local_target", 1, "fixture_call_graph");
    assert!(
        result
            .note
            .as_deref()
            .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
        "projected proof facts should avoid degraded proof-context note: {result:#?}"
    );

    let owner_part = result
        .context
        .iter()
        .find(|part| part.id == owner)
        .expect("request_code_context should materialize the projected proof owner");
    let proof_rows = &owner_part.proof_context;
    let owner_id = owner.to_string();
    let target_id = target.to_string();
    assert_eq!(
        proof_rows.len(),
        3,
        "owner part should carry projected proof rows: {proof_rows:#?}"
    );
    let site = proof_rows
        .iter()
        .find(|row| row.kind == "call_site")
        .expect("projected proof context should include call_site fact");
    let site_id = site
        .call_site_id
        .as_deref()
        .expect("call_site proof fact should carry call_site_id");
    assert_eq!(site.caller_def_id.as_deref(), Some(owner_id.as_str()));
    assert_eq!(
        site.build_domain_id.as_deref(),
        Some("bd:fixture-call-graph")
    );
    assert!(
        proof_rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site_id)
                && row.caller_def_id.as_deref() == Some(owner_id.as_str())
                && row.callee_def_id.as_deref() == Some(target_id.as_str())
                && row.resolution_state.as_deref() == Some("resolved")
        }),
        "projected proof context should include resolved call_edge fact: {proof_rows:#?}"
    );
    assert!(
        proof_rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site_id)
                && row.resolution_state.as_deref() == Some("resolved")
                && row.resolved_def_id.as_deref() == Some(target_id.as_str())
        }),
        "projected proof context should include resolved call_resolution fact: {proof_rows:#?}"
    );

    let payload = tool_result
        .ui_payload
        .as_ref()
        .expect("request_code_context should emit a UI payload");
    let proof_context_count = result
        .context
        .iter()
        .map(|part| part.proof_context.len())
        .sum::<usize>();
    assert!(
        proof_context_count >= proof_rows.len(),
        "UI proof-context count should include at least the owner proof rows: {result:#?}"
    );
    assert_eq!(
        ui_field(payload, "proof_context"),
        proof_context_count.to_string()
    );

    Ok(())
}
