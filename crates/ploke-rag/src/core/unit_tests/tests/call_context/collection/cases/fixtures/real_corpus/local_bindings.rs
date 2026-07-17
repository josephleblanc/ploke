use super::*;
use ploke_core::rag_types::{
    CallCalleeInfo, CallReceiverInfo, CallSiteKind, CallStatusKind, LocalBindingRelationKind,
};

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
async fn local_bindings_exact_expose_memchr_runner_setter_assignment() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_memchr_call_graph_rag()?;

    // Source oracle:
    //   memchr/src/tests/substring/mod.rs:133-138 stores the `search`
    //   parameter into `self.fwd` through `Some(Box::new(search))`.
    //
    // Contract: exact RAG exposes the setter-side field-assignment source
    // evidence without resolving the later boxed `dyn FnMut` call in
    // `Runner::run`.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "fwd",
        "self.fwd = Some(Box::new(search));",
        "src/tests/substring/mod.rs",
    )?;
    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let parameter = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "search"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose memchr Runner::fwd search parameter: {bindings:#?}")
        });
    let assignment = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "FieldAssignment"
                && binding.name == "self.fwd"
                && binding.source_kind == "SelfFieldAssignment"
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == "fwd")
                && binding.callee_kind.as_deref() == Some("Path")
                && matches!(binding.callee_path.as_deref(), Some([segment]) if segment == "search")
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose memchr Runner::fwd self-field assignment: {bindings:#?}")
        });

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == assignment.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-field assignment containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == assignment.id
                && edge.target_id == parameter.id
                && edge.relation == LocalBindingRelationKind::BindingSourceParameter
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose memchr self-field assignment-to-parameter proof: {edges:#?}"
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

#[tokio::test]
async fn future_poll_field_producer_flows_exact_expose_axum_handle_error_poll_frontier()
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
    // Contract: exact RAG surfaces a proof payload tying the targetless dyn
    // Future::poll frontier back to its producer-side returned field. This is
    // proof context, not a promoted traversal edge.
    let producer = method_id_by_name_and_body_substring(
        &db,
        "call",
        "Err(err) => Ok(f(err).await.into_response())",
    )?;
    let poll_owner = method_id_by_file(
        &db,
        "poll",
        "self.project().future.poll(cx)",
        "axum/src/error_handling/mod.rs",
    )?;

    let flows = rag
        .exact_future_poll_field_producer_flows_for_owner(poll_owner)?
        .expect("call context is enabled");
    assert_eq!(
        flows.len(),
        1,
        "RAG should expose one HandleErrorFuture::poll producer flow: {flows:#?}"
    );
    let flow = &flows[0];
    assert_eq!(flow.site.owner_id, poll_owner);
    assert_eq!(flow.site.kind, CallSiteKind::Method);
    assert_eq!(flow.site.status, CallStatusKind::Unsupported);
    assert!(flow.site.targets.is_empty());
    assert!(
        matches!(
            &flow.site.callee,
            CallCalleeInfo::Method {
                name,
                receiver: Some(CallReceiverInfo::MethodResultField {
                    method_name,
                    field_path,
                    ..
                }),
            } if name == "poll"
                && method_name == "project"
                && field_path.iter().map(String::as_str).eq(["future"])
        ),
        "RAG should preserve the project().future poll receiver: {flow:#?}"
    );
    assert_eq!(flow.poll_owner_type, "HandleErrorFuture");
    assert_eq!(flow.producer_id, producer);
    assert_eq!(flow.return_binding.owner_id, producer);
    assert_eq!(flow.return_binding.kind, "ReturnExpression");
    assert_eq!(flow.return_binding.source_kind, "Constructed");
    assert!(
        flow.return_binding
            .source_path
            .as_deref()
            .is_some_and(|path| path
                .iter()
                .map(String::as_str)
                .eq(["future", "HandleErrorFuture"]))
    );
    assert_eq!(flow.field_binding.owner_id, producer);
    assert_eq!(flow.field_binding.kind, "LetBinding");
    assert_eq!(flow.field_binding.name, "return.future");
    assert_eq!(flow.field_binding.source_kind, "PathCallResult");
    assert_eq!(flow.field_binding.source_id, Some(flow.source_site.site_id));
    assert_eq!(flow.source_site.owner_id, producer);
    assert_eq!(flow.source_site.kind, CallSiteKind::Path);
    assert!(
        flow.source_site
            .path
            .as_deref()
            .is_some_and(|path| path.iter().map(String::as_str).eq(["Box", "pin"]))
    );
    assert_eq!(
        flow.source_edge.relation,
        LocalBindingRelationKind::BindingSourceCallResult
    );
    assert_eq!(flow.source_edge.source_id, flow.field_binding.id);
    assert_eq!(flow.source_edge.target_id, flow.source_site.site_id);

    let context = rag.exact_call_context(poll_owner)?;
    let poll_site = context
        .iter()
        .find(|site| site.site_id == flow.site.site_id)
        .unwrap_or_else(|| panic!("RAG should expose poll site in call context: {context:#?}"));
    assert!(
        poll_site.targets.is_empty(),
        "RAG must not fabricate a dyn Future::poll target from producer proof: {poll_site:#?}"
    );

    Ok(())
}
