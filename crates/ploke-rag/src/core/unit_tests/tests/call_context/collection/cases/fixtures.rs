use super::super::super::super::*;
use super::super::helpers::*;

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_collection_reads_real_fixture_blocker_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let macro_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_scoped_macro"),
    )?;
    let ambiguous_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_ambiguous_trait_method"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable blocker call context"
    );

    let call_context = rag.collect_call_context(&[(macro_owner, 1.0), (ambiguous_owner, 1.0)])?;
    let macro_context = call_context
        .get(&macro_owner)
        .expect("macro owner should receive outgoing call context");
    assert_eq!(
        macro_context.len(),
        1,
        "macro owner context: {macro_context:#?}"
    );
    let macro_call = &macro_context[0];
    assert_eq!(macro_call.kind, CallSiteKind::Macro);
    assert_eq!(
        macro_call.callee,
        CallCalleeInfo::Macro {
            name: "crate::crate_scoped_macro".to_string(),
        }
    );
    assert_eq!(macro_call.status, CallStatusKind::Unsupported);
    assert!(macro_call.resolution.is_none());
    assert!(
        macro_call.targets.is_empty(),
        "macro blocker rows must not fabricate RAG targets: {macro_call:#?}"
    );

    let ambiguous_context = call_context
        .get(&ambiguous_owner)
        .expect("ambiguous owner should receive outgoing call context");
    assert_eq!(
        ambiguous_context.len(),
        1,
        "ambiguous owner context: {ambiguous_context:#?}"
    );
    let ambiguous_call = &ambiguous_context[0];
    assert_eq!(ambiguous_call.kind, CallSiteKind::Method);
    assert_eq!(
        ambiguous_call.callee,
        CallCalleeInfo::Method {
            name: "overlap".to_string(),
            receiver: Some(CallReceiverInfo::LocalBinding {
                name: "value".to_string(),
            }),
        }
    );
    assert_eq!(ambiguous_call.status, CallStatusKind::Ambiguous);
    assert!(ambiguous_call.resolution.is_none());
    assert!(
        ambiguous_call.targets.is_empty(),
        "ambiguous blocker rows must not fabricate RAG targets: {ambiguous_call:#?}"
    );

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_collection_reads_real_fixture_external_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let string_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_prelude_string_new"),
    )?;
    let literal_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_literal_str_to_string"),
    )?;
    let vec_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_typed_vec_len_external"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable external call context"
    );

    let call_context =
        rag.collect_call_context(&[(string_owner, 1.0), (literal_owner, 1.0), (vec_owner, 1.0)])?;

    let string_context = call_context
        .get(&string_owner)
        .expect("String::new owner should receive outgoing call context");
    assert_eq!(
        string_context.len(),
        1,
        "String::new owner context: {string_context:#?}"
    );
    let string_call = &string_context[0];
    assert_eq!(string_call.kind, CallSiteKind::Path);
    assert_eq!(
        string_call.callee,
        CallCalleeInfo::Path {
            path: vec!["String".to_string(), "new".to_string()],
        }
    );
    assert_eq!(string_call.status, CallStatusKind::External);
    assert!(string_call.resolution.is_none());
    assert!(
        string_call.targets.is_empty(),
        "external path calls must not fabricate RAG targets: {string_call:#?}"
    );

    let literal_context = call_context
        .get(&literal_owner)
        .expect("literal method owner should receive outgoing call context");
    assert_eq!(
        literal_context.len(),
        1,
        "literal method owner context: {literal_context:#?}"
    );
    let literal_call = &literal_context[0];
    assert_eq!(literal_call.kind, CallSiteKind::Method);
    assert_eq!(
        literal_call.callee,
        CallCalleeInfo::Method {
            name: "to_string".to_string(),
            receiver: Some(CallReceiverInfo::Literal),
        }
    );
    assert_eq!(literal_call.status, CallStatusKind::External);
    assert!(literal_call.resolution.is_none());
    assert!(
        literal_call.targets.is_empty(),
        "external literal method calls must not fabricate RAG targets: {literal_call:#?}"
    );

    let vec_context = call_context
        .get(&vec_owner)
        .expect("typed Vec owner should receive outgoing call context");
    assert_eq!(
        vec_context.len(),
        2,
        "typed Vec owner context: {vec_context:#?}"
    );
    let vec_new = vec_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["Vec".to_string(), "new".to_string()],
                    }
        })
        .expect("Vec::new path call should stay visible");
    assert_eq!(vec_new.status, CallStatusKind::External);
    assert!(vec_new.resolution.is_none());
    assert!(vec_new.targets.is_empty());

    let vec_len = vec_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "len".to_string(),
                        receiver: Some(CallReceiverInfo::TypedLocalBinding {
                            name: "value".to_string(),
                            type_path: vec!["Vec".to_string()],
                        }),
                    }
        })
        .expect("typed Vec::len method call should stay visible");
    assert_eq!(vec_len.status, CallStatusKind::External);
    assert!(vec_len.resolution.is_none());
    assert!(
        vec_len.targets.is_empty(),
        "external Vec::len calls must not fabricate RAG targets: {vec_len:#?}"
    );

    Ok(())
}

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_collection_reads_real_fixture_dynamic_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let resolved_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_aliased_indexed_named_field_function_binding",
        ),
    )?;
    let unsupported_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_dereferenced_closure_binding"),
    )?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable dynamic call context collection"
    );

    let call_context =
        rag.collect_call_context(&[(resolved_owner, 1.0), (unsupported_owner, 1.0)])?;
    let resolved_context = call_context
        .get(&resolved_owner)
        .expect("resolved dynamic owner should receive outgoing call context");
    let resolved_call = resolved_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("resolved dynamic function call should stay visible in RAG call context");
    assert_eq!(resolved_call.status, CallStatusKind::Resolved);
    assert_eq!(
        resolved_call.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(resolved_call.targets.len(), 1);
    assert_eq!(resolved_call.targets[0].target_id, target);
    assert_eq!(
        resolved_call.targets[0].relation,
        CallTargetKind::DynamicFunction
    );

    let unsupported_context = call_context
        .get(&unsupported_owner)
        .expect("unsupported dynamic owner should receive outgoing call context");
    assert_eq!(
        unsupported_context.len(),
        1,
        "unsupported dynamic owner context: {unsupported_context:#?}"
    );
    let unsupported_call = &unsupported_context[0];
    assert_eq!(unsupported_call.kind, CallSiteKind::Dynamic);
    assert_eq!(unsupported_call.callee, CallCalleeInfo::Dynamic);
    assert_eq!(unsupported_call.status, CallStatusKind::Unsupported);
    assert!(unsupported_call.resolution.is_none());
    assert!(
        unsupported_call.targets.is_empty(),
        "unsupported dynamic calls must not fabricate RAG targets: {unsupported_call:#?}"
    );

    Ok(())
}
