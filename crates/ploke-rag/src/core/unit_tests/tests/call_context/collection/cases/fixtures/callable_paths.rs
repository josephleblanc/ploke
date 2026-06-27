use super::super::super::super::super::*;
use super::super::super::helpers::*;
#[tokio::test]
async fn call_context_collection_reads_real_fixture_callable_path_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let make_fn = unique_id_by_name(&db, "function", "make_fn")?;
    let returned_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_returned_function"),
    )?;
    let fn_param_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_function_pointer_param"),
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
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable callable path call context"
    );

    let call_context = rag.collect_call_context(&[
        (returned_owner, 1.0),
        (fn_param_owner, 1.0),
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
        .find(|call| call.kind == CallSiteKind::Dynamic)
        .expect("outer returned-function dynamic call should stay visible");
    assert_eq!(returned_dynamic.callee, CallCalleeInfo::Dynamic);
    assert_eq!(returned_dynamic.status, CallStatusKind::Unsupported);
    assert!(returned_dynamic.resolution.is_none());
    assert!(
        returned_dynamic.targets.is_empty(),
        "returned-function dynamic calls must not fabricate RAG targets: {returned_dynamic:#?}"
    );

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
    assert_eq!(boxed_call.status, CallStatusKind::Unsupported);
    assert!(boxed_call.resolution.is_none());
    assert!(
        boxed_call.targets.is_empty(),
        "boxed dyn Fn path calls must not fabricate RAG targets: {boxed_call:#?}"
    );

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
