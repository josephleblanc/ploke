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
