use super::*;
use ploke_core::rag_types::LocalBindingRelationKind;

#[tokio::test]
async fn local_bindings_exact_expose_axum_tap_io_constructor_frontier() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Source oracle:
    //   axum/src/serve/listener.rs:116-123
    //   `tap_io<F>(self, tap_fn: F) -> TapIo<Self, F>` returns
    //   `TapIo { listener: self, tap_fn }`.
    //
    // Contract: exact RAG exposes the constructor-side local-binding frontier
    // without promoting `TapIo::accept`'s `(self.tap_fn)(&mut io)` call into a
    // traversal edge.
    let owner = method_id_by_name_and_body_substring(&db, "tap_io", "TapIo")?;
    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let return_binding = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ReturnExpression"
                && binding.name == "return"
                && binding.source_kind == "Constructed"
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == "TapIo")
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose tap_io constructed return binding: {bindings:#?}")
        });
    let parameter = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "tap_fn"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| panic!("RAG should expose tap_io tap_fn parameter: {bindings:#?}"));
    let projection = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "FieldProjection"
                && binding.name == "return.tap_fn"
                && binding.source_kind == "FieldProjection"
                && binding.source_id == Some(return_binding.id)
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == "tap_fn")
                && binding.callee_kind.as_deref() == Some("Path")
                && matches!(binding.callee_path.as_deref(), Some([segment]) if segment == "tap_fn")
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose tap_io return.tap_fn field projection: {bindings:#?}")
        });

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == return_binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-return containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == parameter.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-parameter containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == projection.id
                && edge.target_id == return_binding.id
                && edge.relation == LocalBindingRelationKind::BindingProjectsField
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose return.tap_fn projection-to-return proof: {edges:#?}"
    );

    Ok(())
}
