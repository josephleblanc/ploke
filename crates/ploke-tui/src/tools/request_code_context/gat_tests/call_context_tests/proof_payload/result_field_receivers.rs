use super::super::*;
use super::helpers::assert_resolved_call;

struct Case {
    label: &'static str,
    search_term: &'static str,
    call_id: &'static str,
    owner: Uuid,
    targets: Vec<Uuid>,
}

#[tokio::test]
async fn request_code_context_returns_result_field_receiver_proof_context() -> color_eyre::Result<()>
{
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
            search_term: "call_path_result_instance_method",
            call_id: "path_result_receiver_proof_context",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_path_result_instance_method"),
            )?,
            targets: vec![make_target, method_target],
        },
        Case {
            label: "method-call result receiver",
            search_term: "call_method_result_instance_method",
            call_id: "method_result_receiver_proof_context",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_method_result_instance_method"),
            )?,
            targets: vec![clone_target, method_target],
        },
        Case {
            label: "await path-call result receiver",
            search_term: "call_await_result_instance_method",
            call_id: "await_result_receiver_proof_context",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_await_result_instance_method"),
            )?,
            targets: vec![ready_target, method_target],
        },
        Case {
            label: "tuple-field method receiver",
            search_term: "call_tuple_field_instance_method",
            call_id: "tuple_field_receiver_proof_context",
            owner: one_uuid(
                &db,
                &function_in_module_query(&["crate"], "call_tuple_field_instance_method"),
            )?,
            targets: vec![tuple_target, method_target],
        },
    ];

    for case in &cases {
        assert_eq!(
            db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?,
            6,
            "{} should project call_site, call_edge, and call_resolution facts for two calls",
            case.label
        );
    }

    for case in &cases {
        let tool_result =
            execute_fixture_tool_request(&db, case.search_term, 1, case.call_id).await?;
        let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
        assert_result_ok(&result, case.search_term, 1, "fixture_call_graph");
        assert!(
            result
                .note
                .as_deref()
                .is_none_or(|note| { !note.contains("Proof-context expansion is unavailable") }),
            "projected result/field proof facts should avoid degraded proof-context note: {result:#?}"
        );

        let owner_part = result
            .context
            .iter()
            .find(|part| part.id == case.owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} proof owner",
                    case.label
                )
            });
        assert!(
            owner_part.proof_context.len() >= 6,
            "{} proof context: {owner_part:#?}",
            case.label
        );
        for target in &case.targets {
            assert_resolved_call(&owner_part.proof_context, case.owner, *target);
        }
    }

    Ok(())
}
