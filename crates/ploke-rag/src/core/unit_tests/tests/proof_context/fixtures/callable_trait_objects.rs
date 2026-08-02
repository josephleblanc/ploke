use super::super::super::*;
use super::super::helpers::assert_resolved_call;

struct Case {
    label: &'static str,
    owner: Uuid,
    expected_rows: usize,
}

#[tokio::test]
async fn proof_context_collection_preserves_projected_callable_trait_object_rows()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let cases = [
        callable_case(
            &db,
            "boxed dyn Fn value-binding path call",
            "call_boxed_dyn_fn_value_binding",
            5,
        )?,
        callable_case(
            &db,
            "parenthesized boxed dyn Fn value binding",
            "call_parenthesized_boxed_dyn_fn_value_binding",
            5,
        )?,
        callable_case(
            &db,
            "referenced dyn Fn value binding",
            "call_parenthesized_referenced_dyn_fn_value_binding",
            3,
        )?,
        callable_case(
            &db,
            "mut referenced dyn FnMut value binding",
            "call_parenthesized_mut_referenced_dyn_fnmut_value_binding",
            3,
        )?,
        callable_case(
            &db,
            "private referenced dyn Fn parameter",
            "call_single_parenthesized_referenced_dyn_fn_param",
            3,
        )?,
        callable_case(
            &db,
            "private boxed dyn Fn parameter",
            "call_single_boxed_dyn_fn_param",
            3,
        )?,
        callable_case(
            &db,
            "two-hop forwarded boxed dyn Fn parameter",
            "call_two_hop_forwarded_boxed_dyn_fn_leaf",
            3,
        )?,
    ];

    for case in &cases {
        let count = db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?;
        assert_eq!(count, case.expected_rows, "{} proof fact count", case.label);
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected callable trait-object proof facts should enable RAG proof context"
    );

    for case in &cases {
        let proof_context = rag.collect_proof_context(&[(case.owner, 1.0)])?;
        let rows = proof_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} owner seed should receive proof rows", case.label));
        assert_eq!(
            rows.len(),
            case.expected_rows,
            "{} proof rows: {rows:#?}",
            case.label
        );
        assert_resolved_call(rows, case.owner, target);
    }

    Ok(())
}

fn callable_case(
    db: &Database,
    label: &'static str,
    owner: &'static str,
    expected_rows: usize,
) -> Result<Case, Error> {
    Ok(Case {
        label,
        owner: one_uuid(db, &function_in_module_query(&["crate"], owner))?,
        expected_rows,
    })
}
