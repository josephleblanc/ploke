use super::*;

#[test]
fn fixture_context_reads_projected_generic_unsafe_extern_and_chained_calls() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_generic_identity_turbofish")?;
    let target = function_id_by_name(&db, "generic_identity")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "generic function context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["generic_identity"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(1));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner = function_id_by_name(&db, "call_method_turbofish")?;
    let target = method_id_by_impl_self_type_name(&db, "GenericMethodTarget", "generic_instance")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "generic method context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("generic_instance"));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(1));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["GenericMethodTarget"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_unsafe_function")?;
    let target = function_id_by_name(&db, "unsafe_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "unsafe function context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["unsafe_target"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner = function_id_by_name(&db, "call_extern_c_function")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "extern C context rows: {context:#?}");
    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(&["abs"], 1, CallStatusKind::External, "extern C abs"),
    );

    let owner = function_id_by_name(&db, "call_chained_returned_function")?;
    let target = function_id_by_name(&db, "make_unary_fn")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "chained call context rows: {context:#?}");

    let row = row_by_path(&context, &["make_unary_fn"]);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::dynamic_args(
            None,
            1,
            CallStatusKind::Unsupported,
            "returned-function dynamic call",
        ),
    );

    Ok(())
}
