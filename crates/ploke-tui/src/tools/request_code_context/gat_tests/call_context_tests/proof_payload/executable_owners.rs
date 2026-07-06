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
        "pub fn local_target",
        &["local_target"],
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
        "pub fn local_target",
        &["local_target"],
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
        "pub fn local_target",
        &["local_target"],
        "async_closure_owner_projected_proof_context",
    )
    .await
}

#[tokio::test]
async fn request_code_context_returns_local_item_owner_projected_proof_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "assoc_const_value"),
    )?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "local_const_initializer_call_is_not_outer_call_site",
        ),
    )?;
    let local_item = local_item_owner_for_parent_with_label(&db, outer, "local_const")?;
    assert_owner_projected_proof_context(
        &db,
        target,
        local_item,
        "local const item",
        "pub const fn assoc_const_value",
        &["assoc_const_value"],
        "local_const_owner_projected_proof_context",
    )
    .await?;

    let local_fn_outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "local_fn_body_call_is_not_outer_call_site"),
    )?;
    let local_fn = local_item_owner_for_parent_with_label(&db, local_fn_outer, "local_fn:inner")?;
    assert_owner_projected_proof_context(
        &db,
        target,
        local_fn,
        "local fn item",
        "pub const fn assoc_const_value",
        &["assoc_const_value"],
        "local_fn_owner_projected_proof_context",
    )
    .await?;

    let local_impl_outer = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "local_impl_method_body_call_is_not_outer_call_site",
        ),
    )?;
    let local_impl =
        local_item_owner_for_parent_with_label(&db, local_impl_outer, "local_impl_method:value")?;
    assert_owner_projected_proof_context(
        &db,
        target,
        local_impl,
        "local impl method item",
        "pub const fn assoc_const_value",
        &["assoc_const_value"],
        "local_impl_method_owner_projected_proof_context",
    )
    .await
}

async fn assert_owner_projected_proof_context(
    db: &Arc<Database>,
    target: Uuid,
    owner: Uuid,
    label: &str,
    search_term: &str,
    callee_path: &[&str],
    request_label: &'static str,
) -> color_eyre::Result<()> {
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        3,
        "{label} owner should project call_site, call_edge, and call_resolution facts"
    );

    let tool_result = execute_fixture_tool_request(db, search_term, 1, request_label).await?;
    let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
    assert_result_ok(&result, search_term, 1, "fixture_call_graph");
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
                        path: path(callee_path),
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
