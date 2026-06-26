use super::super::*;

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_expansion_adds_incoming_fixture_callers() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "try_local_assoc")?;
    let caller_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_try_result_instance_method"),
    )?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&target),
        "incoming caller expansion must preserve the seed target; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&caller_owner),
        "target-centered expansion should materialize the caller owner; expanded: {expanded:#?}"
    );

    let call_context = rag.collect_call_context(&expanded)?;
    let caller_context = call_context
        .get(&caller_owner)
        .expect("caller owner should receive outgoing call context");
    let path_call = caller_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["try_local_assoc".to_string()],
                    }
        })
        .expect("caller should preserve the call edge to try_local_assoc");
    assert_eq!(path_call.status, CallStatusKind::Resolved);
    assert_eq!(path_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(path_call.targets.len(), 1);
    assert_eq!(path_call.targets[0].target_id, target);
    assert_eq!(path_call.targets[0].relation, CallTargetKind::Function);

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_expansion_respects_max_caller_hits_by_score() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let high_target = unique_id_by_name(&db, "function", "try_local_assoc")?;
    let high_caller = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_try_result_instance_method"),
    )?;
    let low_target = one_uuid(&db, &method_by_impl_self_query("LocalAssoc", "make"))?;
    let low_self_caller = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "call_self_make"),
    )?;
    let low_qualified_caller = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_qualified_local_assoc_make"),
    )?;

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 2;
    rag.cfg.call_context.max_caller_hits = 1;
    rag.cfg.call_context.caller_factor = 0.5;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(high_target, 1.0), (low_target, 0.2)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert_eq!(
        expanded_ids.len(),
        3,
        "max_caller_hits=1 should add exactly one caller to the two seed hits: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&high_target) && expanded_ids.contains(&low_target),
        "incoming caller expansion must preserve seed hits: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&high_caller),
        "higher-scored target should contribute the sole caller hit: {expanded:#?}"
    );
    assert!(
        !expanded_ids.contains(&low_self_caller) && !expanded_ids.contains(&low_qualified_caller),
        "lower-scored associated-function callers should be truncated by max_caller_hits=1: {expanded:#?}"
    );

    let high_score = expanded
        .iter()
        .find(|(id, _)| *id == high_caller)
        .map(|(_, score)| *score)
        .expect("high caller should be present");
    assert_eq!(high_score, 0.5);

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_expansion_adds_incoming_fixture_dynamic_callers() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let dynamic_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_aliased_indexed_named_field_function_binding",
        ),
    )?;

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 128;
    rag.cfg.call_context.max_caller_hits = 1024;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable dynamic caller expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&target),
        "incoming dynamic caller expansion must preserve the seed target; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&dynamic_owner),
        "function target expansion should materialize a dynamic-function caller owner; expanded: {expanded:#?}"
    );

    let call_context = rag.collect_call_context(&expanded)?;
    let dynamic_context = call_context
        .get(&dynamic_owner)
        .expect("dynamic caller owner should receive outgoing call context");
    let dynamic_call = dynamic_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("dynamic caller should preserve the DynamicFunction edge to local_target");
    assert_eq!(dynamic_call.status, CallStatusKind::Resolved);
    assert_eq!(
        dynamic_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(dynamic_call.targets.len(), 1);
    assert_eq!(dynamic_call.targets[0].target_id, target);
    assert_eq!(
        dynamic_call.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_expansion_excludes_closure_async_outer_owners_for_local_target()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let dynamic_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_aliased_indexed_named_field_function_binding",
        ),
    )?;
    let forbidden_owners = [
        one_uuid(
            &db,
            &function_in_module_query(&["crate"], "closure_body_call_is_not_outer_call_site"),
        )?,
        one_uuid(
            &db,
            &function_in_module_query(&["crate"], "async_block_call_is_not_outer_call_site"),
        )?,
        one_uuid(
            &db,
            &function_in_module_query(&["crate"], "call_move_closure_literal_with_body_call"),
        )?,
        one_uuid(
            &db,
            &function_in_module_query(&["crate"], "call_async_closure_literal_with_body_call"),
        )?,
    ];

    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 128;
    rag.cfg.call_context.max_caller_hits = 1024;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable local_target caller expansion"
    );

    let expanded = rag.expand_hits_with_call_context(&[(target, 1.0)])?;
    let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    assert!(
        expanded_ids.contains(&target),
        "incoming local_target expansion must preserve the seed target; expanded: {expanded:#?}"
    );
    assert!(
        expanded_ids.contains(&dynamic_owner),
        "local_target expansion should still materialize real dynamic callers; expanded: {expanded:#?}"
    );
    assert!(
        forbidden_owners
            .iter()
            .all(|owner| !expanded_ids.contains(owner)),
        "local_target expansion leaked closure/async body outer owners: {expanded:#?}"
    );

    let call_context = rag.collect_call_context(&expanded)?;
    for owner in forbidden_owners {
        assert!(
            !call_context.contains_key(&owner),
            "closure/async outer owner received RAG call context after target expansion: {call_context:#?}"
        );
    }

    Ok(())
}

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
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
