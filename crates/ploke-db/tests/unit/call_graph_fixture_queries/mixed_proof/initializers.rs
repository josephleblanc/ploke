use super::*;

#[test]
fn fixture_projection_stores_real_const_and_static_initializer_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let target = function_id_by_name_in_module(&db, &["crate", "const_static"], "five")?;
    let cases = [
        (
            const_id_by_name(&db, "FN_CALL_CONST")?,
            "const initializer proof context rows",
        ),
        (
            static_id_by_name(&db, "STATIC_FN_CALL")?,
            "static initializer proof context rows",
        ),
    ];
    let mut expected = Vec::new();

    for (owner, label) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{label}: {context:#?}");

        let row = row_by_path(&context, &["five"]);
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.owner_id, owner);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-nodes")?;
        assert_eq!(count, 3);
        expected.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "const/static initializer",
        &expected,
        "fixture_nodes/src/const_static.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_associated_const_initializer_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "assoc_const_value")?;
    let cases = [
        const_id_by_name(&db, "IMPL_ASSOC_VALUE")?,
        const_id_by_name(&db, "TRAIT_ASSOC_VALUE")?,
    ];
    let mut expected = Vec::new();

    for owner in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "associated const initializer proof context rows: {context:#?}"
        );

        let row = row_by_path(&context, &["assoc_const_value"]);
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.owner_id, owner);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        expected.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "associated const initializer",
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}
