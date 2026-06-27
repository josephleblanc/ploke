use super::super::super::*;
use super::super::helpers::assert_resolved_call_for_domain;

struct Case {
    label: &'static str,
    fixture: &'static str,
    domain: &'static str,
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
async fn proof_context_collection_preserves_projected_initializer_rows() -> Result<(), Error> {
    init_tracing_once();
    let cases = initializer_cases();

    for case in cases {
        let db = Arc::new(Database::new(setup_db_full_multi_embedding(case.fixture)?));
        let resolved = resolve_case(&db, &case)?;
        let count = db.project_call_proof_facts_for_owner(resolved.owner, case.domain)?;
        assert_eq!(count, 3, "{} proof fact count", case.label);

        let rag = init_test_rag_mock(Arc::clone(&db));
        assert!(
            !rag.proof_context_degraded(),
            "projected {} proof facts should enable RAG proof context",
            case.label
        );

        let proof_context = rag.collect_proof_context(&[(resolved.owner, 1.0)])?;
        let rows = proof_context
            .get(&resolved.owner)
            .unwrap_or_else(|| panic!("{} owner seed should receive proof rows", case.label));
        assert_eq!(rows.len(), 3, "{} proof rows: {rows:#?}", case.label);
        assert_resolved_call_for_domain(rows, resolved.owner, resolved.target, case.domain);
    }

    Ok(())
}

fn initializer_cases() -> [Case; 4] {
    [
        Case {
            label: "const initializer",
            fixture: "fixture_nodes",
            domain: "bd:fixture-nodes",
            owner_relation: "const",
            owner: "FN_CALL_CONST",
            target_module: &["crate", "const_static"],
            target: "five",
        },
        Case {
            label: "static initializer",
            fixture: "fixture_nodes",
            domain: "bd:fixture-nodes",
            owner_relation: "static",
            owner: "STATIC_FN_CALL",
            target_module: &["crate", "const_static"],
            target: "five",
        },
        Case {
            label: "impl associated const initializer",
            fixture: "fixture_call_graph",
            domain: "bd:fixture-call-graph",
            owner_relation: "const",
            owner: "IMPL_ASSOC_VALUE",
            target_module: &["crate"],
            target: "assoc_const_value",
        },
        Case {
            label: "trait associated const initializer",
            fixture: "fixture_call_graph",
            domain: "bd:fixture-call-graph",
            owner_relation: "const",
            owner: "TRAIT_ASSOC_VALUE",
            target_module: &["crate"],
            target: "assoc_const_value",
        },
    ]
}

fn resolve_case(db: &Database, case: &Case) -> Result<ResolvedCase, Error> {
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
