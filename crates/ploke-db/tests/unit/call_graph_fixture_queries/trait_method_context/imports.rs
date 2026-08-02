use super::*;

#[test]
fn fixture_context_reads_projected_imported_trait_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ScopedTrait",
        "ScopedTraitTarget",
        "scoped_value",
    )?;
    let cases = [
        (
            &["crate", "trait_scope", "with_direct_import"][..],
            "call_direct_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_alias_import"][..],
            "call_alias_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_glob_import"][..],
            "call_glob_imported_trait_method",
        ),
        (
            &["crate", "trait_reexport_scope"][..],
            "call_reexported_trait_method",
        ),
        (
            &["crate", "grouped_trait_import_scope"][..],
            "call_grouped_imported_trait_method",
        ),
    ];

    for (module_path, owner_name) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("scoped_value"));
        assert_eq!(
            row.site.receiver,
            Some(CallReceiver::LocalBinding {
                name: "value".to_string(),
            })
        );
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );
    }

    Ok(())
}
