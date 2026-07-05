use super::super::assertions::{assert_expansion, assert_resolved_target, path};
use super::super::*;

#[tokio::test]
async fn request_code_context_returns_targetless_special_form_call_context()
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
    let qself_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_qualified_dyn_any_downcast_mut"),
    )?;
    let chained_target = one_uuid(&db, &function_in_module_query(&["crate"], "make_unary_fn"))?;
    let returned_target = one_uuid(&db, &function_in_module_query(&["crate"], "unary_target"))?;

    let extern_tool_result =
        execute_fixture_tool_request(&db, "call_extern_c_function", 1, "extern_c_call_context")
            .await?;
    let extern_result: RequestCodeContextResult =
        serde_json::from_str(&extern_tool_result.content)?;
    assert_result_ok(
        &extern_result,
        "call_extern_c_function",
        1,
        "fixture_call_graph",
    );
    let extern_part = extern_result
        .context
        .iter()
        .find(|part| part.id == extern_owner)
        .expect("request_code_context should materialize the extern C owner");
    assert_eq!(
        extern_part.call_context.len(),
        1,
        "extern C call context: {extern_part:#?}"
    );
    let extern_call = &extern_part.call_context[0];
    assert_eq!(extern_call.kind, CallSiteKind::Path);
    assert_eq!(
        extern_call.callee,
        CallCalleeInfo::Path {
            path: path(&["abs"]),
        }
    );
    assert_eq!(extern_call.status, CallStatusKind::External);
    assert!(extern_call.resolution.is_none());
    assert!(
        extern_call.targets.is_empty(),
        "extern C calls must not fabricate TUI targets: {extern_call:#?}"
    );
    assert_call_blockers(&extern_tool_result, &extern_result);

    let chained_tool_result = execute_fixture_tool_request(
        &db,
        "call_chained_returned_function",
        1,
        "chained_returned_call_context",
    )
    .await?;
    let chained_result: RequestCodeContextResult =
        serde_json::from_str(&chained_tool_result.content)?;
    assert_result_ok(
        &chained_result,
        "call_chained_returned_function",
        1,
        "fixture_call_graph",
    );
    let owner_part = chained_result
        .context
        .iter()
        .find(|part| part.id == chained_owner)
        .expect("request_code_context should materialize the chained returned-function owner");
    let target_part = chained_result
        .context
        .iter()
        .find(|part| part.id == chained_target)
        .expect("request_code_context should materialize the chained returned-function target");
    assert_eq!(
        owner_part.call_context.len(),
        2,
        "chained returned-function call context: {owner_part:#?}"
    );
    let path_call = owner_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["make_unary_fn"]),
                    }
        })
        .expect("inner make_unary_fn path call should stay visible");
    assert_resolved_target(path_call, chained_target, CallTargetKind::Function);
    assert_expansion(
        target_part,
        chained_owner,
        chained_target,
        path_call.site_id,
        CallExpansionKind::OutgoingTarget,
    );

    let dynamic_call = owner_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == returned_target)
        })
        .expect("outer chained returned-function dynamic call should stay visible");
    assert_eq!(dynamic_call.callee, CallCalleeInfo::Dynamic);
    assert_resolved_target(
        dynamic_call,
        returned_target,
        CallTargetKind::DynamicFunction,
    );
    let returned_part = chained_result
        .context
        .iter()
        .find(|part| part.id == returned_target)
        .expect("request_code_context should materialize the returned function target");
    assert_expansion(
        returned_part,
        chained_owner,
        returned_target,
        dynamic_call.site_id,
        CallExpansionKind::OutgoingTarget,
    );

    let qself_tool_result = execute_fixture_tool_request(
        &db,
        "call_qualified_dyn_any_downcast_mut",
        1,
        "qualified_dyn_any_call_context",
    )
    .await?;
    let qself_result: RequestCodeContextResult = serde_json::from_str(&qself_tool_result.content)?;
    assert_result_ok(
        &qself_result,
        "call_qualified_dyn_any_downcast_mut",
        1,
        "fixture_call_graph",
    );
    let qself_part = qself_result
        .context
        .iter()
        .find(|part| part.id == qself_owner)
        .expect("request_code_context should materialize the qualified dyn Any owner");
    assert_eq!(
        qself_part.call_context.len(),
        1,
        "qualified dyn Any call context: {qself_part:#?}"
    );
    let qself_call = &qself_part.call_context[0];
    assert_eq!(qself_call.kind, CallSiteKind::Path);
    assert_eq!(
        qself_call.callee,
        CallCalleeInfo::Path {
            path: path(&["std", "any", "Any", "downcast_mut"]),
        }
    );
    assert_eq!(qself_call.status, CallStatusKind::External);
    assert!(qself_call.resolution.is_none());
    assert!(
        qself_call.targets.is_empty(),
        "qualified dyn Any calls must not fabricate TUI targets: {qself_call:#?}"
    );
    assert_call_blockers(&qself_tool_result, &qself_result);

    Ok(())
}

fn assert_call_blockers(tool_result: &crate::tools::ToolResult, result: &RequestCodeContextResult) {
    let payload = tool_result
        .ui_payload
        .as_ref()
        .expect("request_code_context should emit a UI payload");
    let expected = result
        .context
        .iter()
        .flat_map(|part| part.call_context.iter())
        .filter(|call| call.status != CallStatusKind::Resolved)
        .count();
    assert_eq!(ui_field(payload, "call_blockers"), expected.to_string());
    assert!(
        payload.summary.contains(&format!(
            "{expected} call {}",
            if expected == 1 { "blocker" } else { "blockers" }
        )),
        "request_code_context summary should surface nonzero call blocker counts for compact UI rendering: {payload:#?}"
    );
}
