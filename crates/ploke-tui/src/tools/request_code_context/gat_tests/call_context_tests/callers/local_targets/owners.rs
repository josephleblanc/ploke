use super::*;

#[tokio::test]
async fn request_code_context_returns_function_and_dynamic_owner_call_context()
-> color_eyre::Result<()> {
    struct Case<'a> {
        label: &'a str,
        search_term: &'a str,
        top_k: usize,
        owner: &'a str,
        call_kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
    }

    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;

    let cases = [
        Case {
            label: "ordinary path caller",
            search_term: "call_crate_local_target",
            top_k: 1,
            owner: "call_crate_local_target",
            call_kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: path(&["crate", "local_target"]),
            },
            relation: CallTargetKind::Function,
        },
        Case {
            label: "dynamic function caller",
            search_term: "call_parenthesized_local_target",
            top_k: 1,
            owner: "call_parenthesized_local_target",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "aliased indexed dynamic function caller",
            search_term: "call_aliased_indexed_named_field_function_binding",
            top_k: 1,
            owner: "call_aliased_indexed_named_field_function_binding",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "multi-caller named-field function parameter",
            search_term: "call_multi_named_field_function_param holder callback local_target",
            top_k: 5,
            owner: "call_multi_named_field_function_param",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "guarded match dynamic function caller",
            search_term: "call_match_guarded_function_item",
            top_k: 1,
            owner: "call_match_guarded_function_item",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "single-caller parenthesized function-pointer parameter",
            search_term: "call_single_parenthesized_function_pointer_param f fn i32",
            top_k: 1,
            owner: "call_single_parenthesized_function_pointer_param",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "single-caller if-branch function-pointer parameter",
            search_term: "call_single_if_function_pointer_param_branch f fn i32",
            top_k: 1,
            owner: "call_single_if_function_pointer_param_branch",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "single-caller match-arm function-pointer parameter",
            search_term: "call_single_match_function_pointer_param_arm f fn i32",
            top_k: 1,
            owner: "call_single_match_function_pointer_param_arm",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "single-caller function-pointer cast parameter",
            search_term: "call_single_function_pointer_param_cast f fn i32",
            top_k: 1,
            owner: "call_single_function_pointer_param_cast",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "dereferenced boxed dyn Fn exact initializer",
            search_term: "call_dereferenced_boxed_dyn_fn_value_binding boxed_fn",
            top_k: 1,
            owner: "call_dereferenced_boxed_dyn_fn_value_binding",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "single-caller generic FnOnce parameter",
            search_term: "call_single_generic_fn_once_param generic_f FnOnce",
            top_k: 1,
            owner: "call_single_generic_fn_once_param",
            call_kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: path(&["generic_f"]),
            },
            relation: CallTargetKind::Function,
        },
        Case {
            label: "single-caller parenthesized generic FnOnce parameter",
            search_term: "call_single_parenthesized_generic_fn_once_param generic_f FnOnce",
            top_k: 1,
            owner: "call_single_parenthesized_generic_fn_once_param",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
    ];

    for case in cases {
        let owner = one_uuid(&db, &function_in_module_query(&["crate"], case.owner))?;
        let result = execute_fixture_request(
            &db,
            case.search_term,
            case.top_k,
            "local_target_call_context",
        )
        .await?;
        assert_result_ok(&result, case.search_term, case.top_k, "fixture_call_graph");

        let target_part = result
            .context
            .iter()
            .find(|part| part.id == target)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the local_target outgoing callee for {}",
                    case.label
                )
            });
        let owner_part = result
            .context
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} owner",
                    case.label
                )
            });
        let call = owner_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == case.call_kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} owner should retain outgoing call context to local_target",
                    case.label
                )
            });
        assert_resolved_target(call, target, case.relation);
        assert_expansion(
            target_part,
            owner,
            target,
            call.site_id,
            CallExpansionKind::OutgoingTarget,
        );
    }

    Ok(())
}

#[tokio::test]
async fn request_code_context_returns_closure_binding_cast_owner_call_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_closure_binding_cast"),
    )?;
    let closure = closure_owner_for_parent(&db, owner)?;

    let result = execute_fixture_request(
        &db,
        "call_closure_binding_cast closure",
        1,
        "closure_binding_cast_call_context",
    )
    .await?;
    assert_result_ok(
        &result,
        "call_closure_binding_cast closure",
        1,
        "fixture_call_graph",
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:695-701:
    // `let closure = || 21; (closure as fn() -> i32)()` carries exact local
    // closure-binding proof. The tool should expose the dynamic call edge to
    // the closure executable owner without inventing a `local_target` edge.
    let owner_part = result
        .context
        .iter()
        .find(|part| part.id == owner)
        .expect("request_code_context should materialize the closure cast owner");
    let closure_part = result
        .context
        .iter()
        .find(|part| part.id == closure)
        .expect("request_code_context should materialize the closure executable owner");
    let call = owner_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == closure)
        })
        .expect("closure cast owner should retain outgoing dynamic closure call context");
    assert_resolved_target(call, closure, CallTargetKind::DynamicClosure);
    assert_expansion(
        closure_part,
        owner,
        closure,
        call.site_id,
        CallExpansionKind::OutgoingTarget,
    );

    Ok(())
}

#[tokio::test]
async fn request_code_context_attaches_incoming_context_to_target_seed() -> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "try_local_assoc"),
    )?;
    let caller = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_try_result_instance_method"),
    )?;

    let result = execute_fixture_request(
        &db,
        "try_local_assoc",
        1,
        "target_seed_incoming_call_context",
    )
    .await?;
    assert_result_ok(&result, "try_local_assoc", 1, "fixture_call_graph");

    let target_part = result
        .context
        .iter()
        .find(|part| part.id == target)
        .expect("request_code_context should preserve the target seed part");
    assert!(
        target_part.call_expansion.is_none(),
        "target seed should not be marked as an expanded caller: {target_part:#?}"
    );
    let call = target_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(&["try_local_assoc"]),
                    }
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("target seed should retain incoming call context from its caller");
    assert_resolved_target(call, target, CallTargetKind::Function);
    assert_eq!(
        call.owner_id, caller,
        "target-seed incoming call context should expose the caller owner id"
    );

    let caller_part = result
        .context
        .iter()
        .find(|part| part.id == caller)
        .expect("request_code_context should materialize the incoming caller owner");
    assert_expansion(
        caller_part,
        target,
        target,
        call.site_id,
        CallExpansionKind::IncomingCaller,
    );

    Ok(())
}

#[tokio::test]
async fn request_code_context_returns_single_caller_function_pointer_parameter_context()
-> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_function_pointer_param"),
    )?;
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;

    let search_term = "call_single_function_pointer_param f fn i32";
    let result =
        execute_fixture_request(&db, search_term, 1, "single_fn_param_call_context").await?;
    assert_result_ok(&result, search_term, 1, "fixture_call_graph");

    let owner_part = result
        .context
        .iter()
        .find(|part| part.id == owner)
        .expect("request_code_context should materialize the single-caller parameter owner");
    let target_part = result
        .context
        .iter()
        .find(|part| part.id == target)
        .expect("request_code_context should materialize the local_target callee");
    let call = owner_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee == CallCalleeInfo::Path { path: path(&["f"]) }
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("single-caller parameter owner should retain resolved f() call context");

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1503-1508:
    // the private helper's complete local caller set passes `local_target`, so
    // the tool should expose the resolved `f()` edge instead of the broader
    // fail-closed function-pointer parameter blocker shape.
    assert_resolved_target(call, target, CallTargetKind::Function);
    assert_expansion(
        target_part,
        owner,
        target,
        call.site_id,
        CallExpansionKind::OutgoingTarget,
    );

    Ok(())
}
