use super::super::super::*;
#[tokio::test]
async fn call_context_expansion_adds_incoming_fixture_trait_dispatch_callers() -> Result<(), Error>
{
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &method_by_impl_trait_self_query(
            "LocalDispatchTrait",
            "TraitDispatchTarget",
            "trait_value",
        ),
    )?;
    let initialized_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_initialized_local_trait_method"),
    )?;
    let chained_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_reference_chain_trait_object_binding_method",
        ),
    )?;

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 64;
    rag.cfg.call_context.max_caller_hits = 64;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable trait-dispatch caller expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&target),
        "incoming trait-dispatch expansion must preserve the seed target; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&initialized_owner),
        "trait-dispatch target expansion should materialize the initialized local caller; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&chained_owner),
        "trait-dispatch target expansion should materialize the chained trait-object caller; expanded: {expanded:#?}"
    );

    let call_context = rag.collect_call_context(&expanded)?;
    for owner in [initialized_owner, chained_owner] {
        let context = call_context
            .get(&owner)
            .expect("trait-dispatch caller should receive outgoing call context");
        let call = context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Method
                    && call.callee
                        == CallCalleeInfo::Method {
                            name: "trait_value".to_string(),
                            receiver: Some(CallReceiverInfo::InitializedLocalBinding {
                                name: "value".to_string(),
                                init_path: vec!["TraitDispatchTarget".to_string()],
                            }),
                        }
            })
            .expect("caller should preserve the trait-dispatch edge to the seed target");
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Method);
    }

    Ok(())
}
