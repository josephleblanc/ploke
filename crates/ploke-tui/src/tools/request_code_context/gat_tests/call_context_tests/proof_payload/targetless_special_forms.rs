use super::super::*;
use super::helpers::{assert_blocked_resolution, assert_resolved_call};

#[tokio::test]
async fn request_code_context_returns_targetless_special_form_proof_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let extern_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_extern_c_function"),
    )?;
    let chained_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_chained_returned_function"),
    )?;
    let chained_target = one_uuid(&db, &function_in_module_query(&["crate"], "make_unary_fn"))?;

    assert_eq!(
        db.project_call_proof_facts_for_owner(extern_owner, "bd:fixture-call-graph")?,
        2,
        "extern C targetless call should project call_site and call_resolution facts"
    );
    assert_eq!(
        db.project_call_proof_facts_for_owner(chained_owner, "bd:fixture-call-graph")?,
        5,
        "chained returned-function call should project resolved and blocked proof facts"
    );

    let extern_result =
        execute_fixture_tool_request(&db, "call_extern_c_function", 1, "extern_c_proof_context")
            .await?;
    let extern_payload: RequestCodeContextResult = serde_json::from_str(&extern_result.content)?;
    assert_result_ok(
        &extern_payload,
        "call_extern_c_function",
        1,
        "fixture_call_graph",
    );
    assert!(
        extern_payload
            .note
            .as_deref()
            .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
        "projected extern C proof facts should avoid degraded proof-context note: {extern_payload:#?}"
    );
    let extern_part = extern_payload
        .context
        .iter()
        .find(|part| part.id == extern_owner)
        .expect("request_code_context should materialize the extern C proof owner");
    assert_eq!(
        extern_part.proof_context.len(),
        2,
        "extern C proof context: {extern_part:#?}"
    );
    assert_blocked_resolution(
        &extern_part.proof_context,
        extern_owner,
        "external_dependency_summary_missing",
    );
    assert_proof_blockers(&extern_result, &extern_payload);

    let chained_result = execute_fixture_tool_request(
        &db,
        "call_chained_returned_function",
        1,
        "chained_returned_proof_context",
    )
    .await?;
    let chained_payload: RequestCodeContextResult = serde_json::from_str(&chained_result.content)?;
    assert_result_ok(
        &chained_payload,
        "call_chained_returned_function",
        1,
        "fixture_call_graph",
    );
    assert!(
        chained_payload
            .note
            .as_deref()
            .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
        "projected chained returned-function proof facts should avoid degraded proof-context note: {chained_payload:#?}"
    );
    let chained_part = chained_payload
        .context
        .iter()
        .find(|part| part.id == chained_owner)
        .expect(
            "request_code_context should materialize the chained returned-function proof owner",
        );
    assert!(
        chained_part.proof_context.len() >= 5,
        "chained returned-function proof context: {chained_part:#?}"
    );
    assert_resolved_call(&chained_part.proof_context, chained_owner, chained_target);
    assert_blocked_resolution(
        &chained_part.proof_context,
        chained_owner,
        "dynamic_dispatch_unbounded",
    );
    assert_proof_blockers(&chained_result, &chained_payload);

    Ok(())
}

fn assert_proof_blockers(
    tool_result: &crate::tools::ToolResult,
    result: &RequestCodeContextResult,
) {
    let payload = tool_result
        .ui_payload
        .as_ref()
        .expect("request_code_context should emit a UI payload");
    let expected = result
        .context
        .iter()
        .flat_map(|part| part.proof_context.iter())
        .filter(|proof| proof.blocker_reason.is_some())
        .count();
    assert_eq!(ui_field(payload, "proof_blockers"), expected.to_string());
}
