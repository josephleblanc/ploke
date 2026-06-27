use super::super::*;
use super::helpers::assert_resolved_call_for_domain;

struct Case {
    label: &'static str,
    fixture: &'static str,
    domain: &'static str,
    search_term: &'static str,
    call_id: &'static str,
    owner_relation: &'static str,
    owner: &'static str,
    target_module: &'static [&'static str],
    target: &'static str,
}

struct ResolvedCase {
    owner: Uuid,
    target: Uuid,
}

#[tokio::test]
async fn request_code_context_returns_initializer_proof_context() -> color_eyre::Result<()> {
    let cases = initializer_cases();

    for case in cases {
        let db = Arc::new(Database::new(setup_db_full_multi_embedding(case.fixture)?));
        let resolved = resolve_case(&db, &case)?;
        assert_eq!(
            db.project_call_proof_facts_for_owner(resolved.owner, case.domain)?,
            3,
            "{} proof fact count",
            case.label
        );

        let tool_result =
            execute_fixture_tool_request(&db, case.search_term, 5, case.call_id).await?;
        let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
        assert_result_ok(&result, case.search_term, 5, case.fixture);
        assert!(
            result
                .note
                .as_deref()
                .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
            "projected {} proof facts should avoid degraded proof-context note: {result:#?}",
            case.label
        );

        let owner_part = result
            .context
            .iter()
            .find(|part| part.id == resolved.owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} proof owner",
                    case.label
                )
            });
        assert_resolved_call_for_domain(
            &owner_part.proof_context,
            resolved.owner,
            resolved.target,
            case.domain,
        );
    }

    Ok(())
}

fn initializer_cases() -> [Case; 4] {
    [
        Case {
            label: "const initializer",
            fixture: "fixture_nodes",
            domain: "bd:fixture-nodes",
            search_term: "FN_CALL_CONST",
            call_id: "const_initializer_proof_context",
            owner_relation: "const",
            owner: "FN_CALL_CONST",
            target_module: &["crate", "const_static"],
            target: "five",
        },
        Case {
            label: "static initializer",
            fixture: "fixture_nodes",
            domain: "bd:fixture-nodes",
            search_term: "STATIC_FN_CALL",
            call_id: "static_initializer_proof_context",
            owner_relation: "static",
            owner: "STATIC_FN_CALL",
            target_module: &["crate", "const_static"],
            target: "five",
        },
        Case {
            label: "impl associated const initializer",
            fixture: "fixture_call_graph",
            domain: "bd:fixture-call-graph",
            search_term: "IMPL_ASSOC_VALUE",
            call_id: "impl_assoc_const_initializer_proof_context",
            owner_relation: "const",
            owner: "IMPL_ASSOC_VALUE",
            target_module: &["crate"],
            target: "assoc_const_value",
        },
        Case {
            label: "trait associated const initializer",
            fixture: "fixture_call_graph",
            domain: "bd:fixture-call-graph",
            search_term: "TRAIT_ASSOC_VALUE",
            call_id: "trait_assoc_const_initializer_proof_context",
            owner_relation: "const",
            owner: "TRAIT_ASSOC_VALUE",
            target_module: &["crate"],
            target: "assoc_const_value",
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
