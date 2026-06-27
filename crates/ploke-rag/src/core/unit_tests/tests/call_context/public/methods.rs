use super::super::super::*;
#[tokio::test]
async fn call_context_sparse_get_context_expands_method_target_hits_to_fixture_callers()
-> Result<(), Error> {
    struct Case {
        owner: Uuid,
        label: &'static str,
        callee: CallCalleeInfo,
    }

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
    let nested_ref_owner = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_typed_double_reference_local_instance_method",
        ),
    )?;
    let assoc_owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_method_as_associated_function"),
    )?;
    let self_field_owner = one_uuid(
        &db,
        &method_by_impl_self_query("SelfFieldAssocOwner", "call_self_field_instance_method"),
    )?;
    let method_cases = [
        Case {
            owner: method_owner,
            label: "method-call caller owner",
            callee: CallCalleeInfo::Method {
                name: "instance_value".to_string(),
                receiver: Some(CallReceiverInfo::TypedLocalBinding {
                    name: "value".to_string(),
                    type_path: vec!["LocalAssoc".to_string()],
                }),
            },
        },
        Case {
            owner: nested_ref_owner,
            label: "nested-reference method caller owner",
            callee: CallCalleeInfo::Method {
                name: "instance_value".to_string(),
                receiver: Some(CallReceiverInfo::TypedLocalBinding {
                    name: "value".to_string(),
                    type_path: vec!["LocalAssoc".to_string()],
                }),
            },
        },
        Case {
            owner: self_field_owner,
            label: "self-field method caller owner",
            callee: CallCalleeInfo::Method {
                name: "instance_value".to_string(),
                receiver: Some(CallReceiverInfo::SelfField {
                    path: vec!["value".to_string()],
                }),
            },
        },
    ];
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

    for case in &method_cases {
        let part = assembled
            .parts
            .iter()
            .find(|part| part.id == case.owner)
            .unwrap_or_else(|| panic!("public get_context should materialize the {}", case.label));
        let call = part
            .call_context
            .iter()
            .find(|call| call.kind == CallSiteKind::Method && call.callee == case.callee)
            .unwrap_or_else(|| {
                panic!(
                    "{} should retain outgoing method context to the seed target",
                    case.label
                )
            });
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Method);
        assert_incoming_expansion(part, call, target);
    }

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
