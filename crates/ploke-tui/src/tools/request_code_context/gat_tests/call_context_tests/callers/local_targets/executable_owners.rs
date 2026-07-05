use super::*;

#[tokio::test]
async fn request_code_context_returns_closure_owner_call_context_for_local_target()
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

    let result =
        execute_fixture_request(&db, "pub fn local_target", 1, "closure_owner_call_context")
            .await?;
    assert_result_ok(&result, "pub fn local_target", 1, "fixture_call_graph");

    assert!(
        result.context.iter().any(|part| part.id == target),
        "request_code_context should preserve the local_target seed part: {result:#?}"
    );
    assert!(
        result.context.iter().all(|part| part.id != outer),
        "closure-body local_target() must not expand to the enclosing outer function: {result:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:155-157:
    // `closure_body_call_is_not_outer_call_site` binds `|| local_target()`.
    // Tool context should expose the closure body owner as the incoming caller.
    let closure_part = result
        .context
        .iter()
        .find(|part| part.id == closure)
        .expect("request_code_context should materialize the closure executable caller");
    let call = closure_part
        .call_context
        .iter()
        .find(|call| {
            call.owner_id == closure
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["local_target"]),
                    }
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("closure owner should retain outgoing local_target() call context");
    assert_resolved_target(call, target, CallTargetKind::Function);
    assert_incoming_expansion(closure_part, call, target);

    Ok(())
}

#[tokio::test]
async fn request_code_context_returns_async_block_owner_call_context_for_local_target()
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

    let result =
        execute_fixture_request(&db, "pub fn local_target", 1, "async_block_call_context").await?;
    assert_result_ok(&result, "pub fn local_target", 1, "fixture_call_graph");

    assert!(
        result.context.iter().any(|part| part.id == target),
        "request_code_context should preserve the local_target seed part: {result:#?}"
    );
    assert!(
        result.context.iter().all(|part| part.id != outer),
        "async-block local_target() must not expand to the enclosing outer function: {result:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:160-164:
    // The async block body calls `local_target()`. Tool context should expose
    // the async block owner as the incoming caller.
    let async_part = result
        .context
        .iter()
        .find(|part| part.id == async_body)
        .expect("request_code_context should materialize the async block caller");
    let call = async_part
        .call_context
        .iter()
        .find(|call| {
            call.owner_id == async_body
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["local_target"]),
                    }
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("async block owner should retain outgoing local_target() call context");
    assert_resolved_target(call, target, CallTargetKind::Function);
    assert_incoming_expansion(async_part, call, target);

    Ok(())
}

#[tokio::test]
async fn request_code_context_returns_move_closure_literal_owner_call_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_move_closure_literal_with_body_call"),
    )?;
    let closure = closure_owner_for_parent(&db, outer)?;

    let result = execute_fixture_request(
        &db,
        "call_move_closure_literal_with_body_call",
        1,
        "move_closure_literal_call_context",
    )
    .await?;
    assert_result_ok(
        &result,
        "call_move_closure_literal_with_body_call",
        1,
        "fixture_call_graph",
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:710:
    // `(move || local_target())()` should expose the outer dynamic call to the
    // closure owner, then the closure-owned body call to `local_target()`.
    let outer_part = result
        .context
        .iter()
        .find(|part| part.id == outer)
        .expect("request_code_context should preserve the move-closure outer function");
    let dynamic_call = outer_part
        .call_context
        .iter()
        .find(|call| {
            call.owner_id == outer
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == closure)
        })
        .expect("outer function should retain the move-closure literal dynamic call");
    assert_resolved_target(dynamic_call, closure, CallTargetKind::DynamicClosure);

    let closure_part = result
        .context
        .iter()
        .find(|part| part.id == closure)
        .expect("request_code_context should materialize the move closure owner");
    assert_expansion(
        closure_part,
        outer,
        closure,
        dynamic_call.site_id,
        CallExpansionKind::OutgoingTarget,
    );
    let body_call = closure_part
        .call_context
        .iter()
        .find(|call| {
            call.owner_id == closure
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["local_target"]),
                    }
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("move closure owner should retain outgoing local_target() call context");
    assert_resolved_target(body_call, target, CallTargetKind::Function);

    Ok(())
}
