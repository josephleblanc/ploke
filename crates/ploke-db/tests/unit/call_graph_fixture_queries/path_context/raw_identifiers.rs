use super::*;

#[test]
fn fixture_context_reads_projected_raw_identifier_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_raw_identifier_function")?;
    let target = function_id_by_exact_name(&db, "r#match")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "raw function context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["r#match"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner = function_id_by_name(&db, "call_raw_identifier_method")?;
    let target = method_id_by_impl_self_type_exact_name(&db, "RawMethodTarget", "r#type")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "raw method context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("r#type"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["RawMethodTarget"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}
