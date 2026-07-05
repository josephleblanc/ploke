use super::super::assertions::path;
use super::super::*;
use super::helpers::assert_projected_owner_rows;

#[tokio::test]
async fn request_code_context_returns_closure_owner_projected_proof_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "closure_body_call_is_not_outer_call_site"),
    )?;
    let closure = closure_owner_for_parent(&db, outer)?;
    assert_owner_projected_proof_context(
        &db,
        target,
        closure,
        "closure",
        "closure_owner_projected_proof_context",
    )
    .await
}

#[tokio::test]
async fn request_code_context_returns_async_block_owner_projected_proof_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "async_block_call_is_not_outer_call_site"),
    )?;
    let async_body = async_block_owner_for_parent(&db, outer)?;
    assert_owner_projected_proof_context(
        &db,
        target,
        async_body,
        "async block",
        "async_block_owner_projected_proof_context",
    )
    .await
}

#[tokio::test]
async fn request_code_context_returns_async_closure_owner_projected_proof_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_async_closure_literal_with_body_call"),
    )?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;
    assert_owner_projected_proof_context(
        &db,
        target,
        async_closure,
        "async closure",
        "async_closure_owner_projected_proof_context",
    )
    .await
}

async fn assert_owner_projected_proof_context(
    db: &Arc<Database>,
    target: Uuid,
    owner: Uuid,
    label: &str,
    request_label: &'static str,
) -> color_eyre::Result<()> {
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        3,
        "{label} owner should project call_site, call_edge, and call_resolution facts"
    );

    let tool_result =
        execute_fixture_tool_request(db, "pub fn local_target", 1, request_label).await?;
    let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
    assert_result_ok(&result, "pub fn local_target", 1, "fixture_call_graph");
    assert!(
        result
            .note
            .as_deref()
            .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
        "{label} projected proof facts should avoid degraded proof-context note: {result:#?}"
    );

    let owner_part = result
        .context
        .iter()
        .find(|part| part.id == owner)
        .unwrap_or_else(|| panic!("request_code_context should materialize the {label} owner"));
    assert!(
        owner_part.call_context.iter().any(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["local_target"]),
                    }
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        }),
        "{label} owner should also carry the body call context: {owner_part:#?}"
    );
    assert_projected_owner_rows(&owner_part.proof_context, owner, target);

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
        proof_context_count >= 3,
        "UI proof-context count should include the {label} owner proof rows: {result:#?}"
    );
    assert_eq!(
        ui_field(payload, "proof_context"),
        proof_context_count.to_string()
    );

    Ok(())
}
