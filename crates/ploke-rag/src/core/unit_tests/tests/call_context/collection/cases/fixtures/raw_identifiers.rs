use super::super::super::super::super::*;
use super::super::super::helpers::*;

#[tokio::test]
async fn call_context_collection_reads_real_raw_identifier_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let raw_function_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_raw_identifier_function"),
    )?;
    let raw_method_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_raw_identifier_method"),
    )?;
    let raw_function_target = single_target_for_owner(&db, raw_function_owner)?;
    let raw_method_target = single_target_for_owner(&db, raw_method_owner)?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable raw identifier call context"
    );

    let call_context =
        rag.collect_call_context(&[(raw_function_owner, 1.0), (raw_method_owner, 1.0)])?;

    let function_context = call_context
        .get(&raw_function_owner)
        .expect("raw identifier function owner should receive outgoing call context");
    assert_eq!(
        function_context.len(),
        1,
        "raw identifier function context: {function_context:#?}"
    );
    let function_call = &function_context[0];
    assert_eq!(function_call.kind, CallSiteKind::Path);
    assert_eq!(
        function_call.callee,
        CallCalleeInfo::Path {
            path: vec!["r#match".to_string()],
        }
    );
    assert_eq!(function_call.status, CallStatusKind::Resolved);
    assert_eq!(
        function_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(function_call.targets.len(), 1);
    assert_eq!(function_call.targets[0].target_id, raw_function_target);
    assert_eq!(function_call.targets[0].relation, CallTargetKind::Function);

    let method_context = call_context
        .get(&raw_method_owner)
        .expect("raw identifier method owner should receive outgoing call context");
    assert_eq!(
        method_context.len(),
        1,
        "raw identifier method context: {method_context:#?}"
    );
    let method_call = &method_context[0];
    assert_eq!(method_call.kind, CallSiteKind::Method);
    assert_eq!(
        method_call.callee,
        CallCalleeInfo::Method {
            name: "r#type".to_string(),
            receiver: Some(CallReceiverInfo::TypedLocalBinding {
                name: "value".to_string(),
                type_path: vec!["RawMethodTarget".to_string()],
            }),
        }
    );
    assert_eq!(method_call.status, CallStatusKind::Resolved);
    assert_eq!(method_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(method_call.targets.len(), 1);
    assert_eq!(method_call.targets[0].target_id, raw_method_target);
    assert_eq!(method_call.targets[0].relation, CallTargetKind::Method);

    Ok(())
}

fn single_target_for_owner(db: &Database, owner: Uuid) -> Result<Uuid, Error> {
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "raw identifier owner should have one DB context row: {context:#?}"
    );
    assert_eq!(
        context[0].targets.len(),
        1,
        "raw identifier owner should have one DB target: {context:#?}"
    );
    Ok(context[0].targets[0].target_id)
}
