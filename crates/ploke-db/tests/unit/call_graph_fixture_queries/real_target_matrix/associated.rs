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
fn axum_real_target_test_client_new_high_fanout_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: high-fanout `TestClient::new`.
    // Source chain:
    //   axum/src/test_helpers/test_client.rs:36 defines `TestClient::new`.
    //   The oracle matrix records 172 selected-member text callsites:
    //   axum-core/src/extract/request_parts.rs:193; axum/src/form.rs:262;
    //   json.rs:{250,266,281,301,320,355};
    //   extract/multipart.rs:{383,423,449};
    //   routing/tests/mod.rs:{90,118,150,188,217,233,242,282,307,323,
    //   339,352,365,377,396,416,454,472,489,506,527,540,572,589,599,
    //   626,643,668,685,700,717,738,748,775,798,815,846,905,952,967,
    //   984,1027,1047,1073,1164,1201}; plus the other file groups
    //   listed in the oracle matrix.
    // Current DB contract: 167 structural `TestClient::new` path rows are
    // projected in the corpus fixture, but they are unsupported and targetless.
    // The five source rows not represented in the DB fanout are the three
    // multipart rows plus one routing/tests/mod.rs row and one
    // routing/tests/nest.rs row; no local traversal edge should be fabricated.
    assert_targetless_path_rows(
        &db,
        &["TestClient", "new"],
        CallStatusKind::Unsupported,
        167,
    )?;
    assert_path_module_fanout(
        &db,
        &["TestClient", "new"],
        CallStatusKind::Unsupported,
        &[
            (&["crate", "extension", "tests"], 1),
            (&["crate", "extract", "connect_info", "tests"], 1),
            (&["crate", "extract", "matched_path", "tests"], 14),
            (&["crate", "extract", "nested_path", "tests"], 6),
            (&["crate", "extract", "path", "tests"], 19),
            (&["crate", "extract", "query", "tests"], 1),
            (&["crate", "extract", "request_parts", "tests"], 1),
            (&["crate", "extract", "tests"], 1),
            (&["crate", "form", "tests"], 1),
            (&["crate", "handler", "tests"], 2),
            (&["crate", "json", "tests"], 6),
            (&["crate", "middleware", "from_extractor", "tests"], 1),
            (&["crate", "middleware", "map_request", "tests"], 2),
            (&["crate", "middleware", "map_response", "tests"], 1),
            (&["crate", "response", "sse", "tests"], 3),
            (&["crate", "response", "tests"], 1),
            (&["crate", "routing", "tests"], 45),
            (&["crate", "routing", "tests", "fallback"], 25),
            (&["crate", "routing", "tests", "handle_error"], 5),
            (&["crate", "routing", "tests", "merge"], 16),
            (&["crate", "routing", "tests", "nest"], 15),
        ],
    )?;

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

    let target = method_id_by_name_and_body_substring(&db, "new", "Self { inner, f, _extractor")?;

    // Matrix: `HandleError::new` inherent constructor rows.
    // Source chains:
    //   axum/src/error_handling/mod.rs:80 defines `HandleError::new`.
    //   axum/src/error_handling/mod.rs:65 calls
    //   `HandleError::new(inner, self.f.clone())` from `Layer::layer`.
    //   axum/src/service_ext.rs:43 calls `HandleError::new(self, f)` from the
    //   `ServiceExt::handle_error` trait default method.
    // Expected traversal: each owner reaches the constructor in one call edge.
    let cases = [
        (
            "Layer::layer -> HandleError::new",
            method_id_by_name_and_body_substring(
                &db,
                "layer",
                "HandleError::new(inner, self.f.clone())",
            )?,
        ),
        (
            "ServiceExt::handle_error -> HandleError::new",
            method_id_by_trait_name(&db, "ServiceExt", "handle_error")?,
        ),
    ];

    for (label, owner) in cases {
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
                label,
                owner,
                target,
                site_id: row.site.id,
            },
        )?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        2,
        "HandleError::new should expose both resolved constructor callers: {callers:#?}"
    );

    // Matrix: user-facing service-extension dispatch.
    // Source chain:
    //   axum/src/routing/tests/handle_error.rs:86 calls
    //   `fallible_service.handle_error(...)`.
    // Current model gap: the user-facing `.handle_error(...)` receiver row is
    // not projected yet, even though the trait default body reaches
    // `HandleError::new`.
    assert_no_method_rows(&db, "handle_error")
}
