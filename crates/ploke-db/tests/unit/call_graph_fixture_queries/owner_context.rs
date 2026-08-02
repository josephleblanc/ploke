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
    let cases = const_static_cases(&db)?;
    assert_initializer_contexts(&db, &cases)?;

    Ok(())
}

#[test]
fn fixture_context_reads_projected_associated_const_initializer_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = assoc_const_cases(&db)?;
    assert_initializer_contexts(&db, &cases)?;

    Ok(())
}
