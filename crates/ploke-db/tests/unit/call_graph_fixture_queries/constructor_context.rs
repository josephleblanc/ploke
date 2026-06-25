use super::*;

#[test]
fn fixture_context_reads_projected_tuple_struct_constructor_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_new_type_constructor")?;
    let target = struct_id_by_name(&db, "NewType")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["NewType"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(
        row.targets[0].relation,
        CallRelationKind::TupleStructConstructor
    );
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Struct);

    Ok(())
}
#[test]
fn fixture_context_reads_projected_enum_variant_constructor_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let owner = function_id_by_name_in_module(&db, &["crate", "imports"], "use_imported_items")?;

    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["EnumWithData", "Variant1"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(
        row.targets[0].relation,
        CallRelationKind::EnumVariantConstructor
    );
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Variant);

    Ok(())
}
