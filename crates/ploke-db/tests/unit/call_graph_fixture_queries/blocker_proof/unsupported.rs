use super::*;

#[test]
fn fixture_projection_marks_unimported_trait_method_without_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_unimported_trait_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let receiver = CallReceiver::LocalBinding {
        name: "value".to_string(),
    };
    let row = assert_targetless_method_row(
        &context,
        owner,
        TargetlessMethodCase::method(
            "scoped_value",
            &receiver,
            CallStatusKind::Unsupported,
            "unimported trait method",
        ),
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 2);
    assert_targetless_blocker_proofs(
        &db,
        "unimported trait method",
        &[BlockerProofSite {
            site: row.site.id,
            span: row.site.span,
            blocker_reason: "type_resolution_missing",
        }],
        "fixture_call_graph/src/lib.rs",
    )
}
