use super::*;

#[test]
fn fixture_context_reads_projected_imported_associated_function_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let imported_make = method_id_by_impl_self_type_name(&db, "ImportedAssoc", "make")?;
    let nested_reexported_make = method_id_by_impl_self_type_name(&db, "NestedGlobAssoc", "make")?;
    let cases = [
        (
            &["crate"][..],
            "call_imported_type_assoc_make",
            path(&["ImportedAssocAlias", "make"]),
            imported_make,
        ),
        (
            &["crate"],
            "call_glob_imported_type_assoc_make",
            path(&["ImportedAssoc", "make"]),
            imported_make,
        ),
        (
            &["crate"],
            "call_reexported_type_assoc_make",
            path(&["ReexportedAssoc", "make"]),
            imported_make,
        ),
        (
            &["crate", "nested_glob_assoc_scope"],
            "call_nested_glob_reexported_type_assoc_make",
            path(&["NestedGlobAssoc", "make"]),
            nested_reexported_make,
        ),
        (
            &["crate", "direct_reexport_assoc_scope"],
            "call_direct_reexported_type_assoc_make",
            path(&["NestedGlobAssoc", "make"]),
            nested_reexported_make,
        ),
        (
            &["crate", "inherited_glob_assoc_parent", "child"],
            "call_inherited_glob_reexported_type_assoc_make",
            path(&["NestedGlobAssoc", "make"]),
            nested_reexported_make,
        ),
    ];

    for (module_path, owner_name, expected_path, target) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
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
