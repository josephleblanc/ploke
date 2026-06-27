use super::super::super::*;
use super::super::helpers::assert_resolved_call;

struct Case {
    label: &'static str,
    owner: Uuid,
    targets: Vec<Uuid>,
}

#[tokio::test]
async fn proof_context_collection_preserves_projected_field_dynamic_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let dynamic_target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let tuple_target = one_uuid(
        &db,
        &struct_in_module_query(&["crate"], "TupleCallbackArrayHolder"),
    )?;
    let cases = vec![
        dynamic_case(
            &db,
            "named-field function",
            "call_named_field_function_binding",
            dynamic_target,
        )?,
        dynamic_case(
            &db,
            "aliased named-field function",
            "call_aliased_named_field_function_binding",
            dynamic_target,
        )?,
        dynamic_case(
            &db,
            "indexed named-field function",
            "call_indexed_named_field_function_binding",
            dynamic_target,
        )?,
        dynamic_case(
            &db,
            "indexed named-field array alias",
            "call_indexed_named_field_array_alias_binding",
            dynamic_target,
        )?,
        dynamic_case(
            &db,
            "aliased indexed named-field function",
            "call_aliased_indexed_named_field_function_binding",
            dynamic_target,
        )?,
        tuple_case(
            &db,
            "indexed tuple-field function",
            "call_indexed_tuple_field_function_binding",
            tuple_target,
            dynamic_target,
        )?,
        tuple_case(
            &db,
            "indexed tuple-field array alias",
            "call_indexed_tuple_field_array_alias_binding",
            tuple_target,
            dynamic_target,
        )?,
        tuple_case(
            &db,
            "aliased indexed tuple-field function",
            "call_aliased_indexed_tuple_field_function_binding",
            tuple_target,
            dynamic_target,
        )?,
    ];

    for case in &cases {
        let count = db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?;
        assert_eq!(
            count,
            case.targets.len() * 3,
            "{} should project resolved proof facts for every field-dynamic call row",
            case.label
        );
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected field-dynamic proof facts should enable RAG proof context"
    );

    for case in &cases {
        let proof_context = rag.collect_proof_context(&[(case.owner, 1.0)])?;
        let rows = proof_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} owner seed should receive proof rows", case.label));
        assert_eq!(
            rows.len(),
            case.targets.len() * 3,
            "{} proof rows: {rows:#?}",
            case.label
        );
        for target in &case.targets {
            assert_resolved_call(rows, case.owner, *target);
        }
    }

    Ok(())
}

fn dynamic_case(
    db: &Database,
    label: &'static str,
    owner: &'static str,
    target: Uuid,
) -> Result<Case, Error> {
    Ok(Case {
        label,
        owner: one_uuid(db, &function_in_module_query(&["crate"], owner))?,
        targets: vec![target],
    })
}

fn tuple_case(
    db: &Database,
    label: &'static str,
    owner: &'static str,
    tuple_target: Uuid,
    dynamic_target: Uuid,
) -> Result<Case, Error> {
    Ok(Case {
        label,
        owner: one_uuid(db, &function_in_module_query(&["crate"], owner))?,
        targets: vec![tuple_target, dynamic_target],
    })
}
