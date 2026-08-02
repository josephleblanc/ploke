use super::*;

#[test]
fn fixture_context_reads_projected_type_alias_associated_function_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let make_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let assoc_cases = [
        (
            "call_type_alias_assoc_make",
            path(&["LocalAssocTypeAlias", "make"]),
        ),
        (
            "call_type_alias_chain_assoc_make",
            path(&["LocalAssocAliasChain", "make"]),
        ),
        (
            "call_imported_type_alias_assoc_make",
            path(&["ImportedLocalAssocAlias", "make"]),
        ),
    ];

    for (owner_name, expected_path) in assoc_cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_resolved_target(
            row,
            make_target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
        );
    }

    let owner = function_id_by_name(&db, "call_method_as_associated_function")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "method-as-associated context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(
        row.site.path.as_ref(),
        Some(&path(&["LocalAssoc", "instance_value"]))
    );
    assert_eq!(row.site.arg_count, Some(1));
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );

    Ok(())
}
