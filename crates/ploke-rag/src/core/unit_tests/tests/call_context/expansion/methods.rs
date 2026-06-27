use super::super::super::*;
#[tokio::test]
async fn call_context_expansion_adds_incoming_fixture_method_callers() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let method_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_typed_local_instance_method"),
    )?;
    let assoc_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_method_as_associated_function"),
    )?;

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 64;
    rag.cfg.call_context.max_caller_hits = 64;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&target),
        "incoming method caller expansion must preserve the seed target; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&method_owner),
        "method target expansion should materialize the method-call owner; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&assoc_owner),
        "method target expansion should materialize the associated-function call owner; expanded: {expanded:#?}"
    );

    let call_context = rag.collect_call_context(&expanded)?;
    let method_context = call_context
        .get(&method_owner)
        .expect("method-call owner should receive outgoing call context");
    let method_call = method_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "instance_value".to_string(),
                        receiver: Some(CallReceiverInfo::TypedLocalBinding {
                            name: "value".to_string(),
                            type_path: vec!["LocalAssoc".to_string()],
                        }),
                    }
        })
        .expect("caller should preserve the method call edge to LocalAssoc::instance_value");
    assert_eq!(method_call.status, CallStatusKind::Resolved);
    assert_eq!(method_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(method_call.targets.len(), 1);
    assert_eq!(method_call.targets[0].target_id, target);
    assert_eq!(method_call.targets[0].relation, CallTargetKind::Method);

    let assoc_context = call_context
        .get(&assoc_owner)
        .expect("associated-function caller should receive outgoing call context");
    let assoc_call = assoc_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["LocalAssoc".to_string(), "instance_value".to_string()],
                    }
        })
        .expect(
            "caller should preserve the associated-function edge to LocalAssoc::instance_value",
        );
    assert_eq!(assoc_call.status, CallStatusKind::Resolved);
    assert_eq!(assoc_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(assoc_call.targets.len(), 1);
    assert_eq!(assoc_call.targets[0].target_id, target);
    assert_eq!(
        assoc_call.targets[0].relation,
        CallTargetKind::AssociatedFunction
    );

    Ok(())
}
