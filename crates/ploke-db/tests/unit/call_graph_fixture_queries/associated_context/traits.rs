use super::*;

#[test]
fn fixture_context_reads_projected_imported_trait_associated_function_calls() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_trait_name(&db, "LocalAssocFunctionTrait", "trait_make")?;
    let owner = function_id_by_name(&db, "call_trait_associated_function")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "local trait associated function context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(
        row.site.path.as_ref(),
        Some(&path(&["LocalAssocFunctionTrait", "trait_make"]))
    );
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );

    let target = method_id_by_trait_name(&db, "ImportedAssocFunctionTrait", "imported_trait_make")?;
    let cases = [
        (
            path(&["crate", "trait_assoc_function_scope", "with_direct_import"]),
            "call_direct_imported_trait_associated_function",
            path(&["ImportedAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "trait_assoc_function_scope", "with_alias_import"]),
            "call_alias_imported_trait_associated_function",
            path(&["VisibleAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "trait_assoc_function_scope", "with_glob_import"]),
            "call_glob_imported_trait_associated_function",
            path(&["ImportedAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "trait_assoc_reexport_scope"]),
            "call_reexported_trait_associated_function",
            path(&["ReexportedAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "grouped_trait_assoc_function_scope"]),
            "call_grouped_imported_trait_associated_function",
            path(&["GroupedAssocFunctionTrait", "imported_trait_make"]),
        ),
    ];

    for (module_path, owner_name, expected_path) in cases {
        let refs = module_path.iter().map(String::as_str).collect::<Vec<_>>();
        let owner = function_id_by_name_in_module(&db, &refs, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
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
