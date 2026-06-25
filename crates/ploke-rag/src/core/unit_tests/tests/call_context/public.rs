use super::super::*;

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_sparse_get_context_expands_constructor_target_hits_to_fixture_callers()
-> Result<(), Error> {
    init_tracing_once();

    struct Case<'a> {
        label: &'a str,
        fixture: &'a str,
        query: &'a str,
        top_k: usize,
        target: ConstructorTarget<'a>,
        owner_module: &'a [&'a str],
        owner: &'a str,
        path: &'a [&'a str],
        relation: CallTargetKind,
    }

    enum ConstructorTarget<'a> {
        Struct {
            module: &'a [&'a str],
            name: &'a str,
        },
        Variant {
            enum_name: &'a str,
            name: &'a str,
        },
    }

    let cases = [
        Case {
            label: "tuple-struct constructor",
            fixture: "fixture_call_graph",
            query: "pub struct NewType",
            top_k: 1,
            target: ConstructorTarget::Struct {
                module: &["crate"],
                name: "NewType",
            },
            owner_module: &["crate"],
            owner: "call_new_type_constructor",
            path: &["NewType"],
            relation: CallTargetKind::TupleStructConstructor,
        },
        Case {
            label: "enum-variant constructor",
            fixture: "fixture_nodes",
            query: "Variant1",
            top_k: 10,
            target: ConstructorTarget::Variant {
                enum_name: "EnumWithData",
                name: "Variant1",
            },
            owner_module: &["crate", "imports"],
            owner: "use_imported_items",
            path: &["EnumWithData", "Variant1"],
            relation: CallTargetKind::EnumVariantConstructor,
        },
    ];

    for case in cases {
        let db = Arc::new(Database::new(setup_db_full_multi_embedding(case.fixture)?));
        let target = match case.target {
            ConstructorTarget::Struct { module, name } => {
                one_uuid(&db, &struct_in_module_query(module, name))?
            }
            ConstructorTarget::Variant { enum_name, name } => {
                one_uuid(&db, &variant_by_enum_query(enum_name, name))?
            }
        };
        let owner = one_uuid(
            &db,
            &function_in_module_query(case.owner_module, case.owner),
        )?;

        let mut cfg = crate::RagConfig::default();
        cfg.type_context.enabled = false;
        cfg.call_context.max_owner_hits = 64;
        cfg.call_context.max_caller_hits = 64;
        let rag = RagService::new_full(
            Arc::clone(&db),
            runtime_for(&db, EmbeddingProcessor::new_mock()),
            IoManagerHandle::new(),
            cfg,
        )?;
        assert!(
            !rag.call_context_degraded(),
            "fresh {} call_graph schema should enable public {} expansion",
            case.fixture,
            case.label
        );

        rag.bm25_rebuild().await?;
        let sparse_hits = rag
            .search_bm25_strict(case.query, case.top_k, LOADED_WORKSPACE_SCOPE)
            .await?;
        assert!(
            sparse_hits.iter().any(|(hit, _)| *hit == target),
            "{} query should seed get_context with the constructor target; hits: {sparse_hits:#?}",
            case.label
        );

        let assembled = rag
            .get_context(
                case.query,
                case.top_k,
                &TokenBudget {
                    max_total: 20_000,
                    per_file_max: 20_000,
                    per_part_max: 4_096,
                },
                &RetrievalStrategy::Sparse { strict: Some(true) },
                LOADED_WORKSPACE_SCOPE,
            )
            .await?;

        assert!(
            assembled.parts.iter().any(|part| part.id == target),
            "public get_context should materialize the {} target seed",
            case.label
        );

        let caller_part = assembled
            .parts
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| {
                panic!(
                    "public get_context should materialize the {} caller owner",
                    case.label
                )
            });
        let expected_path = case
            .path
            .iter()
            .map(|segment| (*segment).to_string())
            .collect::<Vec<_>>();
        let call = caller_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Path
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: expected_path.clone(),
                        }
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "caller part should retain outgoing {} context to the seed target",
                    case.label
                )
            });
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, case.relation);
        assert_incoming_expansion(caller_part, call, target);
    }

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_sparse_get_context_expands_owner_hits_to_fixture_callees() -> Result<(), Error>
{
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_local_target"),
    )?;
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let query = "call_crate_local_target";
    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_caller_hits = 64;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable public outgoing call-context expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with the caller owner only"
    );
    assert_eq!(
        sparse_hits[0].0, owner,
        "test query should seed get_context with the caller owner only"
    );

    let assembled = rag
        .get_context(
            query,
            1,
            &TokenBudget {
                max_total: 4096,
                per_file_max: 4096,
                per_part_max: 1024,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    let caller_part = assembled
        .parts
        .iter()
        .find(|part| part.id == owner)
        .expect("public get_context should preserve the caller owner seed");
    let call = caller_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["crate".to_string(), "local_target".to_string()],
                    }
        })
        .expect("caller part should retain outgoing call context to the callee target");
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);

    let target_part = assembled
        .parts
        .iter()
        .find(|part| part.id == target)
        .expect("public get_context should materialize the outgoing callee target");
    assert_outgoing_expansion(target_part, call, owner, target);

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_sparse_get_context_expands_target_hits_to_fixture_callers()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "try_local_assoc")?;
    let caller_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_try_result_instance_method"),
    )?;
    let query = "try_local_assoc";
    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_caller_hits = 64;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable public call-context expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with the callee target only"
    );
    assert_eq!(
        sparse_hits[0].0, target,
        "test query should seed get_context with the callee target only"
    );

    let assembled = rag
        .get_context(
            query,
            1,
            &TokenBudget {
                max_total: 4096,
                per_file_max: 4096,
                per_part_max: 1024,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    let caller_part = assembled
        .parts
        .iter()
        .find(|part| part.id == caller_owner)
        .expect("public get_context should materialize the incoming caller owner");
    let call = caller_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["try_local_assoc".to_string()],
                    }
        })
        .expect("caller part should retain outgoing call context to the seed target");
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);
    assert_incoming_expansion(caller_part, call, target);

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_sparse_get_context_excludes_closure_async_outer_owners_for_local_target()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "local_target")?;
    let path_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_crate_local_target"),
    )?;
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
    let query = "pub fn local_target";
    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_owner_hits = 64;
    cfg.call_context.max_caller_hits = 1024;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable public local_target expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with local_target only"
    );
    assert_eq!(
        sparse_hits[0].0, target,
        "test query should seed get_context with local_target only; hits: {sparse_hits:#?}"
    );

    let assembled = rag
        .get_context(
            query,
            1,
            &TokenBudget {
                max_total: 65_536,
                per_file_max: 65_536,
                per_part_max: 4096,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    let path_part = assembled
        .parts
        .iter()
        .find(|part| part.id == path_owner)
        .expect("public get_context should materialize the ordinary local_target path caller");
    let path_call = path_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["crate".to_string(), "local_target".to_string()],
                    }
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("path caller part should retain outgoing call context to local_target");
    assert_eq!(path_call.status, CallStatusKind::Resolved);
    assert_eq!(path_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(path_call.targets.len(), 1);
    assert_eq!(path_call.targets[0].target_id, target);
    assert_eq!(path_call.targets[0].relation, CallTargetKind::Function);
    assert_incoming_expansion(path_part, path_call, target);

    let dynamic_part = assembled
        .parts
        .iter()
        .find(|part| part.id == dynamic_owner)
        .expect("public get_context should materialize a real dynamic local_target caller");
    let dynamic_call = dynamic_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target_info| target_info.target_id == target)
        })
        .expect("dynamic caller part should retain outgoing call context to local_target");
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
    assert_incoming_expansion(dynamic_part, dynamic_call, target);

    for forbidden in forbidden_owners {
        assert!(
            assembled.parts.iter().all(|part| part.id != forbidden),
            "public local_target expansion leaked closure/async outer owner {forbidden}: {:#?}",
            assembled.parts
        );
    }

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_sparse_get_context_expands_method_target_hits_to_fixture_callers()
-> Result<(), Error> {
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
    let query = "instance_value";
    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_owner_hits = 64;
    cfg.call_context.max_caller_hits = 64;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable public method call-context expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with the method target only"
    );
    assert_eq!(
        sparse_hits[0].0, target,
        "test query should seed get_context with the method target only"
    );

    let assembled = rag
        .get_context(
            query,
            1,
            &TokenBudget {
                max_total: 4096,
                per_file_max: 4096,
                per_part_max: 1024,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    let method_part = assembled
        .parts
        .iter()
        .find(|part| part.id == method_owner)
        .expect("public get_context should materialize the method-call caller owner");
    let method_call = method_part
        .call_context
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
        .expect("caller part should retain outgoing method call context to the seed target");
    assert_eq!(method_call.status, CallStatusKind::Resolved);
    assert_eq!(method_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(method_call.targets.len(), 1);
    assert_eq!(method_call.targets[0].target_id, target);
    assert_eq!(method_call.targets[0].relation, CallTargetKind::Method);
    assert_incoming_expansion(method_part, method_call, target);

    let assoc_part = assembled
        .parts
        .iter()
        .find(|part| part.id == assoc_owner)
        .expect("public get_context should materialize the associated-function caller owner");
    let assoc_call = assoc_part
        .call_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["LocalAssoc".to_string(), "instance_value".to_string()],
                    }
        })
        .expect(
            "caller part should retain outgoing associated-function context to the seed target",
        );
    assert_eq!(assoc_call.status, CallStatusKind::Resolved);
    assert_eq!(assoc_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(assoc_call.targets.len(), 1);
    assert_eq!(assoc_call.targets[0].target_id, target);
    assert_eq!(
        assoc_call.targets[0].relation,
        CallTargetKind::AssociatedFunction
    );
    assert_incoming_expansion(assoc_part, assoc_call, target);

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_sparse_get_context_expands_associated_function_target_hits_to_fixture_callers()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &trait_method_query("LocalAssocFunctionTrait", "trait_make"),
    )?;
    let caller_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_trait_associated_function"),
    )?;
    let query = "233 trait_make";
    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_owner_hits = 64;
    cfg.call_context.max_caller_hits = 64;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable public associated-function call-context expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with the associated-function target only"
    );
    assert_eq!(
        sparse_hits[0].0, target,
        "test query should seed get_context with the LocalAssocFunctionTrait::trait_make target only; hits: {sparse_hits:#?}"
    );

    let assembled = rag
        .get_context(
            query,
            1,
            &TokenBudget {
                max_total: 20_000,
                per_file_max: 20_000,
                per_part_max: 4_096,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    let caller_part = assembled
        .parts
        .iter()
        .find(|part| part.id == caller_owner)
        .expect("public get_context should materialize the trait associated-function caller owner");
    let call = caller_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Path
                    && call.callee
                        == CallCalleeInfo::Path {
                            path: vec![
                                "LocalAssocFunctionTrait".to_string(),
                                "trait_make".to_string(),
                            ],
                        }
            })
            .expect("caller part should retain outgoing trait associated-function context to the seed target");
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::AssociatedFunction);
    assert_incoming_expansion(caller_part, call, target);

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_sparse_get_context_expands_imported_trait_associated_function_target_hits_to_fixture_callers()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(
        &db,
        &trait_method_query("ImportedAssocFunctionTrait", "imported_trait_make"),
    )?;
    let callers = [
        (
            &["crate", "trait_assoc_function_scope", "with_direct_import"][..],
            "call_direct_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_alias_import"][..],
            "call_alias_imported_trait_associated_function",
            &["VisibleAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_glob_import"][..],
            "call_glob_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_reexport_scope"][..],
            "call_reexported_trait_associated_function",
            &["ReexportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "grouped_trait_assoc_function_scope"][..],
            "call_grouped_imported_trait_associated_function",
            &["GroupedAssocFunctionTrait", "imported_trait_make"][..],
        ),
    ];
    let caller_owners = callers
        .iter()
        .map(|(module_path, name, expected_path)| {
            Ok((
                one_uuid(&db, &function_in_module_query(module_path, name))?,
                *expected_path,
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let query = "987 imported_trait_make";
    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_owner_hits = 64;
    cfg.call_context.max_caller_hits = 64;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable public imported trait associated-function call-context expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 1, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert_eq!(
        sparse_hits.len(),
        1,
        "test query should seed get_context with the imported trait associated-function target only"
    );
    assert_eq!(
        sparse_hits[0].0, target,
        "test query should seed get_context with the ImportedAssocFunctionTrait::imported_trait_make target only; hits: {sparse_hits:#?}"
    );

    let assembled = rag
        .get_context(
            query,
            1,
            &TokenBudget {
                max_total: 24_000,
                per_file_max: 24_000,
                per_part_max: 4_096,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    for (caller_owner, expected_path) in caller_owners {
        let caller_part = assembled
                .parts
                .iter()
                .find(|part| part.id == caller_owner)
                .expect(
                    "public get_context should materialize the imported trait associated-function caller owner",
                );
        let call = caller_part
                .call_context
                .iter()
                .find(|call| {
                    call.kind == CallSiteKind::Path
                        && call.callee
                            == CallCalleeInfo::Path {
                                path: expected_path
                                    .iter()
                                    .map(|segment| (*segment).to_string())
                                    .collect(),
                            }
                })
                .expect("caller part should retain outgoing imported trait associated-function context to the seed target");
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::AssociatedFunction);
        assert_incoming_expansion(caller_part, call, target);
    }

    Ok(())
}

#[cfg(feature = "call_graph")]
#[tokio::test]
async fn call_context_sparse_get_context_expands_trait_dispatch_target_hits_to_fixture_callers()
-> Result<(), Error> {
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
    let query = "144 trait_value";
    let mut cfg = crate::RagConfig::default();
    cfg.type_context.enabled = false;
    cfg.call_context.max_owner_hits = 64;
    cfg.call_context.max_caller_hits = 64;
    let rag = RagService::new_full(
        Arc::clone(&db),
        runtime_for(&db, EmbeddingProcessor::new_mock()),
        IoManagerHandle::new(),
        cfg,
    )?;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable public trait-dispatch call-context expansion"
    );

    rag.bm25_rebuild().await?;
    let sparse_hits = rag
        .search_bm25_strict(query, 5, LOADED_WORKSPACE_SCOPE)
        .await?;
    assert!(
        sparse_hits.iter().any(|(id, _)| *id == target),
        "test query should seed get_context with the concrete trait-dispatch method target; hits: {sparse_hits:#?}"
    );

    let assembled = rag
        .get_context(
            query,
            5,
            &TokenBudget {
                max_total: 20_000,
                per_file_max: 20_000,
                per_part_max: 4_096,
            },
            &RetrievalStrategy::Sparse { strict: Some(true) },
            LOADED_WORKSPACE_SCOPE,
        )
        .await?;

    for owner in [initialized_owner, chained_owner] {
        let caller_part = assembled
            .parts
            .iter()
            .find(|part| part.id == owner)
            .expect("public get_context should materialize the trait-dispatch caller owner");
        let call = caller_part
            .call_context
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
            .expect("caller part should retain outgoing trait-dispatch context to the seed target");
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Method);
        assert_incoming_expansion(caller_part, call, target);
    }

    Ok(())
}
