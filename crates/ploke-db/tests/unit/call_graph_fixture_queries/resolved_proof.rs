use super::*;

#[test]
fn fixture_projection_stores_real_resolved_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;
    let span = context[0].site.span;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3);

    assert_owner_proof_edges(
        &db,
        "resolved call",
        &[OwnerProofEdge {
            owner,
            site,
            span,
            target,
        }],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_path_resolution_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let nested_target =
        function_id_by_name_in_module(&db, &["crate", "local_mod"], "nested_target")?;
    let imported_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "imported_target")?;
    let globbed_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "globbed_target")?;
    let cases: [(&[&str], &str, &[&str], Uuid); 11] = [
        (
            &["crate"],
            "call_unqualified_local_target",
            &["local_target"],
            local_target,
        ),
        (
            &["crate", "local_mod"],
            "call_self_nested_target",
            &["self", "nested_target"],
            nested_target,
        ),
        (
            &["crate"],
            "call_crate_module_nested_target",
            &["crate", "local_mod", "nested_target"],
            nested_target,
        ),
        (
            &["crate"],
            "call_self_module_nested_target",
            &["self", "local_mod", "nested_target"],
            nested_target,
        ),
        (
            &["crate", "super_path_scope"],
            "call_super_local_target",
            &["super", "local_target"],
            local_target,
        ),
        (
            &["crate"],
            "call_imported_alias_target",
            &["imported_alias"],
            imported_target,
        ),
        (
            &["crate"],
            "call_glob_imported_target",
            &["globbed_target"],
            globbed_target,
        ),
        (
            &["crate"],
            "call_reexported_target",
            &["reexported_target"],
            imported_target,
        ),
        (
            &["crate"],
            "call_imported_module_target",
            &["targets_alias", "globbed_target"],
            globbed_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_alias_target",
            &["grouped_alias"],
            imported_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_globbed_target",
            &["grouped_globbed_alias"],
            globbed_target,
        ),
    ];
    let mut expected_edges = Vec::new();

    for (module_path, owner_name, expected_path, target) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_path(&context, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "path-resolution",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_associated_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_make = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let imported_make = method_id_by_impl_self_type_name(&db, "ImportedAssoc", "make")?;
    let instance_value = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let local_trait_make = method_id_by_trait_name(&db, "LocalAssocFunctionTrait", "trait_make")?;
    let imported_trait_make =
        method_id_by_trait_name(&db, "ImportedAssocFunctionTrait", "imported_trait_make")?;

    let mut cases = Vec::new();
    cases.push((
        "call_local_assoc_make",
        function_id_by_name(&db, "call_local_assoc_make")?,
        &["LocalAssoc", "make"][..],
        local_make,
    ));
    cases.push((
        "call_self_make",
        method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?,
        &["Self", "make"],
        local_make,
    ));
    cases.push((
        "call_qualified_local_assoc_make",
        function_id_by_name(&db, "call_qualified_local_assoc_make")?,
        &["LocalAssoc", "make"],
        local_make,
    ));
    cases.push((
        "call_imported_type_assoc_make",
        function_id_by_name(&db, "call_imported_type_assoc_make")?,
        &["ImportedAssocAlias", "make"],
        imported_make,
    ));
    cases.push((
        "call_glob_imported_type_assoc_make",
        function_id_by_name(&db, "call_glob_imported_type_assoc_make")?,
        &["ImportedAssoc", "make"],
        imported_make,
    ));
    cases.push((
        "call_reexported_type_assoc_make",
        function_id_by_name(&db, "call_reexported_type_assoc_make")?,
        &["ReexportedAssoc", "make"],
        imported_make,
    ));
    cases.push((
        "call_type_alias_assoc_make",
        function_id_by_name(&db, "call_type_alias_assoc_make")?,
        &["LocalAssocTypeAlias", "make"],
        local_make,
    ));
    cases.push((
        "call_type_alias_chain_assoc_make",
        function_id_by_name(&db, "call_type_alias_chain_assoc_make")?,
        &["LocalAssocAliasChain", "make"],
        local_make,
    ));
    cases.push((
        "call_imported_type_alias_assoc_make",
        function_id_by_name(&db, "call_imported_type_alias_assoc_make")?,
        &["ImportedLocalAssocAlias", "make"],
        local_make,
    ));
    cases.push((
        "call_method_as_associated_function",
        function_id_by_name(&db, "call_method_as_associated_function")?,
        &["LocalAssoc", "instance_value"],
        instance_value,
    ));
    cases.push((
        "call_trait_associated_function",
        function_id_by_name(&db, "call_trait_associated_function")?,
        &["LocalAssocFunctionTrait", "trait_make"],
        local_trait_make,
    ));

    for (module_path, owner_name, expected_path) in [
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
    ] {
        cases.push((
            owner_name,
            function_id_by_name_in_module(&db, module_path, owner_name)?,
            expected_path,
            imported_trait_make,
        ));
    }

    let mut expected_edges = Vec::new();

    for (owner_name, owner, expected_path, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_path(&context, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "associated-function",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_local_receiver_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let cases = [
        (
            "call_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_initialized_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_parenthesized_typed_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_type_alias_chain_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssocAliasChain"]),
            },
        ),
        (
            "call_imported_type_alias_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["ImportedLocalAssocAlias"]),
            },
        ),
        (
            "call_borrowed_typed_local_instance_method",
            CallReceiver::BorrowedTypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_local_instance_method",
            CallReceiver::DereferencedInitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_param_instance_method",
            CallReceiver::DereferencedLocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_borrowed_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_referenced_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_typed_reference_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
    ];
    let mut expected_edges = Vec::new();

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_method_receiver(&context, "instance_value", &receiver);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "local receiver method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_trait_family_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let generic_target = method_id_by_trait_name(&db, "GenericBoundTrait", "bound_value")?;
    let imported_target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ScopedTrait",
        "ScopedTraitTarget",
        "scoped_value",
    )?;
    let constrained_target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ConstrainedGenericSelfTrait",
        "GenericWrapper",
        "constrained_generic_self_value",
    )?;
    let mut cases = Vec::new();

    for owner_name in [
        "call_inline_generic_bound_method",
        "call_where_generic_bound_method",
        "call_impl_trait_method",
        "call_trait_object_method",
    ] {
        cases.push((
            owner_name,
            function_id_by_name(&db, owner_name)?,
            "bound_value",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            generic_target,
        ));
    }

    cases.push((
        "call_local_trait_object_binding_method",
        function_id_by_name(&db, "call_local_trait_object_binding_method")?,
        "bound_value",
        CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["GenericBoundTrait"]),
        },
        generic_target,
    ));

    for (module_path, owner_name) in [
        (
            &["crate", "trait_scope", "with_direct_import"][..],
            "call_direct_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_alias_import"],
            "call_alias_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_glob_import"],
            "call_glob_imported_trait_method",
        ),
        (
            &["crate", "trait_reexport_scope"],
            "call_reexported_trait_method",
        ),
        (
            &["crate", "grouped_trait_import_scope"],
            "call_grouped_imported_trait_method",
        ),
    ] {
        cases.push((
            owner_name,
            function_id_by_name_in_module(&db, module_path, owner_name)?,
            "scoped_value",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            imported_target,
        ));
    }

    cases.push((
        "call_constrained_generic_self_trait_method",
        function_id_by_name(&db, "call_constrained_generic_self_trait_method")?,
        "constrained_generic_self_value",
        CallReceiver::LocalBinding {
            name: "value".to_string(),
        },
        constrained_target,
    ));

    for (owner_name, trait_name, method_name) in [
        (
            "call_blanket_trait_method",
            "BlanketDispatchTrait",
            "blanket_value",
        ),
        (
            "call_inline_bound_blanket_trait_method",
            "InlineBoundBlanketTrait",
            "inline_bound_value",
        ),
        (
            "call_where_bound_blanket_trait_method",
            "WhereBoundBlanketTrait",
            "where_bound_value",
        ),
        (
            "call_transitive_bound_blanket_trait_method",
            "TransitiveBoundBlanketTrait",
            "transitive_bound_value",
        ),
    ] {
        cases.push((
            owner_name,
            function_id_by_name(&db, owner_name)?,
            method_name,
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            method_id_by_impl_trait_name(&db, trait_name, method_name)?,
        ));
    }

    let mut expected_edges = Vec::new();

    for (owner_name, owner, method_name, receiver, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_method_receiver(&context, method_name, &receiver);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "trait family method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_result_and_field_receiver_method_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let clone_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "clone_assoc")?;
    let make_target = function_id_by_name(&db, "make_local_assoc")?;
    let ready_target = function_id_by_name(&db, "make_ready_local_assoc")?;
    let tuple_target = struct_id_by_name(&db, "TupleFieldMethodReceiver")?;
    let mut expected_edges = Vec::new();

    let owner = function_id_by_name(&db, "call_path_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "path-result context rows: {context:#?}");
    let row = row_by_path(&context, &["make_local_assoc"]);
    assert_resolved_target(
        row,
        make_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: make_target,
    });

    let receiver = CallReceiver::PathCallResult {
        path: path(&["make_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_method_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "method-result context rows: {context:#?}");
    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let row = row_by_method_receiver(&context, "clone_assoc", &receiver);
    assert_resolved_target(
        row,
        clone_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: clone_target,
    });

    let receiver = CallReceiver::MethodCallResult {
        method_name: "clone_assoc".to_string(),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_await_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "await-result context rows: {context:#?}");
    let row = row_by_path(&context, &["make_ready_local_assoc"]);
    assert_resolved_target(
        row,
        ready_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: ready_target,
    });

    let receiver = CallReceiver::AwaitPathCallResult {
        path: path(&["make_ready_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_tuple_field_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-method context rows: {context:#?}");
    let row = row_by_path(&context, &["TupleFieldMethodReceiver"]);
    assert_resolved_target(
        row,
        tuple_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: tuple_target,
    });

    let receiver = CallReceiver::FieldInitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["TupleFieldMethodReceiver"]),
        field_path: path(&["0"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    assert_owner_proof_edges(
        &db,
        "result/field receiver method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_trait_dispatch_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let cases = [
        "call_initialized_local_trait_method",
        "call_concrete_trait_object_binding_method",
        "call_reference_chain_trait_object_binding_method",
    ];
    let mut expected_edges = Vec::new();

    for owner_name in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let receiver = CallReceiver::InitializedLocalBinding {
            name: "value".to_string(),
            init_path: path(&["TraitDispatchTarget"]),
        };
        let row = row_by_method_receiver(&context, "trait_value", &receiver);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        expected_edges.push(OwnerProofEdge {
            owner,
            site: row.site.id,
            span: row.site.span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "trait dispatch",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}
