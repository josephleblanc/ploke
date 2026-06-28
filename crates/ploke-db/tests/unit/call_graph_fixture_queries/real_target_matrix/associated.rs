use super::super::*;
use super::common::*;

#[test]
fn axum_real_target_into_service_future_new_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `IntoServiceFuture::new` generated constructor row.
    // Source chain:
    //   axum/src/handler/future.rs:11-18 defines the generated future type.
    //   axum/src/handler/service.rs:174 calls
    //   `super::future::IntoServiceFuture::new(future)`.
    // Current model gap: the structural path row exists but is unresolved.
    let owner =
        method_id_by_name_and_body_substring(&db, "call", "IntoServiceFuture::new(future)")?;
    let target = method_id_by_name_and_body_substring(&db, "new", "Self { future }")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["super", "future", "IntoServiceFuture", "new"]);

    assert_eq!(row.status.status, CallStatusKind::Unresolved);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "unresolved IntoServiceFuture::new row should not expose traversal targets: {row:#?}"
    );
    assert!(
        db.call_sites_for_target(target)?.is_empty(),
        "IntoServiceFuture::new should remain targetless until generated associated path resolution lands"
    );

    Ok(())
}

#[test]
fn axum_real_target_json_from_bytes_self_paths_are_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = method_id_by_name_and_body_substring(
        &db,
        "from_bytes",
        "serde_json::Deserializer::from_slice(bytes)",
    )?;

    // Matrix: `Json::from_bytes` inherent method row.
    // Source chain:
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:112 and :128 call `Self::from_bytes(&bytes)`.
    // Current model gap: associated-function `Self::...` resolution is not
    // available in the axum fixture, so each structural row remains targetless.
    let owners =
        method_ids_by_name_and_body_substring(&db, "from_request", "Self::from_bytes(&bytes)")?;
    assert_eq!(
        owners.len(),
        2,
        "axum/src/json.rs should expose two from_request owners that call Self::from_bytes"
    );

    for owner in owners {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_kind_path(&context, CallSiteKind::Path, &["Self", "from_bytes"]);
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "unsupported Self::from_bytes row should remain targetless: {row:#?}"
        );
    }

    assert!(
        db.call_sites_for_target(target)?.is_empty(),
        "Json::from_bytes should remain targetless until Self::associated-function resolution lands"
    );

    Ok(())
}

#[test]
fn axum_real_target_handler_service_trait_call_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Handler::call` trait dispatch row.
    // Source chain:
    //   axum/src/handler/mod.rs:153 declares `Handler::call`.
    //   axum/src/handler/service.rs:171 calls `Handler::call(handler, req, state)`.
    // Current model gap: the structural path row is present, but full trait
    // associated-function resolution is not yet modeled for this corpus row.
    let owner = method_id_by_name_and_body_substring(
        &db,
        "call",
        "Handler::call(handler, req, self.state.clone())",
    )?;
    let target = method_id_by_trait_name(&db, "Handler", "call")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["Handler", "call"]);

    assert_eq!(row.status.status, CallStatusKind::Unresolved);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "unresolved Handler::call row should not expose traversal targets: {row:#?}"
    );
    assert!(
        db.call_sites_for_target(target)?.is_empty(),
        "Handler::call should remain targetless until trait associated-function resolution lands"
    );

    Ok(())
}

#[test]
fn axum_real_target_boxed_into_route_explicit_constructor_reaches_struct() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `BoxedIntoRoute` tuple-struct constructor row.
    // Source chain:
    //   axum/src/boxed.rs:12 defines `BoxedIntoRoute<S, E>(...)`.
    //   axum/src/boxed.rs:38 calls `BoxedIntoRoute(Box::new(...))`.
    // Expected traversal: `BoxedIntoRoute::map` -> struct constructor, one call edge.
    let owner = method_id_by_name_and_body_substring(&db, "map", "BoxedIntoRoute(Box::new")?;
    let target = struct_id_by_name(&db, "BoxedIntoRoute")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["BoxedIntoRoute"]);

    assert_resolved_target(
        row,
        target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "BoxedIntoRoute::map -> BoxedIntoRoute",
            owner,
            target,
            site_id: row.site.id,
        },
    )
}

#[test]
fn axum_real_target_handle_error_extension_reaches_constructor() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `HandleError::new` inherent constructor row.
    // Source chain:
    //   axum/src/error_handling/mod.rs:80 defines `HandleError::new`.
    //   axum/src/error_handling/mod.rs:65 calls `HandleError::new(self, f)`.
    // Expected traversal: `HandleErrorExt::handle_error` -> constructor, one call edge.
    let owner =
        method_id_by_name_and_body_substring(&db, "handle_error", "HandleError::new(self, f)")?;
    let target = method_id_by_name_and_body_substring(&db, "new", "Self { inner, f, _extractor")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["HandleError", "new"]);

    assert_resolved_target(
        row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "handle_error -> HandleError::new",
            owner,
            target,
            site_id: row.site.id,
        },
    )
}
