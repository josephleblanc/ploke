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
async fn request_code_context_returns_async_closure_owner_call_context_for_local_target()
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

    let result =
        execute_fixture_request(&db, "pub fn local_target", 1, "async_closure_call_context")
            .await?;
    assert_result_ok(&result, "pub fn local_target", 1, "fixture_call_graph");

    assert!(
        result.context.iter().any(|part| part.id == target),
        "request_code_context should preserve the local_target seed part: {result:#?}"
    );
    assert!(
        result.context.iter().all(|part| part.id != outer),
        "async-closure local_target() must not expand to the enclosing outer function: {result:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:848-850:
    // The async closure body calls `local_target()`. Tool context should
    // expose the async-closure owner as the incoming caller, while the outer
    // dynamic closure invocation remains targetless.
    let async_part = result
        .context
        .iter()
        .find(|part| part.id == async_closure)
        .expect("request_code_context should materialize the async closure caller");
    let call = async_part
        .call_context
        .iter()
        .find(|call| {
            call.owner_id == async_closure
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
        .expect("async closure owner should retain outgoing local_target() call context");
    assert_resolved_target(call, target, CallTargetKind::Function);
    assert_incoming_expansion(async_part, call, target);

    Ok(())
}

#[tokio::test]
async fn request_code_context_returns_local_item_owner_call_context_for_assoc_const_value()
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
    let local_fn_outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "local_fn_body_call_is_not_outer_call_site"),
    )?;
    let local_fn = local_item_owner_for_parent_with_label(&db, local_fn_outer, "local_fn:inner")?;
    let local_impl_outer = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "local_impl_method_body_call_is_not_outer_call_site",
        ),
    )?;
    let local_impl =
        local_item_owner_for_parent_with_label(&db, local_impl_outer, "local_impl_method:value")?;

    let result = execute_fixture_request(
        &db,
        "pub const fn assoc_const_value",
        1,
        "local_item_owner_call_context",
    )
    .await?;
    assert_result_ok(
        &result,
        "pub const fn assoc_const_value",
        1,
        "fixture_call_graph",
    );

    assert!(
        result.context.iter().any(|part| part.id == target),
        "request_code_context should preserve the assoc_const_value seed part: {result:#?}"
    );
    assert!(
        result
            .context
            .iter()
            .all(|part| part.id != outer && part.id != local_impl_outer),
        "local item body calls must not expand to enclosing functions that do not call the local item: {result:#?}"
    );

    // The local function item is now a callable target. The enclosing function
    // legitimately appears because it calls `inner()`, which targets
    // `local_fn:inner`.
    let local_fn_outer_part = result
        .context
        .iter()
        .find(|part| part.id == local_fn_outer)
        .expect("request_code_context should materialize the outer caller of local_fn:inner");
    let inner_call = local_fn_outer_part
        .call_context
        .iter()
        .find(|call| {
            call.owner_id == local_fn_outer
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["inner"]),
                    }
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == local_fn)
        })
        .expect("outer local-fn owner should retain outgoing inner() call context");
    assert_resolved_target(inner_call, local_fn, CallTargetKind::LocalFunction);
    let expansion = local_fn_outer_part
        .call_expansion
        .expect("local function outer caller should carry call-expansion provenance");
    assert_eq!(expansion.seed_id, target);
    assert_eq!(expansion.relation, CallExpansionKind::IncomingCaller);
    assert_eq!(expansion.call_site_id, inner_call.site_id);
    assert_eq!(expansion.target_id, target);
    assert_eq!(expansion.distance, 2);

    for (label, source_line, local_item) in [
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1372
        ("local_const", 1372, local_item),
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1441
        ("local_fn:inner", 1441, local_fn),
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1461
        ("local_impl_method:value", 1461, local_impl),
    ] {
        // The local item body calls `assoc_const_value()`. Tool context should
        // expose the local-item owner as the incoming caller.
        let local_item_part = result
            .context
            .iter()
            .find(|part| part.id == local_item)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {label} caller from tests/fixture_crates/fixture_call_graph/src/lib.rs:{source_line}"
                )
            });
        let call = local_item_part
            .call_context
            .iter()
            .find(|call| {
                call.owner_id == local_item
                    && call.kind == CallSiteKind::Path
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: path(&["assoc_const_value"]),
                        }
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{label} owner from tests/fixture_crates/fixture_call_graph/src/lib.rs:{source_line} should retain outgoing assoc_const_value() call context"
                )
            });
        assert_resolved_target(call, target, CallTargetKind::Function);
        assert_incoming_expansion(local_item_part, call, target);
    }

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

#[tokio::test]
async fn request_code_context_returns_awaited_async_closure_literal_owner_call_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_awaited_async_closure_literal_with_body_call",
        ),
    )?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let result = execute_fixture_request(
        &db,
        "call_awaited_async_closure_literal_with_body_call",
        1,
        "awaited_async_closure_literal_call_context",
    )
    .await?;
    assert_result_ok(
        &result,
        "call_awaited_async_closure_literal_with_body_call",
        1,
        "fixture_call_graph",
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1538-1539:
    // `(async || local_target())().await` immediately polls the async closure
    // future. Tool context should expose the outer dynamic call to the
    // async-closure owner, then the closure-owned body call to `local_target()`.
    let outer_part =
        result.context.iter().find(|part| part.id == outer).expect(
            "request_code_context should preserve the awaited async-closure outer function",
        );
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
                    .any(|target_info| target_info.target_id == async_closure)
        })
        .expect("outer function should retain the awaited async-closure dynamic call");
    assert_resolved_target(dynamic_call, async_closure, CallTargetKind::DynamicClosure);

    let async_part = result
        .context
        .iter()
        .find(|part| part.id == async_closure)
        .expect("request_code_context should materialize the awaited async-closure owner");
    assert_expansion(
        async_part,
        outer,
        async_closure,
        dynamic_call.site_id,
        CallExpansionKind::OutgoingTarget,
    );
    let body_call = async_part
        .call_context
        .iter()
        .find(|call| {
            call.owner_id == async_closure
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
        .expect("awaited async-closure owner should retain outgoing local_target() call context");
    assert_resolved_target(body_call, target, CallTargetKind::Function);

    Ok(())
}
