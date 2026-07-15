use super::*;
use ploke_db::LocalBindingRelationKind;

#[test]
fn fixture_projection_stores_parameter_binding_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_function_pointer_param")?;
    let caller = function_id_by_name(&db, "call_single_function_pointer_param_with_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1523-1528:
    // `call_single_function_pointer_param(f: fn() -> i32) { f() }` is resolved
    // from the private caller that supplies `local_target`; this test proves the
    // callee parameter itself is also queryable as a durable local binding.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["f"]);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let caller_context = db.call_context_for_owner(caller)?;
    let helper_call = row_by_path(&caller_context, &["call_single_function_pointer_param"]);
    assert_resolved_target(
        helper_call,
        owner,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let bindings = bindings
        .into_iter()
        .filter(|binding| binding.kind == "ParameterBinding" && binding.name == "f")
        .collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        1,
        "single function-pointer helper should expose one parameter binding: {bindings:#?}"
    );
    let binding = &bindings[0];
    assert_eq!(binding.source_kind, "Parameter");
    assert_eq!(binding.source_id, None);
    assert_eq!(binding.source_call_kind, None);
    assert_eq!(binding.source_path, None);
    assert_eq!(binding.callee_kind, None);
    assert_eq!(binding.callee_path, None);
    assert!(
        binding.span.0 < binding.span.1,
        "parameter binding should retain the parameter pattern span: {binding:#?}"
    );

    let edges = db.local_binding_edges_for_owner(owner)?;
    let binding_edges = edges
        .iter()
        .filter(|edge| edge.source_id == binding.id || edge.target_id == binding.id)
        .collect::<Vec<_>>();
    assert_eq!(
        binding_edges.len(),
        1,
        "parameter binding should expose containment only until argument-flow edges are modeled: {edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-parameter-binding edge: {binding_edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_let_closure_binding_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_shadowed_local_target_binding")?;
    let module_target = function_id_by_name(&db, "local_target")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "shadowed closure binding context rows: {context:#?}"
    );
    let row = row_by_path(&context, &["local_target"]);
    let closure_id = row.targets[0].target_id;
    assert_ne!(
        closure_id, module_target,
        "local closure binding must not point at the module-level local_target function"
    );
    assert_resolved_target(
        row,
        closure_id,
        CallRelationKind::Closure,
        CallSiteKind::Path,
        CallTargetKind::Closure,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let bindings = bindings
        .into_iter()
        .filter(|binding| binding.kind == "LetBinding")
        .collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        1,
        "shadowed closure owner should expose one let binding: {bindings:#?}"
    );
    let binding = &bindings[0];
    assert_eq!(binding.kind, "LetBinding");
    assert_eq!(binding.name, "local_target");
    assert_eq!(binding.source_kind, "Closure");
    assert_eq!(binding.source_id, Some(closure_id));
    assert_eq!(binding.source_call_kind, None);
    assert_eq!(binding.source_path, None);
    assert_eq!(binding.callee_kind, None);
    assert_eq!(binding.callee_path, None);
    assert!(
        binding.span.0 < binding.span.1,
        "let binding should retain a non-empty source span: {binding:#?}"
    );

    let edges = db.local_binding_edges_for_owner(owner)?;
    let binding_edges = edges
        .iter()
        .filter(|edge| edge.source_id == binding.id || edge.target_id == binding.id)
        .collect::<Vec<_>>();
    assert_eq!(
        binding_edges.len(),
        2,
        "shadowed closure owner should expose owner and source binding edges: {edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-let-binding edge: {binding_edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingSourceClosure
            && edge.source_id == binding.id
            && edge.target_id == closure_id
            && edge.source_kind == "LocalBinding"
            && edge.target_kind == "Closure"),
        "missing let-binding-to-closure edge: {binding_edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_awaited_future_let_call_result_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_stored_returned_async_closure")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2375-2377:
    // `let future = make_returned_async_closure()(); future.await` proves the
    // stored future binding is sourced by the earlier dynamic call result.
    let context = db.call_context_for_owner(owner)?;
    let dynamic = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(
        dynamic.status.status,
        CallStatusKind::Resolved,
        "stored returned async closure dynamic row should resolve once awaited: {dynamic:#?}"
    );
    assert_eq!(
        dynamic.site.kind,
        CallSiteKind::Dynamic,
        "stored future source should be the dynamic returned-callable callsite"
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let bindings = bindings
        .into_iter()
        .filter(|binding| binding.kind == "LetBinding" && binding.name == "future")
        .collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        1,
        "stored returned async closure owner should expose one future let binding: {bindings:#?}"
    );
    let binding = &bindings[0];
    assert_eq!(binding.source_kind, "DynamicCallResult");
    assert_eq!(
        binding.source_id,
        Some(dynamic.site.id),
        "future binding should point at the stored returned-callable dynamic site"
    );
    assert_eq!(binding.source_call_kind.as_deref(), Some("Dynamic"));
    assert_eq!(binding.source_path, None);
    assert_eq!(
        binding.callee_kind.as_deref(),
        Some("AwaitedReturnedPathCall")
    );
    assert_eq!(
        binding.callee_path.as_ref(),
        Some(&path(&["make_returned_async_closure"]))
    );
    assert!(
        binding.span.0 < binding.span.1,
        "future let binding should retain a non-empty source span: {binding:#?}"
    );

    let edges = db.local_binding_edges_for_owner(owner)?;
    let binding_edges = edges
        .iter()
        .filter(|edge| edge.source_id == binding.id || edge.target_id == binding.id)
        .collect::<Vec<_>>();
    assert_eq!(
        binding_edges.len(),
        2,
        "stored future binding should expose owner and source edges: {edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-future-binding edge: {binding_edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingSourceCallResult
            && edge.source_id == binding.id
            && edge.target_id == dynamic.site.id
            && edge.source_kind == "LocalBinding"
            && edge.target_kind == "Dynamic"),
        "missing future-binding-to-dynamic-call edge: {binding_edges:#?}"
    );

    Ok(())
}
