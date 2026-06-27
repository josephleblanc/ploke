use super::super::super::*;
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

#[tokio::test]
async fn call_context_expansion_preserves_resolved_path_resolution_family() -> Result<(), Error> {
    struct Case {
        label: &'static str,
        owner: Uuid,
        target: Uuid,
        callee: CallCalleeInfo,
    }

    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let nested_target = one_uuid(
        &db,
        &function_in_module_query(&["crate", "local_mod"], "nested_target"),
    )?;
    let imported_target = one_uuid(
        &db,
        &function_in_module_query(&["crate", "import_targets"], "imported_target"),
    )?;
    let globbed_target = one_uuid(
        &db,
        &function_in_module_query(&["crate", "import_targets"], "globbed_target"),
    )?;
    let cases = [
        Case {
            label: "self nested path",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate", "local_mod"], "call_self_nested_target"),
            )?,
            target: nested_target,
            callee: CallCalleeInfo::Path {
                path: vec!["self".to_string(), "nested_target".to_string()],
            },
        },
        Case {
            label: "imported alias path",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_imported_alias_target"),
            )?,
            target: imported_target,
            callee: CallCalleeInfo::Path {
                path: vec!["imported_alias".to_string()],
            },
        },
        Case {
            label: "glob imported path",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_glob_imported_target"),
            )?,
            target: globbed_target,
            callee: CallCalleeInfo::Path {
                path: vec!["globbed_target".to_string()],
            },
        },
        Case {
            label: "re-exported path",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_reexported_target"),
            )?,
            target: imported_target,
            callee: CallCalleeInfo::Path {
                path: vec!["reexported_target".to_string()],
            },
        },
        Case {
            label: "module alias path",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_imported_module_target"),
            )?,
            target: globbed_target,
            callee: CallCalleeInfo::Path {
                path: vec!["targets_alias".to_string(), "globbed_target".to_string()],
            },
        },
        Case {
            label: "grouped imported alias path",
            owner: one_uuid(
                &db,
                &function_in_module_query(
                    &["crate", "grouped_function_import_scope"],
                    "call_grouped_imported_alias_target",
                ),
            )?,
            target: imported_target,
            callee: CallCalleeInfo::Path {
                path: vec!["grouped_alias".to_string()],
            },
        },
    ];

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable path-family call-context expansion"
    );

    for case in &cases {
        let expanded = rag.expand_hits_with_call_context(&[(case.owner, 1.0)])?;
        let expanded_ids = expanded.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        assert!(
            expanded_ids.contains(&case.owner) && expanded_ids.contains(&case.target),
            "{} expansion should preserve owner and materialize callee target: {expanded:#?}",
            case.label
        );

        let call_context = rag.collect_call_context(&expanded)?;
        let owner_context = call_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} owner should receive call context", case.label));
        let call = owner_context
            .iter()
            .find(|call| call.kind == CallSiteKind::Path && call.callee == case.callee)
            .unwrap_or_else(|| {
                panic!(
                    "{} should preserve resolved path call context: {owner_context:#?}",
                    case.label
                )
            });
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, case.target);
        assert_eq!(call.targets[0].relation, CallTargetKind::Function);
    }

    Ok(())
}

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
