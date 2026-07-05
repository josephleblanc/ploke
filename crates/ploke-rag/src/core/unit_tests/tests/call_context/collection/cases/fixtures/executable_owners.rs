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
async fn call_context_collection_reads_local_item_owner_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "assoc_const_value")?;
    let cases = [
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1372
        (
            "local_const",
            1372,
            one_uuid(
                &db,
                &function_in_module_query(
                    &["crate"],
                    "local_const_initializer_call_is_not_outer_call_site",
                ),
            )?,
        ),
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1441
        (
            "local_fn",
            1441,
            one_uuid(
                &db,
                &function_in_module_query(&["crate"], "local_fn_body_call_is_not_outer_call_site"),
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
