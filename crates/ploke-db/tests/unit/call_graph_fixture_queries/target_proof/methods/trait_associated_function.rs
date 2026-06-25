use super::*;

#[test]
fn fixture_projection_stores_real_target_centered_imported_trait_assoc_function_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_trait_name(&db, "ImportedAssocFunctionTrait", "imported_trait_make")?;
    let cases = [
        (
            &["crate", "trait_assoc_function_scope", "with_direct_import"][..],
            "call_direct_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_alias_import"],
            "call_alias_imported_trait_associated_function",
            &["VisibleAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_glob_import"],
            "call_glob_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "trait_assoc_reexport_scope"],
            "call_reexported_trait_associated_function",
            &["ReexportedAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "grouped_trait_assoc_function_scope"],
            "call_grouped_imported_trait_associated_function",
            &["GroupedAssocFunctionTrait", "imported_trait_make"],
        ),
    ];

    let mut expected = Vec::new();
    for (module_path, owner_name, expected_path) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        expected.push(ProofSiteCase::path(
            owner,
            expected_path,
            CallRelationKind::AssociatedFunction,
            CallTargetKind::Method,
        ));
    }

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(
        &callers,
        target,
        expected.len(),
        "target-centered imported trait associated-function",
    )?;
    let expected_sites = assert_proof_site_cases(&db, &callers, &expected)?;
    assert_target_proof_projection(
        &db,
        "target-centered imported trait associated-function",
        "bd:fixture-call-graph",
        target,
        &callers,
        &expected_sites,
        "src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}
