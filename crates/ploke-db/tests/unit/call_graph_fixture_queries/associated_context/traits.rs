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

    let trait_method_path_target = method_id_by_trait_name(&db, "TraitMethodPath", "handle")?;
    let owner = function_id_by_name(&db, "call_trait_method_as_path")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait method-as-path context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(
        row.site.path.as_ref(),
        Some(&path(&["TraitMethodPath", "handle"]))
    );
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        trait_method_path_target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );

    let generic_assoc_make_target = method_id_by_trait_name(&db, "GenericAssocPathTrait", "make")?;
    let cases = [
        (
            "call_inline_generic_bound_assoc_path",
            function_id_by_name(&db, "call_inline_generic_bound_assoc_path")?,
        ),
        (
            "call_where_generic_bound_assoc_path",
            function_id_by_name(&db, "call_where_generic_bound_assoc_path")?,
        ),
    ];
    for (owner_name, owner) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&path(&["T", "make"])));
        assert_eq!(row.site.arg_count, Some(1));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            generic_assoc_make_target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
        );
    }

    let parent = function_id_by_name(&db, "call_local_impl_where_bound_trait_associated_function")?;
    let owner = local_item_owner_for_parent_with_label(&db, parent, "local_impl_method:value")?;
    let local_trait_make = method_id_by_trait_name(&db, "LocalAssocFunctionTrait", "trait_make")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "local impl where-bound associated function context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(
        row.site.path.as_ref(),
        Some(&path(&["TraitAssocFunctionTarget", "trait_make"]))
    );
    assert_resolved_target(
        row,
        local_trait_make,
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
