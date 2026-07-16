use super::*;
use ploke_core::rag_types::{CallSiteKind, CallStatusKind, LocalBindingRelationKind};

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

#[tokio::test]
async fn self_field_parameter_flows_exact_expose_axum_tap_io_accept_frontier() -> Result<(), Error>
{
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Source oracle:
    //   axum/src/serve/listener.rs:116-123
    //     `tap_io<F>(self, tap_fn: F) -> TapIo<Self, F>` returns
    //     `TapIo { listener: self, tap_fn }`.
    //   axum/src/serve/listener.rs:236
    //     `TapIo::accept` later calls `(self.tap_fn)(&mut io)`.
    //
    // Contract: exact RAG exposes the constructor-parameter source for the
    // targetless `self.tap_fn` dynamic call without promoting it into a local
    // traversal edge.
    let constructor = method_id_by_name_and_body_substring(&db, "tap_io", "TapIo")?;
    let accept = method_id_by_name_and_body_substring(&db, "accept", "(self.tap_fn)(&mut io)")?;

    let flows = rag
        .exact_self_field_parameter_flows_for_owner(accept)?
        .expect("call context is enabled");
    assert_eq!(
        flows.len(),
        1,
        "RAG should expose one TapIo::accept self-field parameter flow: {flows:#?}"
    );
    let flow = &flows[0];
    assert_eq!(flow.site.owner_id, accept);
    assert_eq!(flow.site.kind, CallSiteKind::Dynamic);
    assert_eq!(
        flow.site.path.as_deref(),
        Some(&["self".to_string(), "tap_fn".to_string()][..])
    );
    assert_eq!(flow.site.status, CallStatusKind::Unsupported);
    assert!(flow.site.targets.is_empty());
    assert_eq!(flow.constructor_id, constructor);
    assert_eq!(flow.return_binding.kind, "ReturnExpression");
    assert_eq!(flow.return_binding.source_kind, "Constructed");
    assert_eq!(flow.field_binding.kind, "FieldProjection");
    assert_eq!(flow.field_binding.source_id, Some(flow.return_binding.id));
    assert_eq!(
        flow.field_binding.source_path.as_deref(),
        Some(&["tap_fn".to_string()][..])
    );
    assert_eq!(flow.parameter_binding.kind, "ParameterBinding");
    assert_eq!(flow.parameter_binding.name, "tap_fn");
    assert_eq!(flow.parameter_binding.source_kind, "Parameter");

    Ok(())
}

#[tokio::test]
async fn local_bindings_exact_expose_axum_handle_error_returned_future_producer()
-> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Source oracle:
    //   axum/src/error_handling/mod.rs:140 creates
    //   `let future = Box::pin(async move { ... })`.
    //   axum/src/error_handling/mod.rs:147 returns
    //   `future::HandleErrorFuture { future }`.
    //   axum/src/error_handling/mod.rs:251 later calls
    //   `self.project().future.poll(cx)`.
    //
    // Contract: exact RAG exposes the producer-side returned-field binding
    // without promoting the later dyn Future::poll dispatch into a local edge.
    let owner = method_id_by_name_and_body_substring(
        &db,
        "call",
        "Err(err) => Ok(f(err).await.into_response())",
    )?;
    let context = rag.exact_call_context(owner)?;
    let box_pin = context
        .iter()
        .find(|row| {
            row.owner_id == owner
                && row.kind == CallSiteKind::Path
                && row
                    .path
                    .as_deref()
                    .is_some_and(|path| path.iter().map(String::as_str).eq(["Box", "pin"]))
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose HandleError::call Box::pin site: {context:#?}")
        });

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
                && binding.source_path.as_deref().is_some_and(|path| {
                    path.iter()
                        .map(String::as_str)
                        .eq(["future", "HandleErrorFuture"])
                })
        })
        .unwrap_or_else(|| {
            panic!(
                "RAG should expose HandleError::call constructed returned future binding: {bindings:#?}"
            )
        });
    let future_field = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LetBinding"
                && binding.name == "return.future"
                && binding.source_kind == "PathCallResult"
                && binding.source_id == Some(box_pin.site_id)
                && binding.source_call_kind.as_deref() == Some("Path")
                && binding
                    .source_path
                    .as_deref()
                    .is_some_and(|path| path.iter().map(String::as_str).eq(["Box", "pin"]))
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        })
        .unwrap_or_else(|| {
            panic!(
                "RAG should expose HandleError::call return.future sourced by Box::pin: {bindings:#?}"
            )
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
        "RAG should expose owner-to-return binding containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == future_field.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-return.future binding containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == future_field.id
                && edge.target_id == box_pin.site_id
                && edge.relation == LocalBindingRelationKind::BindingSourceCallResult
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "Path"
        }),
        "RAG should expose return.future-to-Box::pin source proof: {edges:#?}"
    );

    Ok(())
}
