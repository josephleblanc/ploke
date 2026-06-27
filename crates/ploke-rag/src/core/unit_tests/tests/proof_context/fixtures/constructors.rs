use super::super::super::*;
use super::super::helpers::assert_resolved_call_for_domain;

struct Case {
    label: &'static str,
    fixture: &'static str,
    domain: &'static str,
    owner_module: &'static [&'static str],
    owner: &'static str,
    path: &'static [&'static str],
}

#[tokio::test]
async fn proof_context_collection_preserves_projected_constructor_rows() -> Result<(), Error> {
    init_tracing_once();
    let cases = [
        Case {
            label: "tuple-struct constructor",
            fixture: "fixture_call_graph",
            domain: "bd:fixture-call-graph",
            owner_module: &["crate"],
            owner: "call_new_type_constructor",
            path: &["NewType"],
        },
        Case {
            label: "enum-variant constructor",
            fixture: "fixture_nodes",
            domain: "bd:fixture-nodes",
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
        let count = db.project_call_proof_facts_for_target(target, case.domain)?;
        assert!(
            count >= 3,
            "{} should project at least one resolved constructor proof row set",
            case.label
        );

        let rag = init_test_rag_mock(Arc::clone(&db));
        assert!(
            !rag.proof_context_degraded(),
            "projected {} proof facts should enable RAG proof context",
            case.label
        );

        let proof_context = rag.collect_proof_context(&[(target, 1.0)])?;
        let rows = proof_context
            .get(&target)
            .unwrap_or_else(|| panic!("{} target seed should receive proof rows", case.label));
        assert!(
            rows.len() >= 3,
            "{} proof rows should include at least one constructor proof row set: {rows:#?}",
            case.label
        );
        assert_resolved_call_for_domain(rows, owner, target, case.domain);
    }

    Ok(())
}

fn projected_constructor_target(
    db: &Database,
    owner: Uuid,
    path: &[&str],
    label: &str,
) -> Result<Uuid, Error> {
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
