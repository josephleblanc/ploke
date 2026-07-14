use super::super::super::super::super::*;
use super::super::super::helpers::*;
use super::expected::assert_ambiguous_call_candidates;
#[tokio::test]
async fn call_context_collection_reads_real_fixture_callable_path_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let make_fn = unique_id_by_name(&db, "function", "make_fn")?;
    let make_closure = unique_id_by_name(&db, "function", "make_closure")?;
    let make_bound = unique_id_by_name(&db, "function", "make_bound_closure")?;
    let make_alias = unique_id_by_name(&db, "function", "make_alias_bound_closure")?;
    let make_returned_async = unique_id_by_name(&db, "function", "make_returned_async_closure")?;
    let returned_param_helper =
        unique_id_by_name(&db, "function", "return_forwarded_function_pointer")?;
    let returned_conflicting_helper = unique_id_by_name(
        &db,
        "function",
        "return_conflicting_forwarded_function_pointer",
    )?;
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let other_target = unique_id_by_name(&db, "function", "other_target")?;
    let returned_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_returned_function"),
    )?;
    let returned_param_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_returned_forwarded_function_pointer_param_with_local_target",
        ),
    )?;
    let returned_conflicting_local_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_returned_conflicting_forwarded_function_pointer_param_with_local_target",
        ),
    )?;
    let returned_conflicting_other_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_returned_conflicting_forwarded_function_pointer_param_with_other_target",
        ),
    )?;
    let returned_closure_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_returned_closure"),
    )?;
    let bound_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_returned_bound_closure"),
    )?;
    let alias_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_returned_alias_bound_closure"),
    )?;
    let returned_async_no_await_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_returned_async_closure_without_await"),
    )?;
    let returned_async_awaited_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_awaited_returned_async_closure"),
    )?;
    let returned_async_stored_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_stored_returned_async_closure"),
    )?;
    let branch_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_if_initialized_function_item_binding"),
    )?;
    let block_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_block_initialized_function_item_binding"),
    )?;
    let fn_param_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_function_pointer_param"),
    )?;
    let single_param_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_function_pointer_param"),
    )?;
    let multi_param_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_multi_function_pointer_param"),
    )?;
    let forwarded_param_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_forwarded_function_pointer_leaf"),
    )?;
    let two_hop_forwarded_param_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_two_hop_forwarded_function_pointer_leaf"),
    )?;
    let forwarded_referenced_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_forwarded_referenced_dyn_fn_leaf"),
    )?;
    let two_hop_forwarded_referenced_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_two_hop_forwarded_referenced_dyn_fn_leaf"),
    )?;
    let multi_conflicting_param_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_multi_conflicting_function_pointer_param"),
    )?;
    let forwarded_conflicting_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_forwarded_conflicting_function_pointer_leaf",
        ),
    )?;
    let two_hop_forwarded_conflicting_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_two_hop_forwarded_conflicting_function_pointer_leaf",
        ),
    )?;
    let multi_conflicting_generic_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_multi_conflicting_generic_fn_once_param"),
    )?;
    let multi_conflicting_field_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_multi_conflicting_named_field_function_param",
        ),
    )?;
    let forwarded_conflicting_field_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_forwarded_conflicting_named_field_leaf"),
    )?;
    let single_parenthesized_param_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_single_parenthesized_function_pointer_param",
        ),
    )?;
    let generic_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_generic_fn_once_value_binding"),
    )?;
    let boxed_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_boxed_dyn_fn_value_binding"),
    )?;
    let vec_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_prelude_vec_new"),
    )?;
    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 64;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable callable path call context"
    );

    let call_context = rag.collect_call_context(&[
        (returned_owner, 1.0),
        (returned_param_owner, 1.0),
        (returned_conflicting_local_owner, 1.0),
        (returned_conflicting_other_owner, 1.0),
        (returned_closure_owner, 1.0),
        (bound_owner, 1.0),
        (alias_owner, 1.0),
        (returned_async_no_await_owner, 1.0),
        (returned_async_awaited_owner, 1.0),
        (returned_async_stored_owner, 1.0),
        (branch_owner, 1.0),
        (block_owner, 1.0),
        (fn_param_owner, 1.0),
        (single_param_owner, 1.0),
        (multi_param_owner, 1.0),
        (forwarded_param_owner, 1.0),
        (two_hop_forwarded_param_owner, 1.0),
        (forwarded_referenced_owner, 1.0),
        (two_hop_forwarded_referenced_owner, 1.0),
        (multi_conflicting_param_owner, 1.0),
        (forwarded_conflicting_owner, 1.0),
        (two_hop_forwarded_conflicting_owner, 1.0),
        (multi_conflicting_generic_owner, 1.0),
        (multi_conflicting_field_owner, 1.0),
        (forwarded_conflicting_field_owner, 1.0),
        (single_parenthesized_param_owner, 1.0),
        (generic_owner, 1.0),
        (boxed_owner, 1.0),
        (vec_owner, 1.0),
    ])?;

    let returned_context = call_context
        .get(&returned_owner)
        .expect("returned-function owner should receive outgoing call context");
    assert_eq!(
        returned_context.len(),
        2,
        "returned-function owner context: {returned_context:#?}"
    );
    let returned_path = returned_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["make_fn".to_string()],
                    }
        })
        .expect("inner make_fn path call should stay visible");
    assert_eq!(returned_path.status, CallStatusKind::Resolved);
    assert_eq!(
        returned_path.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(returned_path.targets.len(), 1);
    assert_eq!(returned_path.targets[0].target_id, make_fn);
    assert_eq!(returned_path.targets[0].relation, CallTargetKind::Function);

    let returned_dynamic = returned_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == local_target)
        })
        .expect("outer returned-function dynamic call should stay visible");
    assert_eq!(returned_dynamic.callee, CallCalleeInfo::Dynamic);
    assert_eq!(returned_dynamic.status, CallStatusKind::Resolved);
    assert_eq!(
        returned_dynamic.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(returned_dynamic.targets.len(), 1);
    assert_eq!(returned_dynamic.targets[0].target_id, local_target);
    assert_eq!(
        returned_dynamic.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    let returned_param_context = call_context
        .get(&returned_param_owner)
        .expect("returned parameter function owner should receive outgoing call context");
    assert_eq!(
        returned_param_context.len(),
        2,
        "returned parameter function context: {returned_param_context:#?}"
    );
    let returned_param_path = returned_param_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["return_forwarded_function_pointer".to_string()],
                    }
        })
        .expect("inner returned-parameter helper path call should stay visible");
    assert_eq!(returned_param_path.status, CallStatusKind::Resolved);
    assert_eq!(
        returned_param_path.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(returned_param_path.targets.len(), 1);
    assert_eq!(
        returned_param_path.targets[0].target_id,
        returned_param_helper
    );
    assert_eq!(
        returned_param_path.targets[0].relation,
        CallTargetKind::Function
    );

    let returned_param_dynamic = returned_param_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == local_target)
        })
        .expect("outer returned-parameter dynamic call should stay visible");
    assert_eq!(returned_param_dynamic.callee, CallCalleeInfo::Dynamic);
    assert_eq!(returned_param_dynamic.status, CallStatusKind::Resolved);
    assert_eq!(
        returned_param_dynamic.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(returned_param_dynamic.targets.len(), 1);
    assert_eq!(returned_param_dynamic.targets[0].target_id, local_target);
    assert_eq!(
        returned_param_dynamic.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    for (owner, label) in [
        (
            returned_conflicting_local_owner,
            "returned conflicting local caller",
        ),
        (
            returned_conflicting_other_owner,
            "returned conflicting other caller",
        ),
    ] {
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{label} should receive outgoing call context"));
        assert_eq!(context.len(), 2, "{label} context: {context:#?}");

        let helper = context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Path
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: vec!["return_conflicting_forwarded_function_pointer".to_string()],
                        }
            })
            .unwrap_or_else(|| panic!("{label} should include the returned helper path call"));
        assert_eq!(helper.status, CallStatusKind::Resolved);
        assert_eq!(helper.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(helper.targets.len(), 1);
        assert_eq!(helper.targets[0].target_id, returned_conflicting_helper);
        assert_eq!(helper.targets[0].relation, CallTargetKind::Function);

        let dynamic = context
            .iter()
            .find(|call| call.kind == CallSiteKind::Dynamic)
            .unwrap_or_else(|| panic!("{label} should include an outer dynamic call"));
        assert_eq!(dynamic.callee, CallCalleeInfo::Dynamic);
        assert_ambiguous_call_candidates(
            dynamic,
            local_target,
            other_target,
            CallTargetKind::DynamicFunction,
            label,
        );
    }

    let closure_cases = [
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1431:
        // `make_closure()()` returns a direct closure literal.
        (returned_closure_owner, make_closure, "make_closure"),
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1453:
        // `make_bound_closure()()` returns a local closure binding.
        (bound_owner, make_bound, "make_bound_closure"),
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1500:
        // `make_alias_bound_closure()()` returns a local alias of a closure binding.
        (alias_owner, make_alias, "make_alias_bound_closure"),
    ];
    for (owner, maker, name) in closure_cases {
        let context = call_context
            .get(&owner)
            .expect("returned closure owner should receive outgoing call context");
        assert_eq!(context.len(), 2, "returned closure context: {context:#?}");

        let path = context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Path
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: vec![name.to_string()],
                        }
            })
            .expect("inner returned-closure maker path call should stay visible");
        assert_eq!(path.status, CallStatusKind::Resolved);
        assert_eq!(path.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(path.targets.len(), 1);
        assert_eq!(path.targets[0].target_id, maker);
        assert_eq!(path.targets[0].relation, CallTargetKind::Function);

        let dynamic = context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Dynamic
                    && call
                        .targets
                        .iter()
                        .any(|target| target.relation == CallTargetKind::DynamicClosure)
            })
            .expect("outer returned-closure dynamic call should stay visible");
        assert_eq!(dynamic.callee, CallCalleeInfo::Dynamic);
        assert_eq!(dynamic.status, CallStatusKind::Resolved);
        assert_eq!(dynamic.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(dynamic.targets.len(), 1);
        assert_ne!(
            dynamic.targets[0].target_id, maker,
            "outer returned-closure dynamic target should be the closure owner, not the maker function"
        );
        assert_eq!(dynamic.targets[0].relation, CallTargetKind::DynamicClosure);
    }

    for (owner, label) in [
        (
            returned_async_no_await_owner,
            "un-awaited returned async closure",
        ),
        (
            returned_async_awaited_owner,
            "awaited returned async closure",
        ),
        (
            returned_async_stored_owner,
            "stored returned async closure future",
        ),
    ] {
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:2355-2365:
        // both owners call the same async-closure maker, but only the awaited
        // and stored callers poll the returned future and may receive a
        // DynamicClosure edge.
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{label} should receive outgoing call context"));
        assert_eq!(context.len(), 2, "{label} context: {context:#?}");

        let path = context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Path
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: vec!["make_returned_async_closure".to_string()],
                        }
            })
            .unwrap_or_else(|| panic!("{label} should include the async closure maker path call"));
        assert_eq!(path.status, CallStatusKind::Resolved);
        assert_eq!(path.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(path.targets.len(), 1);
        assert_eq!(path.targets[0].target_id, make_returned_async);
        assert_eq!(path.targets[0].relation, CallTargetKind::Function);

        let dynamic = context
            .iter()
            .find(|call| call.kind == CallSiteKind::Dynamic)
            .unwrap_or_else(|| panic!("{label} should include an outer dynamic call"));
        assert_eq!(dynamic.callee, CallCalleeInfo::Dynamic);
        if owner == returned_async_no_await_owner {
            assert_eq!(dynamic.status, CallStatusKind::Unsupported);
            assert_eq!(dynamic.resolution, None);
            assert!(
                dynamic.targets.is_empty(),
                "un-awaited returned async closure must not fabricate a closure target: {dynamic:#?}"
            );
        } else {
            assert_eq!(dynamic.status, CallStatusKind::Resolved);
            assert_eq!(dynamic.resolution, Some(CallResolutionKind::LocalExact));
            assert_eq!(dynamic.targets.len(), 1);
            assert_ne!(
                dynamic.targets[0].target_id, make_returned_async,
                "awaited returned async closure dynamic target should be the closure owner, not the maker function"
            );
            assert_eq!(dynamic.targets[0].relation, CallTargetKind::DynamicClosure);
        }
    }

    let branch_context = call_context
        .get(&branch_owner)
        .expect("branch-initialized function item owner should receive outgoing call context");
    assert_eq!(
        branch_context.len(),
        1,
        "branch-initialized function item context: {branch_context:#?}"
    );
    let branch_call = &branch_context[0];
    assert_eq!(branch_call.kind, CallSiteKind::Path);
    assert_eq!(
        branch_call.callee,
        CallCalleeInfo::Path {
            path: vec!["f".to_string()],
        }
    );
    assert_eq!(branch_call.status, CallStatusKind::Resolved);
    assert_eq!(branch_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(branch_call.targets.len(), 1);
    assert_eq!(branch_call.targets[0].target_id, local_target);
    assert_eq!(branch_call.targets[0].relation, CallTargetKind::Function);

    let block_context = call_context
        .get(&block_owner)
        .expect("block-initialized function item owner should receive outgoing call context");
    assert_eq!(
        block_context.len(),
        1,
        "block-initialized function item context: {block_context:#?}"
    );
    let block_call = &block_context[0];
    assert_eq!(block_call.kind, CallSiteKind::Path);
    assert_eq!(
        block_call.callee,
        CallCalleeInfo::Path {
            path: vec!["f".to_string()],
        }
    );
    assert_eq!(block_call.status, CallStatusKind::Resolved);
    assert_eq!(block_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(block_call.targets.len(), 1);
    assert_eq!(block_call.targets[0].target_id, local_target);
    assert_eq!(block_call.targets[0].relation, CallTargetKind::Function);

    let fn_param_context = call_context
        .get(&fn_param_owner)
        .expect("function-pointer param owner should receive outgoing call context");
    assert_eq!(
        fn_param_context.len(),
        1,
        "function-pointer param context: {fn_param_context:#?}"
    );
    let fn_param_call = &fn_param_context[0];
    assert_eq!(fn_param_call.kind, CallSiteKind::Path);
    assert_eq!(
        fn_param_call.callee,
        CallCalleeInfo::Path {
            path: vec!["f".to_string()],
        }
    );
    assert_eq!(fn_param_call.status, CallStatusKind::Unsupported);
    assert!(fn_param_call.resolution.is_none());
    assert!(
        fn_param_call.targets.is_empty(),
        "opaque fn pointer path calls must not fabricate RAG targets: {fn_param_call:#?}"
    );

    let single_param_context = call_context
        .get(&single_param_owner)
        .expect("single-caller function-pointer param owner should receive outgoing call context");
    let single_param_call = single_param_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["f".to_string()],
                    }
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == local_target)
        })
        .unwrap_or_else(|| {
            panic!(
                "single-caller function-pointer param context should include resolved f() -> local_target: {single_param_context:#?}"
            )
        });
    assert_eq!(single_param_call.status, CallStatusKind::Resolved);
    assert_eq!(
        single_param_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(single_param_call.targets.len(), 1);
    assert_eq!(single_param_call.targets[0].target_id, local_target);
    assert_eq!(
        single_param_call.targets[0].relation,
        CallTargetKind::Function
    );

    let multi_param_context = call_context
        .get(&multi_param_owner)
        .expect("same-target multi-caller function-pointer param owner should receive outgoing call context");
    let multi_param_call = multi_param_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["f".to_string()],
                    }
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == local_target)
        })
        .unwrap_or_else(|| {
            panic!(
                "same-target multi-caller function-pointer param context should include resolved f() -> local_target: {multi_param_context:#?}"
            )
        });
    assert_eq!(multi_param_call.status, CallStatusKind::Resolved);
    assert_eq!(
        multi_param_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(multi_param_call.targets.len(), 1);
    assert_eq!(multi_param_call.targets[0].target_id, local_target);
    assert_eq!(
        multi_param_call.targets[0].relation,
        CallTargetKind::Function
    );

    for (owner, label) in [
        (forwarded_param_owner, "forwarded function-pointer leaf"),
        (
            two_hop_forwarded_param_owner,
            "two-hop forwarded function-pointer leaf",
        ),
        (
            forwarded_referenced_owner,
            "forwarded referenced dyn Fn leaf",
        ),
        (
            two_hop_forwarded_referenced_owner,
            "two-hop forwarded referenced dyn Fn leaf",
        ),
    ] {
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{label} should receive outgoing call context"));
        let call = context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Path
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: vec!["f".to_string()],
                        }
                    && call
                        .targets
                        .iter()
                        .any(|target| target.target_id == local_target)
            })
            .unwrap_or_else(|| {
                panic!("{label} context should include resolved f() -> local_target: {context:#?}")
            });
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, local_target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Function);
    }

    let multi_conflicting_context = call_context
        .get(&multi_conflicting_param_owner)
        .expect("conflicting multi-caller function-pointer param owner should receive outgoing call context");
    let multi_conflicting_matches = multi_conflicting_context
        .iter()
        .filter(|call| {
            call.owner_id == multi_conflicting_param_owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["f".to_string()],
                    }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        multi_conflicting_matches.len(),
        1,
        "conflicting multi-caller function-pointer param body context: {multi_conflicting_context:#?}"
    );
    let multi_conflicting_call = multi_conflicting_matches[0];
    assert_eq!(multi_conflicting_call.kind, CallSiteKind::Path);
    assert_eq!(
        multi_conflicting_call.callee,
        CallCalleeInfo::Path {
            path: vec!["f".to_string()],
        }
    );
    assert_eq!(multi_conflicting_call.status, CallStatusKind::Ambiguous);
    assert!(multi_conflicting_call.resolution.is_none());
    assert_ambiguous_call_candidates(
        multi_conflicting_call,
        local_target,
        other_target,
        CallTargetKind::Function,
        "conflicting multi-caller function-pointer",
    );

    let forwarded_conflicting_context = call_context
        .get(&forwarded_conflicting_owner)
        .expect("forwarded conflicting function-pointer leaf should receive outgoing call context");
    let forwarded_conflicting_matches = forwarded_conflicting_context
        .iter()
        .filter(|call| {
            call.owner_id == forwarded_conflicting_owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["f".to_string()],
                    }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        forwarded_conflicting_matches.len(),
        1,
        "forwarded conflicting function-pointer leaf context: {forwarded_conflicting_context:#?}"
    );
    let forwarded_conflicting_call = forwarded_conflicting_matches[0];
    assert_eq!(forwarded_conflicting_call.status, CallStatusKind::Ambiguous);
    assert!(forwarded_conflicting_call.resolution.is_none());
    assert_ambiguous_call_candidates(
        forwarded_conflicting_call,
        local_target,
        other_target,
        CallTargetKind::Function,
        "forwarded conflicting function-pointer",
    );

    let two_hop_forwarded_conflicting_context = call_context
        .get(&two_hop_forwarded_conflicting_owner)
        .expect("two-hop forwarded conflicting function-pointer leaf should receive outgoing call context");
    let two_hop_forwarded_conflicting_matches = two_hop_forwarded_conflicting_context
        .iter()
        .filter(|call| {
            call.owner_id == two_hop_forwarded_conflicting_owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["f".to_string()],
                    }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        two_hop_forwarded_conflicting_matches.len(),
        1,
        "two-hop forwarded conflicting function-pointer leaf context: {two_hop_forwarded_conflicting_context:#?}"
    );
    let two_hop_forwarded_conflicting_call = two_hop_forwarded_conflicting_matches[0];
    assert_eq!(
        two_hop_forwarded_conflicting_call.status,
        CallStatusKind::Ambiguous
    );
    assert!(two_hop_forwarded_conflicting_call.resolution.is_none());
    assert_ambiguous_call_candidates(
        two_hop_forwarded_conflicting_call,
        local_target,
        other_target,
        CallTargetKind::Function,
        "two-hop forwarded conflicting function-pointer",
    );

    let multi_conflicting_generic_context =
        call_context.get(&multi_conflicting_generic_owner).expect(
            "conflicting multi-caller generic FnOnce owner should receive outgoing call context",
        );
    let multi_conflicting_generic_matches = multi_conflicting_generic_context
        .iter()
        .filter(|call| {
            call.owner_id == multi_conflicting_generic_owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["generic_f".to_string()],
                    }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        multi_conflicting_generic_matches.len(),
        1,
        "conflicting multi-caller generic FnOnce body context: {multi_conflicting_generic_context:#?}"
    );
    let multi_conflicting_generic_call = multi_conflicting_generic_matches[0];
    assert_eq!(
        multi_conflicting_generic_call.status,
        CallStatusKind::Ambiguous
    );
    assert!(multi_conflicting_generic_call.resolution.is_none());
    assert_ambiguous_call_candidates(
        multi_conflicting_generic_call,
        local_target,
        other_target,
        CallTargetKind::Function,
        "conflicting multi-caller generic FnOnce",
    );

    let multi_conflicting_field_context = call_context
        .get(&multi_conflicting_field_owner)
        .expect("conflicting multi-caller named-field owner should receive outgoing call context");
    let multi_conflicting_field_matches = multi_conflicting_field_context
        .iter()
        .filter(|call| {
            call.owner_id == multi_conflicting_field_owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
        })
        .collect::<Vec<_>>();
    assert_eq!(
        multi_conflicting_field_matches.len(),
        1,
        "conflicting multi-caller named-field body context: {multi_conflicting_field_context:#?}"
    );
    let multi_conflicting_field_call = multi_conflicting_field_matches[0];
    assert_eq!(
        multi_conflicting_field_call.status,
        CallStatusKind::Ambiguous
    );
    assert!(multi_conflicting_field_call.resolution.is_none());
    assert_ambiguous_call_candidates(
        multi_conflicting_field_call,
        local_target,
        other_target,
        CallTargetKind::DynamicFunction,
        "conflicting multi-caller named-field",
    );

    let forwarded_conflicting_field_context = call_context
        .get(&forwarded_conflicting_field_owner)
        .expect("forwarded conflicting named-field leaf should receive outgoing call context");
    let forwarded_conflicting_field_matches = forwarded_conflicting_field_context
        .iter()
        .filter(|call| {
            call.owner_id == forwarded_conflicting_field_owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
        })
        .collect::<Vec<_>>();
    assert_eq!(
        forwarded_conflicting_field_matches.len(),
        1,
        "forwarded conflicting named-field leaf context: {forwarded_conflicting_field_context:#?}"
    );
    let forwarded_conflicting_field_call = forwarded_conflicting_field_matches[0];
    assert_eq!(
        forwarded_conflicting_field_call.status,
        CallStatusKind::Ambiguous
    );
    assert!(forwarded_conflicting_field_call.resolution.is_none());
    assert_ambiguous_call_candidates(
        forwarded_conflicting_field_call,
        local_target,
        other_target,
        CallTargetKind::DynamicFunction,
        "forwarded conflicting named-field",
    );

    let single_parenthesized_param_context = call_context
        .get(&single_parenthesized_param_owner)
        .expect("single-caller parenthesized function-pointer param owner should receive outgoing call context");
    let single_parenthesized_param_call = single_parenthesized_param_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == local_target)
        })
        .unwrap_or_else(|| {
            panic!(
                "single-caller parenthesized function-pointer param context should include resolved (f)() -> local_target: {single_parenthesized_param_context:#?}"
            )
        });
    assert_eq!(
        single_parenthesized_param_call.status,
        CallStatusKind::Resolved
    );
    assert_eq!(
        single_parenthesized_param_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(single_parenthesized_param_call.targets.len(), 1);
    assert_eq!(
        single_parenthesized_param_call.targets[0].target_id,
        local_target
    );
    assert_eq!(
        single_parenthesized_param_call.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    let generic_context = call_context
        .get(&generic_owner)
        .expect("generic FnOnce owner should receive outgoing call context");
    assert_eq!(
        generic_context.len(),
        1,
        "generic FnOnce context: {generic_context:#?}"
    );
    let generic_call = &generic_context[0];
    assert_eq!(generic_call.kind, CallSiteKind::Path);
    assert_eq!(
        generic_call.callee,
        CallCalleeInfo::Path {
            path: vec!["generic_f".to_string()],
        }
    );
    assert_eq!(generic_call.status, CallStatusKind::Unsupported);
    assert!(generic_call.resolution.is_none());
    assert!(
        generic_call.targets.is_empty(),
        "generic FnOnce path calls must not fabricate RAG targets: {generic_call:#?}"
    );

    let boxed_context = call_context
        .get(&boxed_owner)
        .expect("boxed dyn Fn owner should receive outgoing call context");
    assert_eq!(
        boxed_context.len(),
        2,
        "boxed dyn Fn context: {boxed_context:#?}"
    );
    let box_new = boxed_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["Box".to_string(), "new".to_string()],
                    }
        })
        .expect("Box::new setup call should stay visible");
    assert_eq!(box_new.status, CallStatusKind::External);
    assert!(box_new.resolution.is_none());
    assert!(box_new.targets.is_empty());

    let boxed_call = boxed_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["boxed_fn".to_string()],
                    }
        })
        .expect("boxed dyn Fn path call should stay visible");
    assert_eq!(boxed_call.status, CallStatusKind::Resolved);
    assert_eq!(boxed_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(boxed_call.targets.len(), 1);
    assert_eq!(boxed_call.targets[0].target_id, local_target);
    assert_eq!(boxed_call.targets[0].relation, CallTargetKind::Function);

    let vec_context = call_context
        .get(&vec_owner)
        .expect("Vec::new owner should receive outgoing call context");
    assert_eq!(vec_context.len(), 1, "Vec::new context: {vec_context:#?}");
    let vec_new = &vec_context[0];
    assert_eq!(vec_new.kind, CallSiteKind::Path);
    assert_eq!(
        vec_new.callee,
        CallCalleeInfo::Path {
            path: vec!["Vec".to_string(), "new".to_string()],
        }
    );
    assert_eq!(vec_new.status, CallStatusKind::External);
    assert!(vec_new.resolution.is_none());
    assert!(
        vec_new.targets.is_empty(),
        "Vec::new external calls must not fabricate RAG targets: {vec_new:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_generic_fn_once_parameter()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    struct Case {
        owner: &'static str,
        source: &'static str,
        kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
    }

    let cases = [
        Case {
            owner: "call_single_generic_fn_once_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1519-1524 `generic_f()`",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["generic_f".to_string()],
            },
            relation: CallTargetKind::Function,
        },
        Case {
            owner: "call_multi_generic_fn_once_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1764-1769 `generic_f()`",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["generic_f".to_string()],
            },
            relation: CallTargetKind::Function,
        },
        Case {
            owner: "call_single_parenthesized_generic_fn_once_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1729-1734 `(generic_f)()`",
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
    ];
    let owners = cases
        .iter()
        .map(|case| {
            one_uuid(&db, &function_in_module_query(&["crate"], case.owner)).map(|id| (id, 1.0))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let call_context = rag.collect_call_context(&owners)?;

    for (case, (owner, _score)) in cases.iter().zip(owners.iter().copied()) {
        let context = call_context.get(&owner).unwrap_or_else(|| {
            panic!(
                "{} single-caller generic FnOnce owner should receive outgoing call context",
                case.owner
            )
        });
        let call = context
            .iter()
            .find(|call| {
                call.kind == case.kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target| target.target_id == local_target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should include resolved generic FnOnce parameter call -> local_target from {}: {context:#?}",
                    case.owner, case.source
                )
            });

        // These private helpers each have one local caller that supplies
        // `local_target`, so this is exact value-flow proof, not broad
        // callable-trait dispatch.
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, local_target);
        assert_eq!(call.targets[0].relation, case.relation);
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_callable_trait_object_parameters()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    struct Case {
        owner: &'static str,
        source: &'static str,
        kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
        proof_note: &'static str,
    }

    let cases = [
        Case {
            owner: "call_single_referenced_dyn_fn_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:2148-2153 `f()` with `&dyn Fn` parameter",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["f".to_string()],
            },
            relation: CallTargetKind::Function,
            proof_note: "caller supplies `&local_target`",
        },
        Case {
            owner: "call_single_parenthesized_referenced_dyn_fn_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:2156-2161 `(f)()` with `&dyn Fn` parameter",
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
            proof_note: "caller supplies `&local_target`",
        },
        Case {
            owner: "call_single_boxed_dyn_fn_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs EOF `f()` with `Box<dyn Fn>` parameter",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["f".to_string()],
            },
            relation: CallTargetKind::Function,
            proof_note: "caller supplies `Box::new(local_target)`",
        },
        Case {
            owner: "call_single_parenthesized_boxed_dyn_fn_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs EOF `(f)()` with `Box<dyn Fn>` parameter",
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
            proof_note: "caller supplies `Box::new(local_target)`",
        },
        Case {
            owner: "call_forwarded_boxed_dyn_fn_leaf",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs EOF `f()` with forwarded `Box<dyn Fn>` parameter",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["f".to_string()],
            },
            relation: CallTargetKind::Function,
            proof_note: "private forwarding chain ends in `Box::new(local_target)`",
        },
        Case {
            owner: "call_two_hop_forwarded_boxed_dyn_fn_leaf",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs EOF `f()` with two-hop forwarded `Box<dyn Fn>` parameter",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["f".to_string()],
            },
            relation: CallTargetKind::Function,
            proof_note: "two-hop private forwarding chain ends in `Box::new(local_target)`",
        },
    ];
    let owners = cases
        .iter()
        .map(|case| {
            one_uuid(&db, &function_in_module_query(&["crate"], case.owner)).map(|id| (id, 1.0))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let call_context = rag.collect_call_context(&owners)?;

    for (case, (owner, _score)) in cases.iter().zip(owners.iter().copied()) {
        let context = call_context.get(&owner).unwrap_or_else(|| {
            panic!(
                "{} callable trait-object owner should receive outgoing call context",
                case.owner
            )
        });
        let call = context
            .iter()
            .find(|call| {
                call.kind == case.kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target| target.target_id == local_target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should include resolved callable trait-object parameter call -> local_target from {} ({note}): {context:#?}",
                    case.owner,
                    case.source,
                    note = case.proof_note
                )
            });

        // These private helpers each have one local caller that supplies
        // the exact callable target, so this is complete private-caller proof,
        // not broad callable trait-object dispatch.
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, local_target);
        assert_eq!(call.targets[0].relation, case.relation);
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_result_method_callback()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_result_callback"),
    )?;
    let local_target = unique_id_by_name(&db, "function", "local_result_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .expect("single-caller result callback owner should receive outgoing call context");
    let call = context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "and_then".to_string(),
                        receiver: Some(CallReceiverInfo::PathCallResult {
                            path: vec!["Ok".to_string()],
                        }),
                    }
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == local_target)
        })
        .unwrap_or_else(|| {
            panic!(
                "single-caller result callback context should include resolved and_then(f) -> local_result_target: {context:#?}"
            )
        });

    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // `call_single_result_callback(f)` is private and every local caller passes
    // `local_result_target`, so this is complete private-caller proof for the
    // callback argument of `Ok::<i32, ()>(1).and_then(f)`.
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, local_target);
    assert_eq!(
        call.targets[0].relation,
        CallTargetKind::MethodCallbackFunction
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_branch_parameter_calls()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    let cases = [
        (
            "call_single_if_function_pointer_param_branch",
            "tests/fixture_crates/fixture_call_graph/src/lib.rs:1655 `(if flag { f } else { f })()`",
        ),
        (
            "call_single_match_function_pointer_param_arm",
            "tests/fixture_crates/fixture_call_graph/src/lib.rs:1663-1666 match arms return `f`",
        ),
    ];
    let owners = cases
        .iter()
        .map(|(owner, _source)| {
            one_uuid(&db, &function_in_module_query(&["crate"], owner)).map(|id| (id, 1.0))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let call_context = rag.collect_call_context(&owners)?;

    for ((owner_name, source), (owner, _score)) in cases.iter().zip(owners.iter().copied()) {
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{owner_name} should receive outgoing call context"));
        let call = context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Dynamic
                    && call.callee == CallCalleeInfo::Dynamic
                    && call
                        .targets
                        .iter()
                        .any(|target| target.target_id == local_target)
            })
            .unwrap_or_else(|| {
                panic!("{owner_name} should include resolved branch parameter call -> local_target from {source}: {context:#?}")
            });

        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, local_target);
        assert_eq!(call.targets[0].relation, CallTargetKind::DynamicFunction);
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_aliased_callable_parameters()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    struct Case {
        owner: &'static str,
        source: &'static str,
        kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
    }

    let cases = [
        Case {
            owner: "call_single_aliased_function_pointer_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1673-1675 `let g = f; g()`",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["g".to_string()],
            },
            relation: CallTargetKind::Function,
        },
        Case {
            owner: "call_single_parenthesized_aliased_function_pointer_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1682-1684 `let g = f; (g)()`",
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
    ];
    let owners = cases
        .iter()
        .map(|case| {
            one_uuid(&db, &function_in_module_query(&["crate"], case.owner)).map(|id| (id, 1.0))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let call_context = rag.collect_call_context(&owners)?;

    for (case, (owner, _score)) in cases.iter().zip(owners.iter().copied()) {
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.owner));
        let call = context
            .iter()
            .find(|call| {
                call.kind == case.kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target| target.target_id == local_target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should include resolved aliased parameter call -> local_target from {}: {context:#?}",
                    case.owner, case.source
                )
            });

        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, local_target);
        assert_eq!(call.targets[0].relation, case.relation);
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_function_pointer_cast_parameter()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_function_pointer_param_cast"),
    )?;
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .expect("single-caller function-pointer cast owner should receive outgoing call context");
    let call = context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == local_target)
        })
        .unwrap_or_else(|| {
            panic!(
                "single-caller function-pointer cast context should include resolved (f as fn() -> i32)() -> local_target: {context:#?}"
            )
        });

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1564-1569:
    // the helper is private and every local caller supplies `local_target`,
    // so this cast form reuses the existing exact parameter proof boundary.
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, local_target);
    assert_eq!(call.targets[0].relation, CallTargetKind::DynamicFunction);

    Ok(())
}
