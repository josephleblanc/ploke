use super::super::super::super::super::*;

#[tokio::test]
async fn call_context_collection_reads_closure_executable_owner_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "closure_body_call_is_not_outer_call_site"),
    )?;
    let closure = closure_owner_for_parent(&db, outer)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable closure executable owner context"
    );

    let call_context = rag.collect_call_context(&[(outer, 1.0), (closure, 1.0)])?;

    let outer_context = call_context
        .get(&outer)
        .expect("outer function should keep its own call context");
    assert!(
        outer_context.iter().all(|call| {
            call.callee
                != CallCalleeInfo::Path {
                    path: vec!["local_target".to_string()],
                }
        }),
        "outer function must not absorb the closure-body local_target() row: {outer_context:#?}"
    );
    assert!(
        outer_context
            .iter()
            .flat_map(|call| call.targets.iter())
            .all(|target_info| target_info.target_id != target),
        "outer function must not expose a fabricated edge to local_target: {outer_context:#?}"
    );
    let closure_call = outer_context
        .iter()
        .find(|call| {
            call.callee
                == CallCalleeInfo::Path {
                    path: vec!["closure".to_string()],
                }
        })
        .expect("outer function should expose the closure() binding call");
    assert_eq!(closure_call.owner_id, outer);
    assert_eq!(closure_call.status, CallStatusKind::Resolved);
    assert_eq!(closure_call.targets.len(), 1);
    assert_eq!(closure_call.targets[0].target_id, closure);
    assert_eq!(closure_call.targets[0].relation, CallTargetKind::Closure);

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:155-157:
    // `closure_body_call_is_not_outer_call_site` binds `|| local_target()` and
    // invokes the closure. RAG should read the persisted closure owner as the
    // caller for the body path call.
    let closure_context = call_context
        .get(&closure)
        .expect("closure executable owner should receive outgoing call context");
    let call = closure_context
        .iter()
        .find(|call| {
            call.owner_id == closure
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["local_target".to_string()],
                    }
        })
        .expect("closure owner should retain outgoing local_target() call context");
    assert_eq!(call.owner_id, closure);
    assert_eq!(call.kind, CallSiteKind::Path);
    assert_eq!(
        call.callee,
        CallCalleeInfo::Path {
            path: vec!["local_target".to_string()],
        }
    );
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_async_block_owner_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "async_block_call_is_not_outer_call_site"),
    )?;
    let async_body = async_block_owner_for_parent(&db, outer)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable async block owner context"
    );

    let call_context = rag.collect_call_context(&[(outer, 1.0), (async_body, 1.0)])?;

    if let Some(outer_context) = call_context.get(&outer) {
        assert!(
            outer_context.iter().all(|call| {
                call.callee
                    != CallCalleeInfo::Path {
                        path: vec!["local_target".to_string()],
                    }
            }),
            "outer function must not absorb the async-block local_target() row: {outer_context:#?}"
        );
        assert!(
            outer_context
                .iter()
                .flat_map(|call| call.targets.iter())
                .all(|target_info| target_info.target_id != target),
            "outer function must not expose a fabricated edge to local_target: {outer_context:#?}"
        );
    }

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:160-164:
    // `async_block_call_is_not_outer_call_site` creates an async block whose
    // body calls `local_target()`. RAG should read the persisted async block
    // owner as the caller for the body path call.
    let async_context = call_context
        .get(&async_body)
        .expect("async block owner should receive outgoing call context");
    let call = async_context
        .iter()
        .find(|call| {
            call.owner_id == async_body
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["local_target".to_string()],
                    }
        })
        .expect("async block owner should retain outgoing local_target() call context");
    assert_eq!(call.owner_id, async_body);
    assert_eq!(call.kind, CallSiteKind::Path);
    assert_eq!(
        call.callee,
        CallCalleeInfo::Path {
            path: vec!["local_target".to_string()],
        }
    );
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_async_closure_owner_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_async_closure_literal_with_body_call"),
    )?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable async closure owner context"
    );

    let call_context = rag.collect_call_context(&[(outer, 1.0), (async_closure, 1.0)])?;

    let outer_context = call_context
        .get(&outer)
        .expect("outer function should keep its unsupported dynamic call context");
    assert!(
        outer_context.iter().all(|call| {
            call.callee
                != CallCalleeInfo::Path {
                    path: vec!["local_target".to_string()],
                }
        }),
        "outer function must not absorb the async-closure local_target() row: {outer_context:#?}"
    );
    assert!(
        outer_context
            .iter()
            .flat_map(|call| call.targets.iter())
            .all(|target_info| target_info.target_id != target),
        "outer function must not expose a fabricated edge to local_target: {outer_context:#?}"
    );
    let dynamic_call = outer_context
        .iter()
        .find(|call| call.kind == CallSiteKind::Dynamic && call.callee == CallCalleeInfo::Dynamic)
        .expect("outer function should expose the async closure invocation as a dynamic row");
    assert_eq!(dynamic_call.status, CallStatusKind::Unsupported);
    assert!(
        dynamic_call.targets.is_empty(),
        "async closure invocation should stay targetless until the returned future is polled: {dynamic_call:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:848-850:
    // `(async || local_target())()` creates an async closure future. RAG
    // should read the async-closure owner as the caller for the body path call
    // without treating the outer invocation as a direct call to the body.
    let async_context = call_context
        .get(&async_closure)
        .expect("async closure owner should receive outgoing call context");
    let call = async_context
        .iter()
        .find(|call| {
            call.owner_id == async_closure
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["local_target".to_string()],
                    }
        })
        .expect("async closure owner should retain outgoing local_target() call context");
    assert_eq!(call.owner_id, async_closure);
    assert_eq!(call.kind, CallSiteKind::Path);
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_keeps_non_awaited_async_closure_binding_targetless()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_async_closure_binding_without_await_with_body_call",
        ),
    )?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable async closure binding context"
    );

    let call_context = rag.collect_call_context(&[(outer, 1.0), (async_closure, 1.0)])?;

    let outer_context = call_context
        .get(&outer)
        .expect("outer function should keep its unsupported async-closure binding context");
    assert!(
        outer_context.iter().all(|call| {
            call.callee
                != CallCalleeInfo::Path {
                    path: vec!["local_target".to_string()],
                }
        }),
        "outer function must not absorb the async-closure binding local_target() row: {outer_context:#?}"
    );
    assert!(
        outer_context
            .iter()
            .flat_map(|call| call.targets.iter())
            .all(|target_info| target_info.target_id != target),
        "outer function must not expose a fabricated edge to local_target: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1701-1704:
    // `closure()` invokes a local async-closure binding without awaiting the
    // returned future. RAG should surface the path call as unsupported while
    // still reading the closure owner's body call context.
    let closure_call = outer_context
        .iter()
        .find(|call| {
            call.owner_id == outer
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["closure".to_string()],
                    }
        })
        .expect("outer function should expose closure() binding call context");
    assert_eq!(closure_call.status, CallStatusKind::Unsupported);
    assert!(
        closure_call.targets.is_empty(),
        "non-awaited async closure binding should stay targetless: {closure_call:#?}"
    );

    let async_context = call_context
        .get(&async_closure)
        .expect("async closure owner should receive outgoing call context");
    let call = async_context
        .iter()
        .find(|call| {
            call.owner_id == async_closure
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["local_target".to_string()],
                    }
        })
        .expect("async closure owner should retain outgoing local_target() call context");
    assert_eq!(call.kind, CallSiteKind::Path);
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_awaited_async_closure_binding_rows() -> Result<(), Error>
{
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_awaited_async_closure_binding_with_body_call",
        ),
    )?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable awaited async closure binding context"
    );

    let call_context = rag.collect_call_context(&[(outer, 1.0), (async_closure, 1.0)])?;

    let outer_context = call_context
        .get(&outer)
        .expect("outer function should keep its awaited async-closure binding context");
    assert!(
        outer_context.iter().all(|call| {
            call.callee
                != CallCalleeInfo::Path {
                    path: vec!["local_target".to_string()],
                }
        }),
        "outer function must not absorb the awaited async-closure binding local_target() row: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1706-1709:
    // `closure().await` immediately polls the async-closure binding, so RAG
    // should expose the path-call target edge to the closure owner.
    let closure_call = outer_context
        .iter()
        .find(|call| {
            call.owner_id == outer
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["closure".to_string()],
                    }
        })
        .expect("outer function should expose closure().await binding call context");
    assert_eq!(closure_call.status, CallStatusKind::Resolved);
    assert_eq!(
        closure_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(closure_call.targets.len(), 1);
    assert_eq!(closure_call.targets[0].target_id, async_closure);
    assert_eq!(closure_call.targets[0].relation, CallTargetKind::Closure);

    let async_context = call_context
        .get(&async_closure)
        .expect("async closure owner should receive outgoing call context");
    let call = async_context
        .iter()
        .find(|call| {
            call.owner_id == async_closure
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["local_target".to_string()],
                    }
        })
        .expect("async closure owner should retain outgoing local_target() call context");
    assert_eq!(call.kind, CallSiteKind::Path);
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_keeps_non_awaited_async_closure_future_binding_targetless()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_async_closure_future_binding_without_await_with_body_call",
        ),
    )?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let call_context = rag.collect_call_context(&[(outer, 1.0), (async_closure, 1.0)])?;

    let outer_context = call_context
        .get(&outer)
        .expect("outer function should keep its unawaited future binding context");
    assert!(
        outer_context.iter().all(|call| {
            call.callee
                != CallCalleeInfo::Path {
                    path: vec!["local_target".to_string()],
                }
        }),
        "outer function must not absorb the async-closure future body call: {outer_context:#?}"
    );
    assert!(
        outer_context
            .iter()
            .flat_map(|call| call.targets.iter())
            .all(|target_info| target_info.target_id != target),
        "outer function must not expose a fabricated edge to local_target: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1711-1714:
    // `_future = closure()` stores the async-closure future without awaiting it.
    let closure_call = outer_context
        .iter()
        .find(|call| {
            call.owner_id == outer
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["closure".to_string()],
                    }
        })
        .expect("outer function should expose the unawaited closure() future binding call");
    assert_eq!(closure_call.status, CallStatusKind::Unsupported);
    assert!(
        closure_call.targets.is_empty(),
        "unawaited async closure future binding should stay targetless: {closure_call:#?}"
    );

    let async_context = call_context
        .get(&async_closure)
        .expect("async closure owner should receive outgoing call context");
    let call = async_context
        .iter()
        .find(|call| {
            call.owner_id == async_closure
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["local_target".to_string()],
                    }
        })
        .expect("async closure owner should retain outgoing local_target() call context");
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_awaited_async_closure_future_binding_rows()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_awaited_async_closure_future_binding_with_body_call",
        ),
    )?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let call_context = rag.collect_call_context(&[(outer, 1.0), (async_closure, 1.0)])?;

    let outer_context = call_context
        .get(&outer)
        .expect("outer function should keep its awaited future binding context");
    assert!(
        outer_context.iter().all(|call| {
            call.callee
                != CallCalleeInfo::Path {
                    path: vec!["local_target".to_string()],
                }
        }),
        "outer function must not absorb the awaited async-closure future body call: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1716-1720:
    // `future = closure(); future.await;` proves the earlier closure() call is
    // polled in the same block, so RAG should expose the closure owner target.
    let closure_call = outer_context
        .iter()
        .find(|call| {
            call.owner_id == outer
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["closure".to_string()],
                    }
        })
        .expect("outer function should expose the awaited closure() future binding call");
    assert_eq!(closure_call.status, CallStatusKind::Resolved);
    assert_eq!(
        closure_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(closure_call.targets.len(), 1);
    assert_eq!(closure_call.targets[0].target_id, async_closure);
    assert_eq!(closure_call.targets[0].relation, CallTargetKind::Closure);

    let async_context = call_context
        .get(&async_closure)
        .expect("async closure owner should receive outgoing call context");
    let call = async_context
        .iter()
        .find(|call| {
            call.owner_id == async_closure
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["local_target".to_string()],
                    }
        })
        .expect("async closure owner should retain outgoing local_target() call context");
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_awaited_async_closure_future_alias_rows()
-> Result<(), Error> {
    assert_awaited_async_closure_future_context(
        "call_awaited_async_closure_future_alias_with_body_call",
        "awaited async-closure future alias",
    )
    .await
}

#[tokio::test]
async fn call_context_collection_resolves_awaited_async_closure_future_block_alias_rows()
-> Result<(), Error> {
    assert_awaited_async_closure_future_context(
        "call_awaited_async_closure_future_block_alias_with_body_call",
        "awaited async-closure future block alias",
    )
    .await
}

#[tokio::test]
async fn call_context_collection_resolves_awaited_async_closure_future_alias_chain_rows()
-> Result<(), Error> {
    assert_awaited_async_closure_future_context(
        "call_awaited_async_closure_future_alias_chain_with_body_call",
        "awaited async-closure future alias chain",
    )
    .await
}

#[tokio::test]
async fn call_context_collection_resolves_awaited_async_closure_future_tuple_field_rows()
-> Result<(), Error> {
    assert_awaited_async_closure_future_context(
        "call_awaited_async_closure_future_tuple_field_with_body_call",
        "awaited async-closure future tuple field",
    )
    .await
}

async fn assert_awaited_async_closure_future_context(
    owner_name: &str,
    label: &str,
) -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let outer = one_uuid(&db, &function_in_module_query(&["crate"], owner_name))?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    let call_context = rag.collect_call_context(&[(outer, 1.0), (async_closure, 1.0)])?;

    let outer_context = call_context
        .get(&outer)
        .unwrap_or_else(|| panic!("outer function should keep its {label} context"));
    assert!(
        outer_context.iter().all(|call| {
            call.callee
                != CallCalleeInfo::Path {
                    path: vec!["local_target".to_string()],
                }
        }),
        "outer function must not absorb the {label} body call: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1722-1727:
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1821-1825:
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1913-1919:
    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // `future = closure(); alias = future; alias.await;` and
    // `future = closure(); alias = { future }; alias.await;` and the two-step
    // `future = closure(); alias = future; second = alias; second.await;` and
    // `futures = (closure(),); futures.0.await;` prove the original closure()
    // call is polled through bounded same-block evidence.
    let closure_call = outer_context
        .iter()
        .find(|call| {
            call.owner_id == outer
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["closure".to_string()],
                    }
        })
        .unwrap_or_else(|| panic!("outer function should expose the {label} closure() call"));
    assert_eq!(closure_call.status, CallStatusKind::Resolved);
    assert_eq!(
        closure_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(closure_call.targets.len(), 1);
    assert_eq!(closure_call.targets[0].target_id, async_closure);
    assert_eq!(closure_call.targets[0].relation, CallTargetKind::Closure);

    let async_context = call_context
        .get(&async_closure)
        .expect("async closure owner should receive outgoing call context");
    let call = async_context
        .iter()
        .find(|call| {
            call.owner_id == async_closure
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["local_target".to_string()],
                    }
        })
        .expect("async closure owner should retain outgoing local_target() call context");
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_local_fn_item_target_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let outer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "local_fn_body_call_is_not_outer_call_site"),
    )?;
    let local_fn = local_item_owner_for_parent_with_label(&db, outer, "local_fn:inner")?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable local function item context"
    );

    let call_context = rag.collect_call_context(&[(outer, 1.0), (local_fn, 1.0)])?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // the outer function calls block-local `inner()`, and `inner` is stored as
    // an executable local-item body. RAG should expose that target edge.
    let outer_context = call_context
        .get(&outer)
        .expect("outer function should keep outgoing inner() context");
    let call = outer_context
        .iter()
        .find(|call| {
            call.owner_id == outer
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["inner".to_string()],
                    }
        })
        .expect("outer function should expose inner() call context");
    assert_eq!(call.owner_id, outer);
    assert_eq!(call.kind, CallSiteKind::Path);
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, local_fn);
    assert_eq!(call.targets[0].relation, CallTargetKind::LocalFunction);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_local_item_owner_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "assoc_const_value")?;
    let cases = [
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1379
        (
            "local_const",
            1379,
            one_uuid(
                &db,
                &function_in_module_query(
                    &["crate"],
                    "local_const_initializer_call_is_not_outer_call_site",
                ),
            )?,
        ),
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1447
        (
            "local_fn:inner",
            1447,
            one_uuid(
                &db,
                &function_in_module_query(&["crate"], "local_fn_body_call_is_not_outer_call_site"),
            )?,
        ),
        (
            "local_impl_method:value",
            1464,
            one_uuid(
                &db,
                &function_in_module_query(
                    &["crate"],
                    "local_impl_method_body_call_is_not_outer_call_site",
                ),
            )?,
        ),
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:2143
        (
            "local_const",
            2143,
            one_uuid(
                &db,
                &function_in_module_query(
                    &["crate"],
                    "call_const_item_macro_generated_const_initializer",
                ),
            )?,
        ),
    ];
    let mut seeds = Vec::new();
    for (label, _, outer) in cases {
        seeds.push((outer, 1.0));
        seeds.push((
            local_item_owner_for_parent_with_label(&db, outer, label)?,
            1.0,
        ));
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable local item owner context"
    );

    let call_context = rag.collect_call_context(&seeds)?;

    for (label, source_line, outer) in cases {
        let local_item = local_item_owner_for_parent_with_label(&db, outer, label)?;
        if let Some(outer_context) = call_context.get(&outer) {
            assert!(
                outer_context.iter().all(|call| {
                    call.callee
                        != CallCalleeInfo::Path {
                            path: vec!["assoc_const_value".to_string()],
                        }
                }),
                "outer function must not absorb the {label} body call: {outer_context:#?}"
            );
            assert!(
                outer_context
                    .iter()
                    .flat_map(|call| call.targets.iter())
                    .all(|target_info| target_info.target_id != target),
                "outer function must not expose a fabricated edge to assoc_const_value from {label}: {outer_context:#?}"
            );
        }

        // The function-local item body calls `assoc_const_value()`. RAG should
        // read the persisted local-item owner as the caller for that body path
        // call, not the enclosing function.
        let local_item_context = call_context
            .get(&local_item)
            .unwrap_or_else(|| {
                panic!(
                    "{label} owner from tests/fixture_crates/fixture_call_graph/src/lib.rs:{source_line} should receive outgoing call context"
                )
            });
        let call = local_item_context
            .iter()
            .find(|call| {
                call.owner_id == local_item
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: vec!["assoc_const_value".to_string()],
                        }
            })
            .unwrap_or_else(|| {
                panic!(
                    "{label} owner from tests/fixture_crates/fixture_call_graph/src/lib.rs:{source_line} should retain outgoing assoc_const_value() call context"
                )
            });
        assert_eq!(call.owner_id, local_item);
        assert_eq!(call.kind, CallSiteKind::Path);
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Function);
    }

    Ok(())
}
