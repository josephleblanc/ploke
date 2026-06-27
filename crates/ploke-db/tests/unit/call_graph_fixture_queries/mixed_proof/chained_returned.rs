use super::*;

#[test]
fn fixture_projection_stores_real_chained_returned_function_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_chained_returned_function")?;
    let target = function_id_by_name(&db, "make_unary_fn")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "chained returned-function proof context rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["make_unary_fn"]);
    let path_site = path_row.site.id;
    let path_span = path_row.site.span;
    assert_resolved_target(
        path_row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_row = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::dynamic_args(
            None,
            1,
            CallStatusKind::Unsupported,
            "chained returned-function dynamic call",
        ),
    );
    let dynamic_site = dynamic_row.site.id;
    let dynamic_span = dynamic_row.site.span;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 5);

    assert_owner_proof_edges(
        &db,
        "chained returned-function resolved path",
        &[OwnerProofEdge {
            owner,
            site: path_site,
            span: path_span,
            target,
        }],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    assert_blocker_proofs(
        &db,
        "chained returned-function dynamic blocker",
        &[BlockerProofSite {
            site: dynamic_site,
            span: dynamic_span,
            blocker_reason: "dynamic_dispatch_unbounded",
        }],
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}
