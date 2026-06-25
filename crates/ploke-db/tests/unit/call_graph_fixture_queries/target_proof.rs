use super::*;

#[test]
fn fixture_projection_links_target_centered_proof_rows_to_callers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 2,
        "local_target should expose multiple incoming callers: {callers:#?}"
    );
    let resolved_callers = callers
        .iter()
        .filter(|caller| caller.status.status == CallStatusKind::Resolved)
        .collect::<Vec<_>>();
    assert!(
        resolved_callers.len() >= 2,
        "target-centered proof linkage setup should include resolved callers for the seed target: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "target-centered proof linkage setup returned mismatched target rows: {callers:#?}"
    );

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    let expected_count = callers
        .iter()
        .map(|caller| {
            if caller.status.status == CallStatusKind::Resolved {
                3
            } else {
                2
            }
        })
        .sum::<usize>();
    assert_eq!(
        count, expected_count,
        "target-centered proof projection should emit call_site and call_resolution for each caller, plus call_edge for resolved callers"
    );

    let proof_rows = db.proof_graphrag_context("")?;
    assert_eq!(
        proof_rows.len(),
        count,
        "target-centered projection should not store unrelated proof rows: {proof_rows:#?}"
    );
    let checker_edges = db.proof_checker_edges()?;
    assert_eq!(
        checker_edges.len(),
        resolved_callers.len(),
        "proof checker edges should match target-centered caller rows: {checker_edges:#?}"
    );

    for caller in &callers {
        let site = caller.site.id.to_string();
        let site_rows = proof_rows
            .iter()
            .filter(|fact| fact.call_site_id.as_deref() == Some(site.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            proof_kind_count(&site_rows, "call_site"),
            1,
            "target-centered proof rows should include one call_site fact for {site}: {site_rows:#?}"
        );
        assert_eq!(
            proof_kind_count(&site_rows, "call_resolution"),
            1,
            "target-centered proof rows should include one call_resolution fact for {site}: {site_rows:#?}"
        );
        assert_eq!(
            proof_kind_count(&site_rows, "call_edge"),
            usize::from(caller.status.status == CallStatusKind::Resolved),
            "target-centered proof rows should include call_edge facts only for resolved callers for {site}: {site_rows:#?}"
        );

        let resolution = proof_fact_for_kind(&site_rows, "call_resolution");
        if caller.status.status == CallStatusKind::Resolved {
            let edge = proof_fact_for_kind(&site_rows, "call_edge");
            let owner_id = caller.site.owner_id.to_string();
            let target_id = target.to_string();
            assert_eq!(edge.caller_def_id.as_deref(), Some(owner_id.as_str()));
            assert_eq!(edge.callee_def_id.as_deref(), Some(target_id.as_str()));
            assert_eq!(edge.blocker_reason, None);
            assert_eq!(resolution.blocker_reason, None);
        } else {
            assert_eq!(
                resolution.blocker_reason.as_deref(),
                Some("type_resolution_missing")
            );
        }

        let provenance = db
            .proof_source_provenance(&site)?
            .expect("projected target-centered caller source provenance");
        assert_eq!(provenance.call_site_id, site);
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
        assert_eq!(provenance.start_byte, caller.site.span.0);
        assert_eq!(provenance.end_byte, caller.site.span.1);
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "try_local_assoc")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    let site = row_by_path(&context, &["try_local_assoc"]).site.id;
    let callers = db.callers_for_target(target)?;
    assert_resolved_target_callers(&callers, target, 1, "target-centered function")?;

    assert_target_proof_projection(
        &db,
        "target-centered function",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[TargetProofSite { owner, site }],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_dynamic_call_proof_facts() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let owner = function_id_by_name(&db, "call_aliased_indexed_named_field_function_binding")?;
    let context = db.call_context_for_owner(owner)?;
    let site = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["alias", "callbacks", "0"],
    )
    .site
    .id;

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.len() >= 2,
        "local_target should have multiple incoming callers: {callers:#?}"
    );
    assert!(
        callers
            .iter()
            .all(|caller| caller.target.target_id == target),
        "target-centered dynamic proof setup returned mismatched target rows: {callers:#?}"
    );
    let dynamic = caller_by_owner_kind_path(
        &callers,
        owner,
        CallSiteKind::Dynamic,
        &["alias", "callbacks", "0"],
    );
    assert_eq!(dynamic.target.relation, CallRelationKind::DynamicFunction);

    assert_target_proof_projection(
        &db,
        "target-centered dynamic",
        "bd:fixture-call-graph",
        target,
        &callers,
        &[TargetProofSite { owner, site }],
        "fixture_call_graph/src/lib.rs",
        "dynamic_dispatch_unbounded",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_target_centered_constructor_call_proof_facts()
-> Result<(), DbError> {
    for case in constructor_cases() {
        let db = setup_call_graph_fixture_db(case.fixture)?;
        let resolved = assert_constructor_context(&db, case)?;
        let callers = assert_constructor_callers(&db, case, &resolved)?;

        assert_target_proof_projection(
            &db,
            case.label,
            case.domain,
            resolved.target,
            &callers,
            &[TargetProofSite {
                owner: resolved.owner,
                site: resolved.site,
            }],
            case.source_suffix,
            "type_resolution_missing",
        )?;
    }

    Ok(())
}

#[test]
fn fixture_projection_excludes_closure_async_outer_owners_from_target_proof() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let forbidden_owners = [
        function_id_by_name(&db, "closure_body_call_is_not_outer_call_site")?,
        function_id_by_name(&db, "async_block_call_is_not_outer_call_site")?,
        function_id_by_name(&db, "call_move_closure_literal_with_body_call")?,
        function_id_by_name(&db, "call_async_closure_literal_with_body_call")?,
    ]
    .map(|owner| owner.to_string());

    let count = db.project_call_proof_facts_for_target(target, "bd:fixture-call-graph")?;
    assert!(
        count > 0,
        "target-centered proof should project real callers"
    );

    let edges = db.proof_checker_edges()?;
    assert!(
        !edges.is_empty(),
        "target-centered local_target proof should include resolved proof edges"
    );
    assert!(
        edges
            .iter()
            .all(|edge| !forbidden_owners.contains(&edge.caller_def_id)),
        "target-centered local_target proof leaked closure/async outer owners into proof edges: {edges:#?}"
    );

    let rows = db.proof_graphrag_context("")?;
    assert!(
        rows.iter().all(|row| match row.caller_def_id.as_ref() {
            Some(caller) => !forbidden_owners.contains(caller),
            None => true,
        }),
        "target-centered local_target proof leaked closure/async outer owners into proof facts: {rows:#?}"
    );

    Ok(())
}

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
