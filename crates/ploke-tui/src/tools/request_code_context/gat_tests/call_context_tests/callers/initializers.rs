use super::super::assertions::{assert_resolved_target, path};
use super::super::*;

struct Case {
    label: &'static str,
    fixture: &'static str,
    search_term: &'static str,
    call_id: &'static str,
    owner_relation: &'static str,
    owner: &'static str,
    target_module: &'static [&'static str],
    target: &'static str,
    path: &'static [&'static str],
}

struct ResolvedCase {
    owner: Uuid,
    target: Uuid,
}

#[tokio::test]
async fn request_code_context_returns_initializer_call_context() -> color_eyre::Result<()> {
    let cases = initializer_cases();

    for case in cases {
        let db = Arc::new(Database::new(setup_db_full_multi_embedding(case.fixture)?));
        let resolved = resolve_case(&db, &case)?;
        let result = execute_fixture_request(&db, case.search_term, 5, case.call_id).await?;
        assert_result_ok(&result, case.search_term, 5, case.fixture);

        let owner_part = result
            .context
            .iter()
            .find(|part| part.id == resolved.owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} owner",
                    case.label
                )
            });
        let expected_path = path(case.path);
        let call = owner_part
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
                        .any(|target| target.target_id == resolved.target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} owner should retain outgoing initializer call context",
                    case.label
                )
            });
        assert_resolved_target(call, resolved.target, CallTargetKind::Function);
    }

    Ok(())
}

fn initializer_cases() -> [Case; 4] {
    [
        Case {
            label: "const initializer",
            fixture: "fixture_nodes",
            search_term: "FN_CALL_CONST",
            call_id: "const_initializer_call_context",
            owner_relation: "const",
            owner: "FN_CALL_CONST",
            target_module: &["crate", "const_static"],
            target: "five",
            path: &["five"],
        },
        Case {
            label: "static initializer",
            fixture: "fixture_nodes",
            search_term: "STATIC_FN_CALL",
            call_id: "static_initializer_call_context",
            owner_relation: "static",
            owner: "STATIC_FN_CALL",
            target_module: &["crate", "const_static"],
            target: "five",
            path: &["five"],
        },
        Case {
            label: "impl associated const initializer",
            fixture: "fixture_call_graph",
            search_term: "IMPL_ASSOC_VALUE",
            call_id: "impl_assoc_const_initializer_call_context",
            owner_relation: "const",
            owner: "IMPL_ASSOC_VALUE",
            target_module: &["crate"],
            target: "assoc_const_value",
            path: &["assoc_const_value"],
        },
        Case {
            label: "trait associated const initializer",
            fixture: "fixture_call_graph",
            search_term: "TRAIT_ASSOC_VALUE",
            call_id: "trait_assoc_const_initializer_call_context",
            owner_relation: "const",
            owner: "TRAIT_ASSOC_VALUE",
            target_module: &["crate"],
            target: "assoc_const_value",
            path: &["assoc_const_value"],
        },
    ]
}

fn resolve_case(db: &Database, case: &Case) -> color_eyre::Result<ResolvedCase> {
    Ok(ResolvedCase {
        owner: one_uuid(db, &item_by_name_query(case.owner_relation, case.owner))?,
        target: one_uuid(
            db,
            &function_in_module_query(case.target_module, case.target),
        )?,
    })
}

fn item_by_name_query(relation: &str, name: &str) -> String {
    format!(r#"?[id] := *{relation} {{ id, name: "{name}" @ 'NOW' }}"#)
}
