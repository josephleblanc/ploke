use super::*;

#[test]
fn fixture_projection_stores_real_target_centered_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let method_owner = function_id_by_name(&db, "call_typed_local_instance_method")?;
    let assoc_owner = function_id_by_name(&db, "call_method_as_associated_function")?;

    let method_context = db.call_context_for_owner(method_owner)?;
    let method_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let method_site = row_by_method_receiver(&method_context, "instance_value", &method_receiver)
        .site
        .id;

    let assoc_context = db.call_context_for_owner(assoc_owner)?;
    let assoc_site = row_by_path(&assoc_context, &["LocalAssoc", "instance_value"])
        .site
        .id;

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 2, "target-centered method")?;
    assert_target_proof_projection(
        &db,
        "target-centered method",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[
            TargetProofSite {
                owner: method_owner,
                site: method_site,
            },
            TargetProofSite {
                owner: assoc_owner,
                site: assoc_site,
            },
        ],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_associated_function_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let local_owner = function_id_by_name(&db, "call_local_assoc_make")?;
    let self_owner = method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?;
    let qualified_owner = function_id_by_name(&db, "call_qualified_local_assoc_make")?;

    let local_context = db.call_context_for_owner(local_owner)?;
    let local_site = row_by_path(&local_context, &["LocalAssoc", "make"]).site.id;

    let self_context = db.call_context_for_owner(self_owner)?;
    let self_site = row_by_path(&self_context, &["Self", "make"]).site.id;

    let qualified_context = db.call_context_for_owner(qualified_owner)?;
    let qualified_site = row_by_path(&qualified_context, &["LocalAssoc", "make"])
        .site
        .id;

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 3, "target-centered associated-function")?;

    for (owner, expected_path) in [
        (local_owner, &["LocalAssoc", "make"][..]),
        (self_owner, &["Self", "make"][..]),
        (qualified_owner, &["LocalAssoc", "make"][..]),
    ] {
        let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, expected_path);
        assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Method);
    }

    assert_target_proof_projection(
        &db,
        "target-centered associated-function",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[
            TargetProofSite {
                owner: local_owner,
                site: local_site,
            },
            TargetProofSite {
                owner: self_owner,
                site: self_site,
            },
            TargetProofSite {
                owner: qualified_owner,
                site: qualified_site,
            },
        ],
        "src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}

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
        let context = db.call_context_for_owner(owner)?;
        let site = row_by_path(&context, expected_path).site.id;
        expected.push((owner, site, expected_path));
    }

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(
        &callers,
        target,
        expected.len(),
        "target-centered imported trait associated-function",
    )?;

    for (owner, _, expected_path) in &expected {
        let caller = caller_by_owner_kind_path(&callers, *owner, CallSiteKind::Path, expected_path);
        assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Method);
    }

    let expected_sites = expected
        .iter()
        .map(|(owner, site, _)| TargetProofSite {
            owner: *owner,
            site: *site,
        })
        .collect::<Vec<_>>();
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

#[test]
fn fixture_projection_stores_real_target_centered_trait_dispatch_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let initialized_owner = function_id_by_name(&db, "call_initialized_local_trait_method")?;
    let chained_owner =
        function_id_by_name(&db, "call_reference_chain_trait_object_binding_method")?;
    let receiver = CallReceiver::InitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["TraitDispatchTarget"]),
    };

    let initialized_context = db.call_context_for_owner(initialized_owner)?;
    let initialized_site = row_by_method_receiver(&initialized_context, "trait_value", &receiver)
        .site
        .id;

    let chained_context = db.call_context_for_owner(chained_owner)?;
    let chained_site = row_by_method_receiver(&chained_context, "trait_value", &receiver)
        .site
        .id;

    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 3, "target-centered trait dispatch")?;

    let initialized =
        caller_by_owner_method_receiver(&callers, initialized_owner, "trait_value", &receiver);
    assert_eq!(initialized.target.relation, CallRelationKind::Method);
    let chained =
        caller_by_owner_method_receiver(&callers, chained_owner, "trait_value", &receiver);
    assert_eq!(chained.target.relation, CallRelationKind::Method);

    assert_target_proof_projection(
        &db,
        "target-centered trait dispatch",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[
            TargetProofSite {
                owner: initialized_owner,
                site: initialized_site,
            },
            TargetProofSite {
                owner: chained_owner,
                site: chained_site,
            },
        ],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}
