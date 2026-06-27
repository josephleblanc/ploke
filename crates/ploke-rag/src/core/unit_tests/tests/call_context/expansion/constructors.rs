use super::super::super::*;
#[tokio::test]
async fn call_context_expansion_adds_incoming_fixture_constructor_callers() -> Result<(), Error> {
    init_tracing_once();

    let tuple_db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let tuple_target = one_uuid(&tuple_db, &struct_in_module_query(&["crate"], "NewType"))?;
    let tuple_owner = one_uuid(
        &tuple_db,
        &function_in_module_query(&["crate"], "call_new_type_constructor"),
    )?;

    let mut tuple_rag = init_test_rag_mock(Arc::clone(&tuple_db));
    tuple_rag.cfg.call_context.max_owner_hits = 64;
    tuple_rag.cfg.call_context.max_caller_hits = 64;
    assert!(
        !tuple_rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable tuple-constructor caller expansion"
    );

    let tuple_hits = tuple_rag.expand_hits_with_call_context(&[(tuple_target, 1.0)])?;
    let tuple_ids = tuple_hits.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        tuple_ids.contains(&tuple_target),
        "incoming tuple-constructor expansion must preserve the seed target; expanded: {tuple_hits:#?}"
    );
    assert!(
        tuple_ids.contains(&tuple_owner),
        "tuple-struct constructor target expansion should materialize the caller owner; expanded: {tuple_hits:#?}"
    );

    let tuple_context = tuple_rag.collect_call_context(&tuple_hits)?;
    let tuple_owner_context = tuple_context
        .get(&tuple_owner)
        .expect("tuple constructor caller should receive outgoing call context");
    let tuple_call = tuple_owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["NewType".to_string()],
                    }
        })
        .expect("caller should preserve the NewType constructor edge");
    assert_eq!(tuple_call.status, CallStatusKind::Resolved);
    assert_eq!(tuple_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(tuple_call.targets.len(), 1);
    assert_eq!(tuple_call.targets[0].target_id, tuple_target);
    assert_eq!(
        tuple_call.targets[0].relation,
        CallTargetKind::TupleStructConstructor
    );

    let variant_db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_nodes",
    )?));
    let variant_target = one_uuid(
        &variant_db,
        &variant_by_enum_query("EnumWithData", "Variant1"),
    )?;
    let variant_owner = one_uuid(
        &variant_db,
        &function_in_module_query(&["crate", "imports"], "use_imported_items"),
    )?;

    let mut variant_rag = init_test_rag_mock(Arc::clone(&variant_db));
    variant_rag.cfg.call_context.max_owner_hits = 64;
    variant_rag.cfg.call_context.max_caller_hits = 64;
    assert!(
        !variant_rag.call_context_degraded(),
        "fresh fixture_nodes call_graph schema should enable enum-constructor caller expansion"
    );

    let variant_hits = variant_rag.expand_hits_with_call_context(&[(variant_target, 1.0)])?;
    let variant_ids = variant_hits.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        variant_ids.contains(&variant_target),
        "incoming enum-constructor expansion must preserve the seed target; expanded: {variant_hits:#?}"
    );
    assert!(
        variant_ids.contains(&variant_owner),
        "enum-variant constructor target expansion should materialize the caller owner; expanded: {variant_hits:#?}"
    );

    let variant_context = variant_rag.collect_call_context(&variant_hits)?;
    let variant_owner_context = variant_context
        .get(&variant_owner)
        .expect("enum constructor caller should receive outgoing call context");
    let variant_call = variant_owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["EnumWithData".to_string(), "Variant1".to_string()],
                    }
        })
        .expect("caller should preserve the EnumWithData::Variant1 constructor edge");
    assert_eq!(variant_call.status, CallStatusKind::Resolved);
    assert_eq!(
        variant_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(variant_call.targets.len(), 1);
    assert_eq!(variant_call.targets[0].target_id, variant_target);
    assert_eq!(
        variant_call.targets[0].relation,
        CallTargetKind::EnumVariantConstructor
    );

    Ok(())
}
