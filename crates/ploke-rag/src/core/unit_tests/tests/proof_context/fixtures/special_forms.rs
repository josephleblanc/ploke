use super::super::super::*;
use super::super::helpers::assert_projected_owner_rows;

#[tokio::test]
async fn proof_context_collection_preserves_projected_special_form_rows() -> Result<(), Error> {
    struct Case {
        label: &'static str,
        owner: Uuid,
        target: Uuid,
    }

    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let cases = [
        Case {
            label: "generic function turbofish",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_generic_identity_turbofish"),
            )?,
            target: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "generic_identity"),
            )?,
        },
        Case {
            label: "generic method turbofish",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_method_turbofish"),
            )?,
            target: one_uuid(
                &db,
                &method_by_impl_self_query("GenericMethodTarget", "generic_instance"),
            )?,
        },
        Case {
            label: "unsafe local function",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_unsafe_function"),
            )?,
            target: one_uuid(&db, &function_in_module_query(&["crate"], "unsafe_target"))?,
        },
    ];

    for case in &cases {
        let count = db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?;
        assert_eq!(
            count, 3,
            "{} should project call_site, call_edge, and call_resolution facts",
            case.label
        );
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected special-form proof facts should enable RAG proof context"
    );

    for case in &cases {
        let proof_context = rag.collect_proof_context(&[(case.owner, 1.0)])?;
        let rows = proof_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} owner seed should receive proof rows", case.label));
        assert_projected_owner_rows(rows, case.owner, case.target);
    }

    Ok(())
}
