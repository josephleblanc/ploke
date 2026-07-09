use super::*;

#[test]
fn fixture_projection_marks_real_branch_and_match_dynamic_failures_without_edges()
-> Result<(), DbError> {
    let cases = [
        (
            "call_if_function_pointer_param_branch",
            Some(&["f"][..]),
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_match_function_pointer_param_arm",
            Some(&["f"][..]),
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
    ];

    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut expected = Vec::new();

    for (owner_name, path, expected_status, blocker_reason) in cases {
        assert_projected_blockers(
            &db,
            &mut expected,
            owner_name,
            &[TargetlessBlockerCase {
                row: TargetlessRowCase::dynamic(path, expected_status, owner_name),
                blocker_reason,
            }],
        )?;
    }

    assert_targetless_blocker_proofs(
        &db,
        "branch/match dynamic failure",
        &expected,
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_attaches_non_awaited_async_closure_poll_resume_blockers_without_edges()
-> Result<(), DbError> {
    let cases = [
        (
            "call_async_closure_binding_without_await_with_body_call",
            "non-awaited async closure binding",
        ),
        (
            "call_async_closure_future_binding_without_await_with_body_call",
            "unawaited async closure future binding",
        ),
    ];

    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut expected = Vec::new();
    let mut explicit_sites = Vec::new();

    for (owner_name, label) in cases {
        assert_projected_blockers(
            &db,
            &mut expected,
            owner_name,
            &[TargetlessBlockerCase {
                row: TargetlessRowCase::path(&["closure"], 0, CallStatusKind::Unsupported, label),
                blocker_reason: "type_resolution_missing",
            }],
        )?;

        let site = expected
            .last()
            .expect("projected async closure callsite")
            .site;
        db.upsert_proof_fact_values(&[
            ploke_test_utils::fixture_async_closure_poll_resume_blocker(site, owner_name),
        ])?;
        explicit_sites.push((site, owner_name));
    }

    assert_targetless_blocker_proofs(
        &db,
        "non-awaited async closure poll/resume",
        &expected,
        "fixture_call_graph/src/lib.rs",
    )?;
    assert!(
        db.proof_checker_edges()?.is_empty(),
        "async closure poll/resume blockers must not fabricate proof edges"
    );

    let blockers = db.proof_blockers()?;
    let proof_rows = db.proof_graphrag_context("dynamic_dispatch_unbounded")?;
    for (site, owner_name) in explicit_sites {
        let site = site.to_string();
        assert!(
            blockers.iter().any(|proof| {
                proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.reason == "dynamic_dispatch_unbounded"
                    && proof.status == "blocked"
                    && proof.detail.contains(owner_name)
            }),
            "{owner_name} should expose an explicit async poll/resume proof blocker: {blockers:#?}"
        );
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "proof_blocker"
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
            }),
            "{owner_name} GraphRAG proof context should expose the async poll/resume blocker: {proof_rows:#?}"
        );
    }

    Ok(())
}
