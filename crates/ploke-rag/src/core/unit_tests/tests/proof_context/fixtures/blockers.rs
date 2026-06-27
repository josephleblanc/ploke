use super::super::super::*;

struct Case {
    label: &'static str,
    owner: &'static str,
    expected_rows: usize,
    reasons: &'static [&'static str],
}

#[tokio::test]
async fn proof_context_collection_preserves_projected_blocker_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let cases = blocker_cases()
        .into_iter()
        .map(|case| resolve_case(&db, case))
        .collect::<Result<Vec<_>, _>>()?;

    for case in &cases {
        assert_eq!(
            db.project_call_proof_facts_for_owner(case.owner, "bd:fixture-call-graph")?,
            case.expected_rows,
            "{} proof fact count",
            case.label
        );
    }

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected blocker proof facts should enable RAG proof context"
    );

    let seeds = cases
        .iter()
        .map(|case| (case.owner, 1.0))
        .collect::<Vec<_>>();
    let proof_context = rag.collect_proof_context(&seeds)?;
    for case in &cases {
        let rows = proof_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} owner seed should receive proof rows", case.label));
        assert_eq!(
            rows.len(),
            case.expected_rows,
            "{} proof rows: {rows:#?}",
            case.label
        );
        for reason in case.reasons {
            assert_owner_blocker_reason(rows, case.owner, reason);
        }
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

fn resolve_case(db: &Database, case: Case) -> Result<ResolvedCase, Error> {
    Ok(ResolvedCase {
        label: case.label,
        owner: one_uuid(db, &function_in_module_query(&["crate"], case.owner))?,
        expected_rows: case.expected_rows,
        reasons: case.reasons,
    })
}

struct ResolvedCase {
    label: &'static str,
    owner: Uuid,
    expected_rows: usize,
    reasons: &'static [&'static str],
}

fn assert_owner_blocker_reason(rows: &[ProofContextInfo], owner: Uuid, reason: &str) {
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
