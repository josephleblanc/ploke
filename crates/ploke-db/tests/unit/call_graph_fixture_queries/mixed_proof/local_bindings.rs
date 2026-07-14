use super::*;
use ploke_db::LocalBindingRelationKind;

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
