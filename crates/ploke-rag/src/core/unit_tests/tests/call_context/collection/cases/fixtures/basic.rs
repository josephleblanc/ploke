use super::super::super::super::super::*;
use super::super::super::helpers::*;
#[tokio::test]
async fn call_context_collection_reads_real_fixture_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_try_result_instance_method"),
    )?;
    let try_target = unique_id_by_name(&db, "function", "try_local_assoc")?;
    let method_target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let owner_context = call_context
        .get(&owner)
        .expect("fixture owner should receive outgoing call context");
    assert_eq!(owner_context.len(), 3, "owner context: {owner_context:#?}");

    let ok_call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["Ok".to_string()],
                    }
        })
        .expect("Ok wrapper call should stay visible");
    assert_eq!(ok_call.status, CallStatusKind::Unsupported);
    assert!(ok_call.resolution.is_none());
    assert!(ok_call.targets.is_empty());

    let try_call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["try_local_assoc".to_string()],
                    }
        })
        .expect("try_local_assoc path call should be present");
    assert_eq!(try_call.status, CallStatusKind::Resolved);
    assert_eq!(try_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(try_call.targets.len(), 1);
    assert_eq!(try_call.targets[0].target_id, try_target);
    assert_eq!(try_call.targets[0].relation, CallTargetKind::Function);

    let method_call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "instance_value".to_string(),
                        receiver: Some(CallReceiverInfo::TryPathCallResult {
                            path: vec!["try_local_assoc".to_string()],
                        }),
                    }
        })
        .expect("try-result method call should be present");
    assert_eq!(method_call.status, CallStatusKind::Resolved);
    assert_eq!(method_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(method_call.targets.len(), 1);
    assert_eq!(method_call.targets[0].target_id, method_target);
    assert_eq!(method_call.targets[0].relation, CallTargetKind::Method);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_incoming_rows_for_target_seed() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "try_local_assoc")?;
    let caller = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_try_result_instance_method"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(target, 1.0)])?;
    let target_context = call_context
        .get(&target)
        .expect("target seed should receive incoming caller context");
    let call = target_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["try_local_assoc".to_string()],
                    }
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .expect(
            "target seed should retain incoming call context from call_try_result_instance_method",
        );
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);

    let caller_context = db.call_context_for_owner(caller)?;
    assert!(
        caller_context.iter().any(|row| row.site.id == call.site_id),
        "incoming target context should point at the real caller site"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_real_self_field_method_owner() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &method_by_impl_self_query("SelfFieldAssocOwner", "call_self_field_instance_method"),
    )?;
    let target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let owner_context = call_context
        .get(&owner)
        .expect("self-field method owner should receive outgoing call context");
    assert_eq!(
        owner_context.len(),
        1,
        "self-field method owner context: {owner_context:#?}"
    );

    let call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "instance_value".to_string(),
                        receiver: Some(CallReceiverInfo::SelfField {
                            path: vec!["value".to_string()],
                        }),
                    }
        })
        .expect("self-field method call should be present");
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Method);

    Ok(())
}
