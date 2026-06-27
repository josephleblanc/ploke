use super::super::*;
use super::helpers::assert_projected_owner_rows;

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
    assert_projected_owner_rows(proof_rows, owner, target);

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

#[tokio::test]
async fn request_code_context_returns_expanded_method_proof_context() -> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let method_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_typed_local_instance_method"),
    )?;
    let nested_ref_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_typed_double_reference_local_instance_method",
        ),
    )?;
    let method_callers = [
        (method_owner, "method-call owner"),
        (nested_ref_owner, "nested-reference method owner"),
    ];
    for &(owner, label) in &method_callers {
        assert_eq!(
            db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
            3,
            "{label} should project call_site, call_edge, and call_resolution facts"
        );
    }

    let tool_result =
        execute_fixture_tool_request(&db, "instance_value", 1, "expanded_method_proof_context")
            .await?;
    let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
    assert_result_ok(&result, "instance_value", 1, "fixture_call_graph");
    assert!(
        result
            .note
            .as_deref()
            .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
        "projected method proof facts should avoid degraded proof-context note: {result:#?}"
    );

    for &(owner, label) in &method_callers {
        let part = result
            .context
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| {
                panic!("request_code_context should materialize the {label} proof context")
            });
        assert!(
            part.call_expansion.is_some(),
            "{label} should be present because call-context expansion required it"
        );
        assert_projected_owner_rows(&part.proof_context, owner, target);
    }

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
        proof_context_count >= method_callers.len() * 3,
        "UI proof-context count should include both expanded method caller proof rows: {result:#?}"
    );
    assert_eq!(
        ui_field(payload, "proof_context"),
        proof_context_count.to_string()
    );

    Ok(())
}
