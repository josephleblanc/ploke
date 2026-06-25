use super::*;

#[test]
fn fixture_projection_stores_real_returned_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_function")?;
    let target = function_id_by_name(&db, "make_fn")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned function proof context rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["make_fn"]);
    let path_site = path_row.site.id;
    let path_span = path_row.site.span;
    assert_resolved_target(
        path_row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic)
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_rows.len(),
        1,
        "expected one outer returned-function dynamic row: {context:#?}"
    );
    let dynamic_row = dynamic_rows[0];
    let dynamic_site = dynamic_row.site.id;
    let dynamic_span = dynamic_row.site.span;
    assert_eq!(dynamic_row.status.status, CallStatusKind::Unsupported);
    assert_eq!(dynamic_row.status.resolution, None);
    assert!(
        dynamic_row.targets.is_empty(),
        "returned-function dynamic proof setup must be targetless: {dynamic_row:#?}"
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 5);

    assert_owner_proof_edges(
        &db,
        "returned-function resolved path",
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
        "returned-function dynamic blocker",
        &[BlockerProofSite {
            site: dynamic_site,
            span: dynamic_span,
            blocker_reason: "dynamic_dispatch_unbounded",
        }],
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}
