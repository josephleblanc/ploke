use ploke_db::ProofInvariantStatus;

use super::*;

#[test]
fn fixture_projection_marks_real_external_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_prelude_string_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;
    let span = context[0].site.span;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);
    assert_targetless_blocker_proofs(
        &db,
        "external",
        &[BlockerProofSite {
            site,
            span,
            blocker_reason: "external_dependency_summary_missing",
        }],
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projected_external_call_blocker_feeds_proof_invariants() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_prelude_string_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);

    db.upsert_proof_fact_values(&[serde_json::json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": "effect:fixture-external-call",
        "call_site_id": site.to_string(),
        "effect_class": "operating_system_process_create",
        "confidence": "fixture-test",
        "blocker_if_unresolved": true,
        "evidence_use": "proof_only"
    })])?;

    let findings = db.proof_invariant_findings()?;
    let finding = findings
        .iter()
        .find(|finding| {
            finding.invariant == "detached_process_successor_handoff"
                && finding.call_site_id.as_deref() == Some(site.to_string().as_str())
        })
        .expect("detached process finding for fixture external call proof");
    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(
        finding
            .reason
            .contains("external_dependency_summary_missing"),
        "fixture external call blocker should feed proof invariants: {finding:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_marks_real_macro_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        ("call_crate_scoped_macro", "crate::crate_scoped_macro"),
        ("call_vec_macro", "vec"),
        ("call_imported_macro_alias", "imported_macro_alias"),
        ("call_item_macro_inside_body", "call_graph_item_macro"),
    ];
    let mut expected = Vec::new();

    for (owner_name, macro_name) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = &context[0];
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.kind, CallSiteKind::Macro);
        assert_eq!(row.site.macro_name.as_deref(), Some(macro_name));
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{owner_name} macro proof setup must be targetless: {row:#?}"
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2);
        expected.push(BlockerProofSite {
            site,
            span,
            blocker_reason: "macro_expansion_not_available",
        });
    }

    assert_targetless_blocker_proofs(&db, "macro", &expected, "fixture_call_graph/src/lib.rs")?;

    Ok(())
}

#[test]
fn fixture_projection_marks_real_ambiguous_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_ambiguous_trait_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let row = &context[0];
    let site = row.site.id;
    let span = row.site.span;
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("overlap"));
    assert_eq!(row.status.status, CallStatusKind::Ambiguous);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "ambiguous proof setup must be targetless: {row:#?}"
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);
    assert_targetless_blocker_proofs(
        &db,
        "ambiguous",
        &[BlockerProofSite {
            site,
            span,
            blocker_reason: "type_resolution_missing",
        }],
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_marks_real_callable_path_and_vec_external_rows_without_edges()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut expected = Vec::new();

    for (owner_name, expected_path) in [
        ("call_function_pointer_param", &["f"][..]),
        ("call_generic_fn_once_value_binding", &["generic_f"][..]),
    ] {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "{owner_name} proof context rows: {context:#?}"
        );

        let row = assert_targetless_row(
            &context,
            owner,
            TargetlessRowCase::path(expected_path, 0, CallStatusKind::Unsupported, owner_name),
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2);
        expected.push(BlockerProofSite {
            site: row.site.id,
            span: row.site.span,
            blocker_reason: "type_resolution_missing",
        });
    }

    let owner = function_id_by_name(&db, "call_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "boxed dyn Fn proof context rows: {context:#?}"
    );
    let box_new = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["Box", "new"],
            1,
            CallStatusKind::External,
            "Box::new proof setup",
        ),
    );
    let boxed_fn = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["boxed_fn"],
            0,
            CallStatusKind::Unsupported,
            "boxed dyn Fn proof setup",
        ),
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 4);
    expected.push(BlockerProofSite {
        site: box_new.site.id,
        span: box_new.site.span,
        blocker_reason: "external_dependency_summary_missing",
    });
    expected.push(BlockerProofSite {
        site: boxed_fn.site.id,
        span: boxed_fn.site.span,
        blocker_reason: "type_resolution_missing",
    });

    let owner = function_id_by_name(&db, "call_prelude_vec_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "Vec::new proof context rows: {context:#?}"
    );
    let vec_new = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["Vec", "new"],
            0,
            CallStatusKind::External,
            "Vec::new proof setup",
        ),
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);
    expected.push(BlockerProofSite {
        site: vec_new.site.id,
        span: vec_new.site.span,
        blocker_reason: "external_dependency_summary_missing",
    });

    assert_targetless_blocker_proofs(
        &db,
        "callable path and external rows",
        &expected,
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_marks_real_parenthesized_callable_dynamic_rows_without_edges()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut expected = Vec::new();

    let owner = function_id_by_name(&db, "call_parenthesized_generic_fn_once_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "parenthesized generic FnOnce proof context rows: {context:#?}"
    );
    let generic_f = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::dynamic(
            Some(&["generic_f"]),
            CallStatusKind::Unsupported,
            "parenthesized generic FnOnce dynamic proof setup",
        ),
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);
    expected.push(BlockerProofSite {
        site: generic_f.site.id,
        span: generic_f.site.span,
        blocker_reason: "dynamic_dispatch_unbounded",
    });

    let owner = function_id_by_name(&db, "call_parenthesized_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "parenthesized boxed dyn Fn proof context rows: {context:#?}"
    );
    let box_new = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["Box", "new"],
            1,
            CallStatusKind::External,
            "parenthesized boxed dyn Fn Box::new proof setup",
        ),
    );
    let boxed_fn = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::dynamic(
            Some(&["boxed_fn"]),
            CallStatusKind::Unsupported,
            "parenthesized boxed dyn Fn dynamic proof setup",
        ),
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 4);
    expected.push(BlockerProofSite {
        site: box_new.site.id,
        span: box_new.site.span,
        blocker_reason: "external_dependency_summary_missing",
    });
    expected.push(BlockerProofSite {
        site: boxed_fn.site.id,
        span: boxed_fn.site.span,
        blocker_reason: "dynamic_dispatch_unbounded",
    });

    assert_targetless_blocker_proofs(
        &db,
        "parenthesized callable dynamic rows",
        &expected,
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}
