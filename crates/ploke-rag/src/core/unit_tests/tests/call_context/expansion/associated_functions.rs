use super::super::super::*;

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_expansion_adds_incoming_fixture_associated_function_callers()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &method_by_impl_self_query("LocalAssoc", "make"))?;
    let self_owner = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "call_self_make"),
    )?;
    let qualified_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_qualified_local_assoc_make"),
    )?;

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 64;
    rag.cfg.call_context.max_caller_hits = 64;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable associated-function caller expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&target),
        "incoming associated-function caller expansion must preserve the seed target; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&self_owner),
        "associated-function target expansion should materialize the method owner for Self::make; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&qualified_owner),
        "associated-function target expansion should materialize the qualified function owner; expanded: {expanded:#?}"
    );

    let call_context = rag.collect_call_context(&expanded)?;
    let self_context = call_context
        .get(&self_owner)
        .expect("Self::make caller method owner should receive outgoing call context");
    let self_call = self_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["Self".to_string(), "make".to_string()],
                    }
        })
        .expect("method owner should preserve the Self::make edge to LocalAssoc::make");
    assert_eq!(self_call.status, CallStatusKind::Resolved);
    assert_eq!(self_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(self_call.targets.len(), 1);
    assert_eq!(self_call.targets[0].target_id, target);
    assert_eq!(
        self_call.targets[0].relation,
        CallTargetKind::AssociatedFunction
    );

    let qualified_context = call_context
        .get(&qualified_owner)
        .expect("qualified associated-function caller should receive outgoing call context");
    let qualified_call = qualified_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["LocalAssoc".to_string(), "make".to_string()],
                    }
        })
        .expect("function owner should preserve the qualified edge to LocalAssoc::make");
    assert_eq!(qualified_call.status, CallStatusKind::Resolved);
    assert_eq!(
        qualified_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(qualified_call.targets.len(), 1);
    assert_eq!(qualified_call.targets[0].target_id, target);
    assert_eq!(
        qualified_call.targets[0].relation,
        CallTargetKind::AssociatedFunction
    );

    Ok(())
}
