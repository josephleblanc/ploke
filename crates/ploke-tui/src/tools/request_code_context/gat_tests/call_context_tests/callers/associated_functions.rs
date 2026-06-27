use super::super::assertions::{
    assert_incoming_expansion, assert_resolved_target, path, trait_method_query,
};
use super::super::*;

#[tokio::test]
async fn request_code_context_returns_associated_function_target_callers_with_call_context()
-> color_eyre::Result<()> {
    struct Case<'a> {
        label: &'a str,
        search_term: &'a str,
        call_id: &'static str,
        target_trait: &'a str,
        method: &'a str,
        callers: &'a [Caller<'a>],
    }

    struct Caller<'a> {
        module: &'a [&'a str],
        owner: &'a str,
        path: &'a [&'a str],
    }

    let cases = [
        Case {
            label: "local trait associated-function",
            search_term: "233 trait_make",
            call_id: "associated_function_call_context",
            target_trait: "LocalAssocFunctionTrait",
            method: "trait_make",
            callers: &[Caller {
                module: &["crate"],
                owner: "call_trait_associated_function",
                path: &["LocalAssocFunctionTrait", "trait_make"],
            }],
        },
        Case {
            label: "imported trait associated-function",
            search_term: "987 imported_trait_make",
            call_id: "imported_associated_function_call_context",
            target_trait: "ImportedAssocFunctionTrait",
            method: "imported_trait_make",
            callers: &[
                Caller {
                    module: &["crate", "trait_assoc_function_scope", "with_direct_import"],
                    owner: "call_direct_imported_trait_associated_function",
                    path: &["ImportedAssocFunctionTrait", "imported_trait_make"],
                },
                Caller {
                    module: &["crate", "trait_assoc_function_scope", "with_alias_import"],
                    owner: "call_alias_imported_trait_associated_function",
                    path: &["VisibleAssocFunctionTrait", "imported_trait_make"],
                },
                Caller {
                    module: &["crate", "trait_assoc_function_scope", "with_glob_import"],
                    owner: "call_glob_imported_trait_associated_function",
                    path: &["ImportedAssocFunctionTrait", "imported_trait_make"],
                },
                Caller {
                    module: &["crate", "trait_assoc_reexport_scope"],
                    owner: "call_reexported_trait_associated_function",
                    path: &["ReexportedAssocFunctionTrait", "imported_trait_make"],
                },
                Caller {
                    module: &["crate", "grouped_trait_assoc_function_scope"],
                    owner: "call_grouped_imported_trait_associated_function",
                    path: &["GroupedAssocFunctionTrait", "imported_trait_make"],
                },
            ],
        },
    ];

    for case in cases {
        let db = Arc::new(Database::new(setup_db_full_multi_embedding(
            "fixture_call_graph",
        )?));
        let target = one_uuid(&db, &trait_method_query(case.target_trait, case.method))?;

        let result = execute_fixture_request(&db, case.search_term, 1, case.call_id).await?;
        assert_result_ok(&result, case.search_term, 1, "fixture_call_graph");

        for caller in case.callers {
            let owner = one_uuid(&db, &function_in_module_query(caller.module, caller.owner))?;
            assert_associated_caller(&result, owner, target, caller.path, case.label);
        }
    }

    Ok(())
}

fn assert_associated_caller(
    result: &RequestCodeContextResult,
    owner: Uuid,
    target: Uuid,
    call_path: &[&str],
    label: &str,
) {
    let caller_part = result
        .context
        .iter()
        .find(|part| part.id == owner)
        .unwrap_or_else(|| {
            panic!("request_code_context should materialize the {label} caller owner")
        });
    let expected_path = path(call_path);
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
            panic!("{label} caller should retain outgoing context to the seed target")
        });
    assert_resolved_target(call, target, CallTargetKind::AssociatedFunction);
    assert_incoming_expansion(caller_part, call, target);
}
