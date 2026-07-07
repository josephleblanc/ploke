use super::super::super::super::super::*;
use super::super::super::helpers::*;
#[tokio::test]
async fn call_context_collection_reads_real_fixture_dynamic_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let resolved_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_aliased_indexed_named_field_function_binding",
        ),
    )?;
    let branch_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_parenthesized_match_initialized_function_item_binding",
        ),
    )?;
    let block_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_parenthesized_block_initialized_function_item_binding",
        ),
    )?;
    let guarded_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_match_guarded_function_item"),
    )?;
    let boxed_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_parenthesized_boxed_dyn_fn_value_binding"),
    )?;
    let closure_binding_owner =
        one_uuid(&db, &function_in_module_query(&["crate"], "dynamic_calls"))?;
    let dereferenced_closure_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_dereferenced_closure_binding"),
    )?;
    let field_param_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_indexed_field_function_param"),
    )?;
    let field_param_caller = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_single_indexed_field_function_param_with_local_target",
        ),
    )?;
    let named_field_param_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_named_field_function_param"),
    )?;
    let named_field_param_caller = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_single_named_field_function_param_with_local_target",
        ),
    )?;
    let tuple_field_param_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_indexed_tuple_field_function_param"),
    )?;
    let tuple_field_param_caller = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_single_indexed_tuple_field_function_param_with_local_target",
        ),
    )?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable dynamic call context collection"
    );

    let call_context = rag.collect_call_context(&[
        (resolved_owner, 1.0),
        (branch_owner, 1.0),
        (block_owner, 1.0),
        (guarded_owner, 1.0),
        (boxed_owner, 1.0),
        (closure_binding_owner, 1.0),
        (dereferenced_closure_owner, 1.0),
        (named_field_param_owner, 1.0),
        (field_param_owner, 1.0),
        (tuple_field_param_owner, 1.0),
    ])?;
    let resolved_context = call_context
        .get(&resolved_owner)
        .expect("resolved dynamic owner should receive outgoing call context");
    let resolved_call = resolved_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("resolved dynamic function call should stay visible in RAG call context");
    assert_eq!(resolved_call.status, CallStatusKind::Resolved);
    assert_eq!(
        resolved_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(resolved_call.targets.len(), 1);
    assert_eq!(resolved_call.targets[0].target_id, target);
    assert_eq!(
        resolved_call.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    for (label, owner) in [
        ("branch-initialized", branch_owner),
        ("block-initialized", block_owner),
        ("guarded-match", guarded_owner),
    ] {
        let context = call_context.get(&owner).unwrap_or_else(|| {
            panic!("{label} dynamic owner should receive outgoing call context")
        });
        assert_eq!(
            context.len(),
            1,
            "{label} dynamic owner context: {context:#?}"
        );
        let call = &context[0];
        assert_eq!(call.kind, CallSiteKind::Dynamic, "{label}");
        assert_eq!(call.callee, CallCalleeInfo::Dynamic, "{label}");
        assert_eq!(call.status, CallStatusKind::Resolved, "{label}");
        assert_eq!(
            call.resolution,
            Some(CallResolutionKind::LocalExact),
            "{label}"
        );
        assert_eq!(call.targets.len(), 1, "{label}");
        assert_eq!(call.targets[0].target_id, target, "{label}");
        assert_eq!(
            call.targets[0].relation,
            CallTargetKind::DynamicFunction,
            "{label}"
        );
    }

    let boxed_context = call_context
        .get(&boxed_owner)
        .expect("boxed dyn Fn dynamic owner should receive outgoing call context");
    assert_eq!(
        boxed_context.len(),
        2,
        "boxed dyn Fn dynamic owner context: {boxed_context:#?}"
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
        .expect("Box::new setup call should stay visible for boxed dyn Fn dynamic owner");
    assert_eq!(box_new.status, CallStatusKind::External);
    assert!(box_new.resolution.is_none());
    assert!(box_new.targets.is_empty());
    let boxed_call = boxed_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("boxed dyn Fn dynamic call should resolve to local_target");
    assert_eq!(boxed_call.status, CallStatusKind::Resolved);
    assert_eq!(boxed_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(boxed_call.targets.len(), 1);
    assert_eq!(boxed_call.targets[0].target_id, target);
    assert_eq!(
        boxed_call.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    let closure_binding_context = call_context
        .get(&closure_binding_owner)
        .expect("dynamic closure binding owner should receive outgoing call context");
    assert_eq!(
        closure_binding_context.len(),
        2,
        "dynamic closure binding owner context: {closure_binding_context:#?}"
    );
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:5-6:
    // `(closure)()` and `(|| 11)()` should both resolve to local closure
    // executable owners. The literal call remains pathless, so RAG exposes both
    // callees as generic dynamic calls with `DynamicClosure` targets.
    let dynamic_closure_calls = closure_binding_context
        .iter()
        .filter(|call| {
            call.kind == CallSiteKind::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.relation == CallTargetKind::DynamicClosure)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_closure_calls.len(),
        2,
        "dynamic_calls should expose named and literal closure targets: {closure_binding_context:#?}"
    );
    for call in dynamic_closure_calls {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.callee, CallCalleeInfo::Dynamic);
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].relation, CallTargetKind::DynamicClosure);
    }

    let dereferenced_closure_context = call_context
        .get(&dereferenced_closure_owner)
        .expect("dereferenced closure owner should receive outgoing call context");
    assert_eq!(
        dereferenced_closure_context.len(),
        1,
        "dereferenced closure owner context: {dereferenced_closure_context:#?}"
    );
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:692-694:
    // `let closure = || 34; (*closure)()` carries exact local closure-binding
    // proof, so RAG should expose the same resolved DynamicClosure edge that
    // the DB call graph stores.
    let dereferenced_call = &dereferenced_closure_context[0];
    assert_eq!(dereferenced_call.kind, CallSiteKind::Dynamic);
    assert_eq!(dereferenced_call.callee, CallCalleeInfo::Dynamic);
    assert_eq!(dereferenced_call.status, CallStatusKind::Resolved);
    assert_eq!(
        dereferenced_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(
        dereferenced_call.targets.len(),
        1,
        "dereferenced closure call should resolve to one closure owner: {dereferenced_call:#?}"
    );
    assert_eq!(
        dereferenced_call.targets[0].relation,
        CallTargetKind::DynamicClosure
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1555 and 1598-1606:
    // private helper parameters receive constructed holder values from a
    // single local caller, so DB proof resolves `(holder.callback)()`,
    // `holder.callbacks[0]()` and `holder.0[0]()` to `local_target`. RAG node
    // context also includes the incoming wrapper helper call, while still
    // exposing pathless dynamic callees as generic `Dynamic` calls.
    for (label, owner, caller, helper_path) in [
        (
            "single-caller-named-field-parameter",
            named_field_param_owner,
            named_field_param_caller,
            vec!["call_single_named_field_function_param".to_string()],
        ),
        (
            "single-caller-field-parameter",
            field_param_owner,
            field_param_caller,
            vec!["call_single_indexed_field_function_param".to_string()],
        ),
        (
            "single-caller-tuple-field-parameter",
            tuple_field_param_owner,
            tuple_field_param_caller,
            vec!["call_single_indexed_tuple_field_function_param".to_string()],
        ),
    ] {
        let context = call_context.get(&owner).unwrap_or_else(|| {
            panic!("{label} dynamic owner should receive outgoing call context")
        });
        assert_eq!(
            context.len(),
            2,
            "{label} dynamic owner context: {context:#?}"
        );
        let call = context
            .iter()
            .find(|call| call.owner_id == owner && call.kind == CallSiteKind::Dynamic)
            .unwrap_or_else(|| {
                panic!("{label} should expose outgoing indexed dynamic call: {context:#?}")
            });
        assert_eq!(call.kind, CallSiteKind::Dynamic, "{label}");
        assert_eq!(call.callee, CallCalleeInfo::Dynamic, "{label}");
        assert_eq!(call.status, CallStatusKind::Resolved, "{label}");
        assert_eq!(
            call.resolution,
            Some(CallResolutionKind::LocalExact),
            "{label}"
        );
        assert_eq!(call.targets.len(), 1, "{label}: {call:#?}");
        assert_eq!(call.targets[0].target_id, target, "{label}");
        assert_eq!(
            call.targets[0].relation,
            CallTargetKind::DynamicFunction,
            "{label}"
        );

        let incoming = context
            .iter()
            .find(|call| call.owner_id == caller && call.kind == CallSiteKind::Path)
            .unwrap_or_else(|| {
                panic!("{label} should expose incoming wrapper helper call: {context:#?}")
            });
        assert_eq!(
            incoming.callee,
            CallCalleeInfo::Path { path: helper_path },
            "{label}"
        );
        assert_eq!(incoming.arg_count, Some(1), "{label}");
        assert_eq!(incoming.status, CallStatusKind::Resolved, "{label}");
        assert_eq!(
            incoming.resolution,
            Some(CallResolutionKind::LocalExact),
            "{label}"
        );
        assert_eq!(incoming.targets.len(), 1, "{label}: {incoming:#?}");
        assert_eq!(incoming.targets[0].target_id, owner, "{label}");
        assert_eq!(
            incoming.targets[0].relation,
            CallTargetKind::Function,
            "{label}"
        );
    }

    Ok(())
}
