use super::*;

#[test]
fn fixture_projection_stores_real_chained_returned_function_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_chained_returned_function")?;
    let maker = function_id_by_name(&db, "make_unary_fn")?;
    let returned = function_id_by_name(&db, "unary_target")?;
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
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["make_unary_fn"]);
    let dynamic_site = dynamic_row.site.id;
    let dynamic_span = dynamic_row.site.span;
    assert_eq!(dynamic_row.site.arg_count, Some(1));
    assert_resolved_target(
        dynamic_row,
        returned,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 6);

    assert_owner_proof_edges(
        &db,
        "chained returned-function resolved calls",
        &[
            OwnerProofEdge {
                owner,
                site: path_site,
                span: path_span,
                target: maker,
            },
            OwnerProofEdge {
                owner,
                site: dynamic_site,
                span: dynamic_span,
                target: returned,
            },
        ],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}
