use super::*;

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
