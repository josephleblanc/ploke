use super::*;

#[tokio::test]
async fn call_context_expansion_adds_outgoing_fixture_targets() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_local_target"),
    )?;
    let target = unique_id_by_name(&db, "function", "local_target")?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable outgoing target expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(owner, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&owner),
        "outgoing target expansion must preserve the seed owner; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&target),
        "owner-centered expansion should materialize the outgoing callee target; expanded: {expanded:#?}"
    );

    let target_score = expanded
        .iter()
        .find(|(id, _)| *id == target)
        .map(|(_, score)| *score)
        .expect("outgoing target should be present");
    assert_eq!(target_score, 0.5);

    let call_context = rag.collect_call_context(&expanded)?;
    let owner_context = call_context
        .get(&owner)
        .expect("seed owner should retain outgoing call context");
    let path_call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["crate".to_string(), "local_target".to_string()],
                    }
        })
        .expect("owner should preserve the call edge to local_target");
    assert_eq!(path_call.status, CallStatusKind::Resolved);
    assert_eq!(path_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(path_call.targets.len(), 1);
    assert_eq!(path_call.targets[0].target_id, target);
    assert_eq!(path_call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}
