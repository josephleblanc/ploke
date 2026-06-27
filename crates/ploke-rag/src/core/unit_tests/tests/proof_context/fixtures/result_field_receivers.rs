use super::super::super::*;
use super::super::helpers::assert_resolved_call;

struct Case {
    label: &'static str,
    owner: Uuid,
    targets: Vec<Uuid>,
}

#[tokio::test]
async fn proof_context_collection_preserves_projected_result_field_receiver_rows()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let method_target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let clone_target = one_uuid(&db, &method_by_impl_self_query("LocalAssoc", "clone_assoc"))?;
    let make_target = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_local_assoc"),
    )?;
    let ready_target = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_ready_local_assoc"),
    )?;
    let tuple_target = one_uuid(
        &db,
        &struct_in_module_query(&["crate"], "TupleFieldMethodReceiver"),
    )?;
    let cases = vec![
        Case {
            label: "path-call result receiver",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_path_result_instance_method"),
            )?,
            targets: vec![make_target, method_target],
        },
        Case {
            label: "method-call result receiver",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_method_result_instance_method"),
            )?,
            targets: vec![clone_target, method_target],
        },
        Case {
            label: "await path-call result receiver",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_await_result_instance_method"),
            )?,
            targets: vec![ready_target, method_target],
        },
        Case {
            label: "tuple-field method receiver",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_tuple_field_instance_method"),
            )?,
            targets: vec![tuple_target, method_target],
        },
    ];

    for case in &cases {
        let count = db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?;
        assert_eq!(
            count, 6,
            "{} should project call_site, call_edge, and call_resolution facts for two calls",
            case.label
        );
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected result/field receiver proof facts should enable RAG proof context"
    );

    for case in &cases {
        let proof_context = rag.collect_proof_context(&[(case.owner, 1.0)])?;
        let rows = proof_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} owner seed should receive proof rows", case.label));
        assert_eq!(rows.len(), 6, "{} proof rows: {rows:#?}", case.label);
        for target in &case.targets {
            assert_resolved_call(rows, case.owner, *target);
        }
    }

    Ok(())
}
