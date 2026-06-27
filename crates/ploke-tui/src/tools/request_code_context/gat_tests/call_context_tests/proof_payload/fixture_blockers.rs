use super::super::*;

struct Case {
    label: &'static str,
    owner: &'static str,
    expected_rows: usize,
    reasons: &'static [&'static str],
}

#[tokio::test]
async fn request_code_context_returns_fixture_blocker_proof_context() -> color_eyre::Result<()> {
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let cases = blocker_cases()
        .into_iter()
        .map(|case| resolve_case(&db, case))
        .collect::<color_eyre::Result<Vec<_>>>()?;

    for case in &cases {
        assert_eq!(
            db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?,
            case.expected_rows,
            "{} proof fact count",
            case.label
        );
    }

    for case in &cases {
        let tool_result =
            execute_fixture_tool_request(&db, case.search_term, 5, case.call_id).await?;
        let result: RequestCodeContextResult = serde_json::from_str(&tool_result.content)?;
        assert_result_ok(&result, case.search_term, 5, "fixture_call_graph");
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
            .find(|part| part.id == case.owner)
            .unwrap_or_else(|| {
                panic!(
                    "request_code_context should materialize the {} proof owner",
                    case.label
                )
            });
        assert_eq!(
            owner_part.proof_context.len(),
            case.expected_rows,
            "{} proof context: {owner_part:#?}",
            case.label
        );
        for reason in case.reasons {
            assert_owner_blocker_reason(&owner_part.proof_context, case.owner, reason);
        }
        assert_proof_blockers(&tool_result, &result);
    }

    Ok(())
}

fn blocker_cases() -> [Case; 6] {
    [
        Case {
            label: "String::new external blocker",
            owner: "call_prelude_string_new",
            expected_rows: 2,
            reasons: &["external_dependency_summary_missing"],
        },
        Case {
            label: "crate macro blocker",
            owner: "call_crate_scoped_macro",
            expected_rows: 2,
            reasons: &["macro_expansion_not_available"],
        },
        Case {
            label: "ambiguous trait method blocker",
            owner: "call_ambiguous_trait_method",
            expected_rows: 2,
            reasons: &["type_resolution_missing"],
        },
        Case {
            label: "function pointer path blocker",
            owner: "call_function_pointer_param",
            expected_rows: 2,
            reasons: &["type_resolution_missing"],
        },
        Case {
            label: "boxed dyn Fn path blockers",
            owner: "call_boxed_dyn_fn_value_binding",
            expected_rows: 4,
            reasons: &[
                "external_dependency_summary_missing",
                "type_resolution_missing",
            ],
        },
        Case {
            label: "parenthesized boxed dyn Fn blockers",
            owner: "call_parenthesized_boxed_dyn_fn_value_binding",
            expected_rows: 4,
            reasons: &[
                "dynamic_dispatch_unbounded",
                "external_dependency_summary_missing",
            ],
        },
    ]
}

fn resolve_case(db: &Database, case: Case) -> color_eyre::Result<ResolvedCase> {
    Ok(ResolvedCase {
        label: case.label,
        owner: one_uuid(db, &function_in_module_query(&["crate"], case.owner))?,
        search_term: case.owner,
        call_id: match case.owner {
            "call_prelude_string_new" => "string_new_blocker_proof_context",
            "call_crate_scoped_macro" => "macro_blocker_proof_context",
            "call_ambiguous_trait_method" => "ambiguous_method_blocker_proof_context",
            "call_function_pointer_param" => "fn_pointer_blocker_proof_context",
            "call_boxed_dyn_fn_value_binding" => "boxed_dyn_fn_blocker_proof_context",
            "call_parenthesized_boxed_dyn_fn_value_binding" => {
                "parenthesized_boxed_dyn_fn_blocker_proof_context"
            }
            _ => unreachable!("unregistered blocker proof fixture case"),
        },
        expected_rows: case.expected_rows,
        reasons: case.reasons,
    })
}

fn assert_proof_blockers(
    tool_result: &crate::tools::ToolResult,
    result: &RequestCodeContextResult,
) {
    let payload = tool_result
        .ui_payload
        .as_ref()
        .expect("request_code_context should emit a UI payload");
    let expected = result
        .context
        .iter()
        .flat_map(|part| part.proof_context.iter())
        .filter(|proof| proof.blocker_reason.is_some())
        .count();
    assert_eq!(ui_field(payload, "proof_blockers"), expected.to_string());
}

struct ResolvedCase {
    label: &'static str,
    owner: Uuid,
    search_term: &'static str,
    call_id: &'static str,
    expected_rows: usize,
    reasons: &'static [&'static str],
}

fn assert_owner_blocker_reason(
    rows: &[ploke_core::rag_types::ProofContextInfo],
    owner: Uuid,
    reason: &str,
) {
    let owner = owner.to_string();
    let site_ids = rows
        .iter()
        .filter(|row| {
            row.kind == "call_site" && row.caller_def_id.as_deref() == Some(owner.as_str())
        })
        .filter_map(|row| row.call_site_id.as_deref())
        .collect::<Vec<_>>();
    assert!(
        !site_ids.is_empty(),
        "proof context should include owner call_site facts: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.blocker_reason.as_deref() == Some(reason)
                && row
                    .call_site_id
                    .as_deref()
                    .is_some_and(|site| site_ids.contains(&site))
        }),
        "proof context should include owner-linked call_resolution blocker reason {reason}: {rows:#?}"
    );
}
