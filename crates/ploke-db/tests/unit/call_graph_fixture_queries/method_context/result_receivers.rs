use super::*;

#[test]
fn fixture_context_preserves_path_result_owner_order_and_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_path_result_instance_method")?;
    let make_target = function_id_by_name(&db, "make_local_assoc")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "context rows: {context:#?}");

    let path_row = &context[0];
    assert_eq!(path_row.site.kind, CallSiteKind::Path);
    assert_eq!(
        path_row.site.path.as_ref(),
        Some(&path(&["make_local_assoc"]))
    );
    assert_eq!(path_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        path_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(path_row.targets.len(), 1);
    assert_eq!(path_row.targets[0].target_id, make_target);
    assert_eq!(path_row.targets[0].relation, CallRelationKind::Function);

    let method_row = &context[1];
    assert_eq!(method_row.site.kind, CallSiteKind::Method);
    assert_eq!(method_row.site.method.as_deref(), Some("instance_value"));
    assert_eq!(
        method_row.site.receiver,
        Some(CallReceiver::PathCallResult {
            path: path(&["make_local_assoc"]),
        })
    );
    assert_eq!(method_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        method_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(method_row.targets.len(), 1);
    assert_eq!(method_row.targets[0].relation, CallRelationKind::Method);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_result_receiver_method_chains() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let owner = function_id_by_name(&db, "call_method_result_instance_method")?;
    let clone_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "clone_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "method-result context rows: {context:#?}");

    let clone_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let row = row_by_method_receiver(&context, "clone_assoc", &clone_receiver);
    assert_resolved_target(
        row,
        clone_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let result_receiver = CallReceiver::MethodCallResult {
        method_name: "clone_assoc".to_string(),
    };
    let row = row_by_method_receiver(&context, "instance_value", &result_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_await_result_instance_method")?;
    let ready_target = function_id_by_name(&db, "make_ready_local_assoc")?;
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

    let await_receiver = CallReceiver::AwaitPathCallResult {
        path: path(&["make_ready_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &await_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "try-result context rows: {context:#?}");

    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(&["Ok"], 1, CallStatusKind::Unsupported, "try-result Ok"),
    );

    let row = row_by_path(&context, &["try_local_assoc"]);
    assert_resolved_target(
        row,
        try_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let try_receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &try_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}
