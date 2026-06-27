use super::*;

#[tokio::test]
async fn request_code_context_returns_function_and_dynamic_owner_call_context()
-> color_eyre::Result<()> {
    struct Case<'a> {
        label: &'a str,
        search_term: &'a str,
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
            owner: "call_parenthesized_local_target",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "aliased indexed dynamic function caller",
            search_term: "call_aliased_indexed_named_field_function_binding",
            owner: "call_aliased_indexed_named_field_function_binding",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
    ];

    for case in cases {
        let owner = one_uuid(&db, &function_in_module_query(&["crate"], case.owner))?;
        let result =
            execute_fixture_request(&db, case.search_term, 1, "local_target_call_context").await?;
        assert_result_ok(&result, case.search_term, 1, "fixture_call_graph");

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
