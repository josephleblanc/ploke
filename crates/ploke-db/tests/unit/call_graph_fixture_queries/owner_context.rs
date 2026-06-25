use super::*;

#[test]
fn fixture_context_reads_projected_method_body_owner_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = method_id_by_trait_name(&db, "TraitDefaultCall", "default_calls_local")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait default path context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["local_target"]);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner = method_id_by_impl_trait_and_self_type_names(
        &db,
        "TraitImplBodyCallTrait",
        "TraitDispatchTarget",
        "impl_calls_required",
    )?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "TraitImplBodyCallTrait",
        "TraitDispatchTarget",
        "required_impl_call",
    )?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait impl self-method context rows: {context:#?}"
    );

    let row = row_by_method_receiver(&context, "required_impl_call", &CallReceiver::SelfValue);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = method_id_by_trait_name(&db, "DefaultRequiredCall", "default_calls_required")?;
    let target = method_id_by_trait_name(&db, "DefaultRequiredCall", "required")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait default self-method context rows: {context:#?}"
    );

    let row = row_by_method_receiver(&context, "required", &CallReceiver::SelfValue);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = method_id_by_trait_name(&db, "TraitDefaultAssocCall", "default_calls_assoc")?;
    let target = method_id_by_trait_name(&db, "TraitDefaultAssocCall", "required_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait default associated-function context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["Self", "required_assoc"]);
    assert_eq!(row.site.owner_id, owner);
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
fn fixture_context_reads_projected_const_and_static_initializer_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let target = function_id_by_name_in_module(&db, &["crate", "const_static"], "five")?;

    let cases = [
        (
            const_id_by_name(&db, "FN_CALL_CONST")?,
            "const initializer context rows",
        ),
        (
            static_id_by_name(&db, "STATIC_FN_CALL")?,
            "static initializer context rows",
        ),
    ];

    for (owner, label) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{label}: {context:#?}");

        let row = row_by_path(&context, &["five"]);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_associated_const_initializer_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "assoc_const_value")?;
    let cases = [
        const_id_by_name(&db, "IMPL_ASSOC_VALUE")?,
        const_id_by_name(&db, "TRAIT_ASSOC_VALUE")?,
    ];

    for owner in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "associated const initializer context rows: {context:#?}"
        );

        let row = row_by_path(&context, &["assoc_const_value"]);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
    }

    Ok(())
}
