use super::super::assertions::{assert_expansion, assert_resolved_target, path};
use super::super::*;

#[tokio::test]
async fn request_code_context_returns_function_and_dynamic_owner_call_context()
-> color_eyre::Result<()> {
    struct Case<'a> {
        label: &'a str,
        search_term: &'a str,
        owner: &'a str,
        call_kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
    }

    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;

    let cases = [
        Case {
            label: "ordinary path caller",
            search_term: "call_crate_local_target",
            owner: "call_crate_local_target",
            call_kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: path(&["crate", "local_target"]),
            },
            relation: CallTargetKind::Function,
        },
        Case {
            label: "dynamic function caller",
            search_term: "call_parenthesized_local_target",
            owner: "call_parenthesized_local_target",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
        Case {
            label: "aliased indexed dynamic function caller",
            search_term: "call_aliased_indexed_named_field_function_binding",
            owner: "call_aliased_indexed_named_field_function_binding",
            call_kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
    ];

    for case in cases {
        let owner = one_uuid(&db, &function_in_module_query(&["crate"], case.owner))?;
        let result =
            execute_fixture_request(&db, case.search_term, 1, "local_target_call_context").await?;
        assert_result_ok(&result, case.search_term, 1, "fixture_call_graph");

        let target_part = result
            .context
            .iter()
            .find(|part| part.id == target)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the local_target outgoing callee for {}",
                    case.label
                )
            });
        let owner_part = result
            .context
            .iter()
            .find(|part| part.id == owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} owner",
                    case.label
                )
            });
        let call = owner_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == case.call_kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} owner should retain outgoing call context to local_target",
                    case.label
                )
            });
        assert_resolved_target(call, target, case.relation);
        assert_expansion(
            target_part,
            owner,
            target,
            call.site_id,
            CallExpansionKind::OutgoingTarget,
        );
    }

    Ok(())
}

#[tokio::test]
async fn request_code_context_returns_path_resolution_family_call_context() -> color_eyre::Result<()>
{
    struct Case {
        label: &'static str,
        search_term: &'static str,
        call_id: &'static str,
        owner: Uuid,
        target: Uuid,
        callee: CallCalleeInfo,
    }

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
            search_term: "call_self_nested_target",
            call_id: "path_family_self_nested",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate", "local_mod"], "call_self_nested_target"),
            )?,
            target: nested_target,
            callee: CallCalleeInfo::Path {
                path: path(&["self", "nested_target"]),
            },
        },
        Case {
            label: "imported alias path",
            search_term: "call_imported_alias_target",
            call_id: "path_family_imported_alias",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_imported_alias_target"),
            )?,
            target: imported_target,
            callee: CallCalleeInfo::Path {
                path: path(&["imported_alias"]),
            },
        },
        Case {
            label: "glob imported path",
            search_term: "call_glob_imported_target",
            call_id: "path_family_glob_import",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_glob_imported_target"),
            )?,
            target: globbed_target,
            callee: CallCalleeInfo::Path {
                path: path(&["globbed_target"]),
            },
        },
        Case {
            label: "re-exported path",
            search_term: "call_reexported_target",
            call_id: "path_family_reexport",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_reexported_target"),
            )?,
            target: imported_target,
            callee: CallCalleeInfo::Path {
                path: path(&["reexported_target"]),
            },
        },
        Case {
            label: "module alias path",
            search_term: "call_imported_module_target",
            call_id: "path_family_module_alias",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_imported_module_target"),
            )?,
            target: globbed_target,
            callee: CallCalleeInfo::Path {
                path: path(&["targets_alias", "globbed_target"]),
            },
        },
        Case {
            label: "grouped imported alias path",
            search_term: "call_grouped_imported_alias_target",
            call_id: "path_family_grouped_alias",
            owner: one_uuid(
                &db,
                &function_in_module_query(
                    &["crate", "grouped_function_import_scope"],
                    "call_grouped_imported_alias_target",
                ),
            )?,
            target: imported_target,
            callee: CallCalleeInfo::Path {
                path: path(&["grouped_alias"]),
            },
        },
    ];

    for case in cases {
        let result = execute_fixture_request(&db, case.search_term, 1, case.call_id).await?;
        assert_result_ok(&result, case.search_term, 1, "fixture_call_graph");

        let owner_part = result
            .context
            .iter()
            .find(|part| part.id == case.owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} owner",
                    case.label
                )
            });
        let target_part = result
            .context
            .iter()
            .find(|part| part.id == case.target)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} target",
                    case.label
                )
            });
        let call = owner_part
            .call_context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Path
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target_info| target_info.target_id == case.target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} owner should retain outgoing resolved path call context",
                    case.label
                )
            });
        assert_resolved_target(call, case.target, CallTargetKind::Function);
        assert_expansion(
            target_part,
            case.owner,
            case.target,
            call.site_id,
            CallExpansionKind::OutgoingTarget,
        );
    }

    Ok(())
}
