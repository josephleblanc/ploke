use super::super::super::super::super::*;
use super::super::super::helpers::*;

#[tokio::test]
async fn call_context_collection_reads_real_targetless_special_form_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let extern_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_extern_c_function"),
    )?;
    let chained_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_chained_returned_function"),
    )?;
    let chained_target = one_uuid(&db, &function_in_module_query(&["crate"], "make_unary_fn"))?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable targetless special-form call context"
    );

    let call_context = rag.collect_call_context(&[(extern_owner, 1.0), (chained_owner, 1.0)])?;

    let extern_context = call_context
        .get(&extern_owner)
        .expect("extern C owner should receive outgoing call context");
    assert_eq!(
        extern_context.len(),
        1,
        "extern C owner context: {extern_context:#?}"
    );
    let extern_call = &extern_context[0];
    assert_eq!(extern_call.kind, CallSiteKind::Path);
    assert_eq!(
        extern_call.callee,
        CallCalleeInfo::Path {
            path: vec!["abs".to_string()],
        }
    );
    assert_eq!(extern_call.status, CallStatusKind::External);
    assert!(extern_call.resolution.is_none());
    assert!(
        extern_call.targets.is_empty(),
        "extern C calls must not fabricate RAG targets: {extern_call:#?}"
    );

    let chained_context = call_context
        .get(&chained_owner)
        .expect("chained returned-function owner should receive outgoing call context");
    assert_eq!(
        chained_context.len(),
        2,
        "chained returned-function owner context: {chained_context:#?}"
    );
    let path_call = chained_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["make_unary_fn".to_string()],
                    }
        })
        .expect("inner make_unary_fn path call should stay visible");
    assert_eq!(path_call.status, CallStatusKind::Resolved);
    assert_eq!(path_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(path_call.targets.len(), 1);
    assert_eq!(path_call.targets[0].target_id, chained_target);
    assert_eq!(path_call.targets[0].relation, CallTargetKind::Function);

    let dynamic_call = chained_context
        .iter()
        .find(|call| call.kind == CallSiteKind::Dynamic)
        .expect("outer chained returned-function dynamic call should stay visible");
    assert_eq!(dynamic_call.callee, CallCalleeInfo::Dynamic);
    assert_eq!(dynamic_call.status, CallStatusKind::Unsupported);
    assert!(dynamic_call.resolution.is_none());
    assert!(
        dynamic_call.targets.is_empty(),
        "outer chained returned-function calls must not fabricate RAG targets: {dynamic_call:#?}"
    );

    Ok(())
}
