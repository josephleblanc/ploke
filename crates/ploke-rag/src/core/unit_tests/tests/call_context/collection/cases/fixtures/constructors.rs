use super::super::super::super::super::*;
use super::super::super::helpers::*;

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_collection_reads_real_fixture_constructor_rows() -> Result<(), Error> {
    init_tracing_once();

    let tuple_db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let tuple_owner = one_uuid(
        &tuple_db,
        &function_in_module_query(&["crate"], "call_new_type_constructor"),
    )?;
    let tuple_target = one_uuid(&tuple_db, &struct_in_module_query(&["crate"], "NewType"))?;
    let tuple_rag = init_test_rag_mock(Arc::clone(&tuple_db));
    assert!(
        !tuple_rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable tuple constructor call context"
    );

    let tuple_context = tuple_rag.collect_call_context(&[(tuple_owner, 1.0)])?;
    let tuple_owner_context = tuple_context
        .get(&tuple_owner)
        .expect("tuple constructor owner should receive outgoing call context");
    assert_eq!(
        tuple_owner_context.len(),
        1,
        "tuple constructor context: {tuple_owner_context:#?}"
    );
    let tuple_call = &tuple_owner_context[0];
    assert_eq!(tuple_call.kind, CallSiteKind::Path);
    assert_eq!(
        tuple_call.callee,
        CallCalleeInfo::Path {
            path: vec!["NewType".to_string()],
        }
    );
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
    let variant_owner = one_uuid(
        &variant_db,
        &function_in_module_query(&["crate", "imports"], "use_imported_items"),
    )?;
    let variant_target = one_uuid(
        &variant_db,
        &variant_by_enum_query("EnumWithData", "Variant1"),
    )?;
    let variant_rag = init_test_rag_mock(Arc::clone(&variant_db));
    assert!(
        !variant_rag.call_context_degraded(),
        "fresh fixture_nodes call_graph schema should enable enum variant call context"
    );

    let variant_context = variant_rag.collect_call_context(&[(variant_owner, 1.0)])?;
    let variant_owner_context = variant_context
        .get(&variant_owner)
        .expect("enum variant owner should receive outgoing call context");
    let variant_call = variant_owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["EnumWithData".to_string(), "Variant1".to_string()],
                    }
        })
        .expect("EnumWithData::Variant1 call should be present");
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
