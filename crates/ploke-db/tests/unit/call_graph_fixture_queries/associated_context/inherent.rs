use super::*;

#[test]
fn fixture_context_reads_projected_associated_function_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_local_assoc_make")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["LocalAssoc", "make"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_self_and_qualified_associated_function_calls()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let cases = [
        (
            method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?,
            path(&["Self", "make"]),
        ),
        (
            function_id_by_name(&db, "call_qualified_local_assoc_make")?,
            path(&["LocalAssoc", "make"]),
        ),
    ];

    for (owner, expected_path) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
        );
    }

    Ok(())
}
