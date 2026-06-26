use super::*;

#[tokio::test]
async fn request_code_context_ui_payload_reports_context_carrier_counts() -> color_eyre::Result<()>
{
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));

    let tool_result =
        execute_fixture_tool_request(&db, "local_target", 1, "context_count_fields").await?;
    let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
    assert_result_ok(&result, "local_target", 1, "fixture_call_graph");

    let payload = tool_result
        .ui_payload
        .as_ref()
        .expect("request_code_context should emit a UI payload");

    let expected_call_context = result
        .context
        .iter()
        .map(|part| part.call_context.len())
        .sum::<usize>();
    let expected_call_blockers = result
        .context
        .iter()
        .flat_map(|part| part.call_context.iter())
        .filter(|call| call.status != CallStatusKind::Resolved)
        .count();
    let expected_type_context = result
        .context
        .iter()
        .filter(|part| part.type_context.is_some())
        .count();
    let expected_call_expansion = result
        .context
        .iter()
        .filter(|part| part.call_expansion.is_some())
        .count();
    let expected_proof_context = result
        .context
        .iter()
        .map(|part| part.proof_context.len())
        .sum::<usize>();
    let expected_proof_blockers = result
        .context
        .iter()
        .flat_map(|part| part.proof_context.iter())
        .filter(|proof| proof.blocker_reason.is_some())
        .count();

    assert_eq!(
        ui_field(payload, "type_context"),
        expected_type_context.to_string()
    );
    assert_eq!(
        ui_field(payload, "call_context"),
        expected_call_context.to_string()
    );
    assert_eq!(
        ui_field(payload, "call_blockers"),
        expected_call_blockers.to_string()
    );
    assert_eq!(
        ui_field(payload, "call_expansion"),
        expected_call_expansion.to_string()
    );
    assert_eq!(
        ui_field(payload, "proof_context"),
        expected_proof_context.to_string()
    );
    assert_eq!(
        ui_field(payload, "proof_blockers"),
        expected_proof_blockers.to_string()
    );

    Ok(())
}
