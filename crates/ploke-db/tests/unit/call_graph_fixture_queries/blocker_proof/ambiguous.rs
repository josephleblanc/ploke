use super::*;

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
