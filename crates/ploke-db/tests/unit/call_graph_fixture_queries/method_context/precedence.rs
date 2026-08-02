use super::*;

#[test]
fn fixture_context_reads_projected_inherent_method_precedence() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_inherent_over_trait_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "inherent precedence rows: {context:#?}");

    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["InherentPrecedenceTarget"]),
    };
    let row = row_by_method_receiver(&context, "priority", &receiver);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].relation, CallRelationKind::Method);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
    assert!(
        method_owner_is_inherent_impl(&db, row.targets[0].target_id, "InherentPrecedenceTarget")?,
        "inherent method call must project a target owned by the inherent impl: {row:#?}"
    );

    Ok(())
}
