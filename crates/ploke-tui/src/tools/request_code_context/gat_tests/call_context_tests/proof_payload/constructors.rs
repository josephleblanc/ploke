use super::super::*;
use super::helpers::assert_resolved_call_for_domain;

struct Case {
    label: &'static str,
    fixture: &'static str,
    domain: &'static str,
    search_term: &'static str,
    top_k: usize,
    call_id: &'static str,
    owner_module: &'static [&'static str],
    owner: &'static str,
    path: &'static [&'static str],
}

#[tokio::test]
async fn request_code_context_returns_constructor_proof_context() -> color_eyre::Result<()> {
    let cases = [
        Case {
            label: "tuple-struct constructor",
            fixture: "fixture_call_graph",
            domain: "bd:fixture-call-graph",
            search_term: "pub struct NewType",
            top_k: 1,
            call_id: "tuple_constructor_proof_context",
            owner_module: &["crate"],
            owner: "call_new_type_constructor",
            path: &["NewType"],
        },
        Case {
            label: "enum-variant constructor",
            fixture: "fixture_nodes",
            domain: "bd:fixture-nodes",
            search_term: "Variant1",
            top_k: 10,
            call_id: "variant_constructor_proof_context",
            owner_module: &["crate", "imports"],
            owner: "use_imported_items",
            path: &["EnumWithData", "Variant1"],
        },
    ];

    for case in cases {
        let db = Arc::new(Database::new(setup_db_full_multi_embedding(case.fixture)?));
        let owner = one_uuid(
            &db,
            &function_in_module_query(case.owner_module, case.owner),
        )?;
        let target = projected_constructor_target(&db, owner, case.path, case.label)?;
        assert!(
            db.project_call_proof_facts_for_target(target, case.domain)? >= 3,
            "{} should project at least one resolved constructor proof row set",
            case.label
        );

        let tool_result =
            execute_fixture_tool_request(&db, case.search_term, case.top_k, case.call_id).await?;
        let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
        assert_result_ok(&result, case.search_term, case.top_k, case.fixture);
        assert!(
            result
                .note
                .as_deref()
                .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
            "projected {} proof facts should avoid degraded proof-context note: {result:#?}",
            case.label
        );

        let target_part = result
            .context
            .iter()
            .find(|part| part.id == target)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} proof target",
                    case.label
                )
            });
        assert_resolved_call_for_domain(&target_part.proof_context, owner, target, case.domain);
    }

    Ok(())
}

fn projected_constructor_target(
    db: &Database,
    owner: Uuid,
    path: &[&str],
    label: &str,
) -> color_eyre::Result<Uuid> {
    let expected_path = path
        .iter()
        .map(|segment| (*segment).to_string())
        .collect::<Vec<_>>();
    let context = db.call_context_for_owner(owner)?;
    let row = context
        .iter()
        .find(|row| row.site.path.as_ref() == Some(&expected_path))
        .unwrap_or_else(|| panic!("{label} call context missing constructor path: {context:#?}"));
    assert_eq!(
        row.status.status,
        ploke_db::call_graph::CallStatusKind::Resolved,
        "{label} constructor call should be resolved"
    );
    assert_eq!(
        row.targets.len(),
        1,
        "{label} constructor call should have one target: {row:#?}"
    );
    Ok(row.targets[0].target_id)
}
