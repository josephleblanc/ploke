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
async fn request_code_context_returns_field_dynamic_proof_context() -> color_eyre::Result<()> {
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
        assert_eq!(
            db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?,
            case.targets.len() * 3,
            "{} should project resolved proof facts for every field-dynamic call row",
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
            "projected field-dynamic proof facts should avoid degraded proof-context note: {result:#?}"
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
            owner_part.proof_context.len() >= case.targets.len() * 3,
            "{} proof context: {owner_part:#?}",
            case.label
        );
        for target in &case.targets {
            assert_resolved_call(&owner_part.proof_context, case.owner, *target);
        }
    }

    Ok(())
}

fn dynamic_case(
    db: &Database,
    label: &'static str,
    owner: &'static str,
    target: Uuid,
) -> color_eyre::Result<Case> {
    Ok(Case {
        label,
        search_term: owner,
        call_id: "field_dynamic_proof_context",
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
) -> color_eyre::Result<Case> {
    Ok(Case {
        label,
        search_term: owner,
        call_id: "field_dynamic_tuple_proof_context",
        owner: one_uuid(db, &function_in_module_query(&["crate"], owner))?,
        targets: vec![tuple_target, dynamic_target],
    })
}
