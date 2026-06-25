use super::*;

#[test]
fn fixture_projection_stores_real_dynamic_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_aliased_indexed_named_field_function_binding")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let row = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["alias", "callbacks", "0"],
    );
    let site = row.site.id;
    let span = row.site.span;
    assert_resolved_target(
        row,
        target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3);

    assert_owner_proof_edges(
        &db,
        "dynamic",
        &[OwnerProofEdge {
            owner,
            site,
            span,
            target,
        }],
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_branch_and_match_dynamic_call_proof_facts() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let mut expected_edges = Vec::new();

    for owner_name in [
        "call_if_same_function_item",
        "call_match_same_function_item",
    ] {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["local_target"]);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "branch/match dynamic",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_callable_expression_dynamic_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases: [(&str, &[&str]); 11] = [
        ("call_parenthesized_local_target", &["local_target"]),
        ("call_parenthesized_function_item_binding", &["f"]),
        ("call_parenthesized_aliased_function_item_binding", &["g"]),
        (
            "call_parenthesized_typed_function_pointer_alias_binding",
            &["g"],
        ),
        ("call_function_pointer_cast_path", &["local_target"]),
        ("call_function_pointer_cast_binding", &["f"]),
        ("call_dereferenced_function_pointer_binding", &["f"]),
        ("call_block_function_item", &["local_target"]),
        ("call_indexed_initialized_function_array", &["funcs", "0"]),
        (
            "call_typed_indexed_initialized_function_array",
            &["funcs", "0"],
        ),
        (
            "call_aliased_indexed_initialized_function_array",
            &["alias", "0"],
        ),
    ];
    let mut expected_edges = Vec::new();

    for (owner_name, expected_path) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "callable dynamic",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_field_dynamic_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases: [(&str, &[&str]); 8] = [
        ("call_named_field_function_binding", &["holder", "callback"]),
        (
            "call_aliased_named_field_function_binding",
            &["alias", "callback"],
        ),
        (
            "call_indexed_named_field_function_binding",
            &["holder", "callbacks", "0"],
        ),
        (
            "call_indexed_named_field_array_alias_binding",
            &["holder", "callbacks", "0"],
        ),
        (
            "call_aliased_indexed_named_field_function_binding",
            &["alias", "callbacks", "0"],
        ),
        (
            "call_indexed_tuple_field_function_binding",
            &["holder", "0", "0"],
        ),
        (
            "call_indexed_tuple_field_array_alias_binding",
            &["holder", "0", "0"],
        ),
        (
            "call_aliased_indexed_tuple_field_function_binding",
            &["alias", "0", "0"],
        ),
    ];
    let mut expected_edges = Vec::new();

    for (owner_name, expected_path) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        let expected_count = context
            .iter()
            .map(|row| 2 + row.targets.len())
            .sum::<usize>();
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, expected_count, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "field dynamic",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
        ProofEdgeCount::AtLeast,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_marks_real_unsupported_dynamic_call_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut expected = Vec::new();

    for owner_name in [
        "call_closure_binding_cast",
        "call_dereferenced_closure_binding",
    ] {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = &context[0];
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.kind, CallSiteKind::Dynamic);
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{owner_name} unsupported dynamic proof setup must be targetless: {row:#?}"
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2);
        expected.push(BlockerProofSite {
            site,
            span,
            blocker_reason: "dynamic_dispatch_unbounded",
        });
    }

    assert_targetless_blocker_proofs(
        &db,
        "unsupported dynamic",
        &expected,
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_marks_real_branch_and_match_dynamic_ambiguity_with_candidates()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;
    let expected_names = candidate_strings(&expected);

    for owner_name in AMBIGUOUS_DYNAMIC_OWNERS {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = &context[0];
        assert_dynamic_candidates(row, owner, &expected, owner_name);

        let site = row.site.id.to_string();
        let facts = db.call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_candidate_proof(&facts, &site, &expected_names, owner_name);

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2, "{owner_name} projected proof fact count");
        assert_candidate_blocker(&db, &site, owner_name)?;
    }

    Ok(())
}

#[test]
fn fixture_projection_marks_real_branch_and_match_dynamic_failures_without_edges()
-> Result<(), DbError> {
    let cases = [
        (
            "call_match_guarded_function_item",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_if_closure_branch",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_match_closure_arm",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_if_function_pointer_param_branch",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_match_function_pointer_param_arm",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_if_nested_branch_expression",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
        (
            "call_match_nested_arm_expression",
            CallStatusKind::Unsupported,
            "dynamic_dispatch_unbounded",
        ),
    ];

    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let mut expected = Vec::new();

    for (owner_name, expected_status, blocker_reason) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = &context[0];
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.kind, CallSiteKind::Dynamic);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.status.status, expected_status);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{owner_name} dynamic failure proof setup must be targetless: {row:#?}"
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 2);
        expected.push(BlockerProofSite {
            site,
            span,
            blocker_reason,
        });
    }

    assert_targetless_blocker_proofs(
        &db,
        "branch/match dynamic failure",
        &expected,
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}
