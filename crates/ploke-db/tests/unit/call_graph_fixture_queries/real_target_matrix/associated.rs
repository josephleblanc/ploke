use super::super::*;
use super::common::*;
use super::source_lines::{SourceLineFanout, assert_resolved_path_line_fanout};
use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;

#[test]
fn axum_real_target_into_service_future_new_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `IntoServiceFuture::new` generated constructor row.
    // Source chain:
    //   axum/src/handler/future.rs:11-18 defines the generated future type.
    //   axum/src/macros.rs:19-20 contains the macro template that would
    //   generate the inherent `new` constructor after expansion.
    //   axum/src/handler/service.rs:155 binds
    //   `type Future = super::future::IntoServiceFuture<H::Future>`.
    //   axum/src/handler/service.rs:174 calls
    //   `super::future::IntoServiceFuture::new(future)`.
    // Current model gap: the structural path row exists, but the parser does
    // not expand `opaque_future!`, so there is no concrete generated
    // `IntoServiceFuture::new` method node to traverse to.
    let owner =
        method_id_by_name_and_body_substring(&db, "call", "IntoServiceFuture::new(future)")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["super", "future", "IntoServiceFuture", "new"]);

    assert_eq!(row.status.status, CallStatusKind::Unresolved);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "unresolved IntoServiceFuture::new row should not expose traversal targets: {row:#?}"
    );
    assert!(
        relations_for_site(&db, row.site.id)?.rows.is_empty(),
        "IntoServiceFuture::new structural row should have zero persisted call edges"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        owner,
        row.site.id,
        "axum/src/handler/service.rs:174 IntoServiceFuture::new",
    )?;

    Ok(())
}

#[test]
fn axum_exact_trait_impl_lookup_disambiguates_handler_service_call() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let service_file = file_path_by_suffix(&db, "axum/src/handler/service.rs")?;
    let module_path = vec![
        "crate".to_string(),
        "handler".to_string(),
        "service".to_string(),
    ];

    // Source oracle:
    //   axum/src/handler/service.rs:146 defines
    //   `impl Service<Request<B>> for HandlerService`.
    //   axum/src/handler/service.rs:165 is the request-handling `call`.
    //   axum/src/handler/service.rs:183 defines another
    //   `impl Service<serve::IncomingStream<'_, L>> for HandlerService`.
    // Expected exact lookup behavior: a trait + self-type qualifier alone keeps
    // both Service impl methods visible, while the `Request` trait-input root
    // selects the owner that contains the generated `IntoServiceFuture::new`
    // frontier row.
    let ambiguous = ploke_db::helpers::graph_resolve_exact_trait_impl_method(
        &db,
        &service_file,
        &module_path,
        "call",
        "Service",
        "HandlerService",
        None,
    )?;
    assert_eq!(
        ambiguous.len(),
        2,
        "plain Service for HandlerService should keep both call overloads visible"
    );

    let rows = ploke_db::helpers::graph_resolve_exact_trait_impl_method(
        &db,
        &service_file,
        &module_path,
        "call",
        "Service",
        "HandlerService",
        Some("Request"),
    )?;
    assert_eq!(
        rows.len(),
        1,
        "Service<Request> for HandlerService should select the request handler call"
    );
    assert_eq!(rows[0].name, "call");
    assert_eq!(rows[0].file_path, service_file);
    assert!(
        rows[0].start_byte < rows[0].end_byte,
        "trait impl method span should be non-empty"
    );

    Ok(())
}

#[test]
fn axum_real_target_json_from_bytes_self_paths_reach_inherent_method() -> Result<(), DbError> {
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
    // Expected traversal: both trait impl methods for `Json<T>` reach the
    // inherent `Json::from_bytes` method in one local-exact associated-function
    // edge.
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
                label: "axum/src/json.rs:112 or :128 Self::from_bytes",
                owner,
                target,
                site_id: row.site.id,
                expected_edge_count: 1,
            },
        )?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        2,
        "Json::from_bytes should expose both resolved Self::from_bytes callers: {callers:#?}"
    );
    assert_sites_match_callers(&db, target, &callers, "Json::from_bytes callers")?;

    Ok(())
}

#[test]
fn axum_real_target_handler_service_trait_call_reaches_trait_method() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Handler::call` trait dispatch row.
    // Source chain:
    //   axum/src/handler/mod.rs:153 declares `Handler::call`.
    //   axum/src/handler/service.rs:171 calls `Handler::call(handler, req, state)`.
    // Expected traversal: path-style trait method dispatch resolves directly
    // to the trait method binding in one persisted call edge. The concrete
    // runtime impl remains type-parameter dependent and is not guessed here.
    let owner = method_id_by_name_and_body_substring(
        &db,
        "call",
        "Handler::call(handler, req, self.state.clone())",
    )?;
    let target = method_id_by_trait_name(&db, "Handler", "call")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["Handler", "call"]);

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
            label: "axum/src/handler/service.rs:171 Handler::call",
            owner,
            target,
            site_id: row.site.id,
            expected_edge_count: 1,
        },
    )?;
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        1,
        "Handler::call should expose the inspected real-corpus caller: {callers:#?}"
    );
    assert_sites_match_callers(&db, target, &callers, "Handler::call real-corpus caller")?;
    caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, &["Handler", "call"]);

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
    // Current DB contract: recursive `cfg(test)` inclusion, nested glob
    // re-export traversal, inherited parent glob imports, and direct imports
    // through the public `test_helpers` glob re-export, plus the axum-core
    // workspace dependency glob import, make 168 projected structural rows
    // resolve to the gated local test helper target.
    let target =
        assert_resolved_path_target_count(&db, &["TestClient", "new"], 168, "TestClient::new")?;
    let line_target = assert_resolved_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["TestClient", "new"],
        CallRelationKind::AssociatedFunction,
        CallTargetKind::Method,
        &[
            SourceLineFanout {
                file_suffix: "axum-core/src/extract/request_parts.rs",
                lines: &[193],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extension.rs",
                lines: &[228],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/connect_info.rs",
                lines: &[386],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/matched_path.rs",
                lines: &[
                    162, 178, 197, 217, 237, 254, 271, 291, 312, 326, 346, 361, 374, 394,
                ],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/mod.rs",
                lines: &[103],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/nested_path.rs",
                lines: &[136, 154, 172, 190, 205, 224],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/path/mod.rs",
                lines: &[
                    619, 632, 645, 664, 687, 700, 716, 732, 751, 784, 798, 822, 854, 912, 946, 974,
                    989, 1010, 1034,
                ],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/query.rs",
                lines: &[158],
            },
            SourceLineFanout {
                file_suffix: "axum/src/form.rs",
                lines: &[262],
            },
            SourceLineFanout {
                file_suffix: "axum/src/handler/mod.rs",
                lines: &[418, 443],
            },
            SourceLineFanout {
                file_suffix: "axum/src/json.rs",
                lines: &[250, 266, 281, 301, 320, 355],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/from_extractor.rs",
                lines: &[351],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/map_request.rs",
                lines: &[412, 432],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/map_response.rs",
                lines: &[357],
            },
            SourceLineFanout {
                file_suffix: "axum/src/response/mod.rs",
                lines: &[529],
            },
            SourceLineFanout {
                file_suffix: "axum/src/response/sse.rs",
                lines: &[714, 756, 793],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/fallback.rs",
                lines: &[
                    10, 25, 40, 53, 69, 89, 101, 118, 134, 150, 171, 190, 207, 221, 241, 261, 280,
                    299, 314, 325, 338, 359, 377, 389, 402,
                ],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/handle_error.rs",
                lines: &[25, 42, 60, 76, 90],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/merge.rs",
                lines: &[
                    14, 63, 81, 85, 96, 116, 136, 150, 162, 179, 208, 234, 267, 301, 345, 379,
                ],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/mod.rs",
                lines: &[
                    90, 118, 150, 188, 217, 233, 242, 282, 307, 323, 339, 352, 365, 377, 396, 416,
                    454, 472, 489, 506, 527, 540, 572, 589, 599, 626, 643, 668, 685, 700, 717, 738,
                    748, 775, 798, 815, 846, 905, 952, 967, 984, 1027, 1047, 1073, 1164, 1201,
                ],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/nest.rs",
                lines: &[
                    41, 65, 135, 159, 182, 193, 210, 229, 280, 298, 309, 328, 408, 431, 489,
                ],
            },
        ],
    )?;
    assert_eq!(line_target, target);
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        168,
        "TestClient::new should expose every resolved projected real-corpus caller: {callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "TestClient::new resolved projected callers",
    )?;
    assert_targetless_path_rows(&db, &["TestClient", "new"], CallStatusKind::Unsupported, 0)?;

    // Matrix immediate candidate:
    //   axum/src/json.rs:237 imports `test_helpers::*`.
    //   axum/src/json.rs:250 calls `TestClient::new(app)` from
    //   `deserialize_body`.
    // Expected traversal: nested glob-reexport visibility resolves the path
    // call to axum/src/test_helpers/test_client.rs:36 in one local-exact
    // associated-function edge.
    let json_owner =
        function_id_by_name_in_module(&db, &["crate", "json", "tests"], "deserialize_body")?;
    let json_context = db.call_context_for_owner(json_owner)?;
    let json_row = row_by_path(&json_context, &["TestClient", "new"]);
    assert_resolved_target(
        json_row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/json.rs:250 glob-import TestClient::new",
            owner: json_owner,
            target,
            site_id: json_row.site.id,
            expected_edge_count: 1,
        },
    )?;

    // Matrix inherited-parent-glob candidate:
    //   axum/src/routing/tests/mod.rs:8-11 imports `crate::test_helpers::*`.
    //   axum/src/routing/tests/fallback.rs:6 imports `super::*`.
    //   axum/src/routing/tests/fallback.rs:10 calls `TestClient::new(app)`.
    // Expected traversal: child module inherited glob visibility resolves the
    // associated-function path in one edge to test_client.rs:36.
    let fallback_owner =
        function_id_by_name_in_module(&db, &["crate", "routing", "tests", "fallback"], "basic")?;
    let fallback_context = db.call_context_for_owner(fallback_owner)?;
    let fallback_row = row_by_path(&fallback_context, &["TestClient", "new"]);
    assert_resolved_target(
        fallback_row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/routing/tests/fallback.rs:10 inherited-glob TestClient::new",
            owner: fallback_owner,
            target,
            site_id: fallback_row.site.id,
            expected_edge_count: 1,
        },
    )?;

    // Matrix cross-crate workspace dependency glob candidate:
    //   axum-core/src/extract/request_parts.rs:177 imports
    //   `axum::{..., test_helpers::*, Router}`.
    //   axum-core/src/extract/request_parts.rs:193 calls
    //   `TestClient::new(...)`.
    // Expected traversal: the dependency-root glob import resolves through the
    // parsed axum workspace member to test_client.rs:36 in one edge.
    let axum_core_owner = function_id_by_name_in_module(
        &db,
        &["crate", "extract", "request_parts", "tests"],
        "extract_request_parts",
    )?;
    let axum_core_context = db.call_context_for_owner(axum_core_owner)?;
    let axum_core_row = row_by_path(&axum_core_context, &["TestClient", "new"]);
    assert_resolved_target(
        axum_core_row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-core/src/extract/request_parts.rs:193 workspace-glob TestClient::new",
            owner: axum_core_owner,
            target,
            site_id: axum_core_row.site.id,
            expected_edge_count: 1,
        },
    )?;

    Ok(())
}

#[test]
fn axum_real_target_test_client_new_direct_grouped_import_resolves() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: axum/src/form.rs:262 calls `TestClient::new(app)`.
    // Source chain:
    //   axum/src/form.rs:136-140 imports
    //   `crate::{ routing::{...}, test_helpers::TestClient, Router }`.
    //   axum/src/test_helpers/mod.rs re-exports `test_client::TestClient`.
    //   axum/src/test_helpers/test_client.rs:36 defines `TestClient::new`.
    // Expected traversal: the grouped direct import should resolve this
    // associated-function path in one local-exact edge.
    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "form", "tests"],
        "deserialize_error_status_codes",
    )?;
    let target = method_id_by_name_body_and_file_suffix(
        &db,
        "new",
        "spawn_service(svc)",
        "axum/src/test_helpers/test_client.rs",
    )?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["TestClient", "new"]);

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
            label: "axum/src/form.rs:262 grouped-import TestClient::new",
            owner,
            target,
            site_id: row.site.id,
            expected_edge_count: 1,
        },
    )
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
            expected_edge_count: 1,
        },
    )
}

#[test]
fn axum_real_target_boxed_into_route_self_constructors_reach_struct() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `BoxedIntoRoute` tuple-struct constructor rows.
    // Source chains:
    //   axum/src/boxed.rs:12 defines `BoxedIntoRoute<S, E>(...)`.
    //   axum/src/boxed.rs:23 calls `Self(Box::new(MakeErasedHandler { ... }))`
    //   from `BoxedIntoRoute::from_handler`.
    //   axum/src/boxed.rs:51 calls `Self(self.0.clone_box())` from
    //   `Clone for BoxedIntoRoute::clone`.
    // Expected traversal after the regenerated axum fixture: both `Self(...)`
    // constructor rows resolve through the enclosing impl self type to the
    // `BoxedIntoRoute` tuple struct.
    let target = struct_id_by_name(&db, "BoxedIntoRoute")?;
    let cases = [
        (
            "axum/src/boxed.rs:23 BoxedIntoRoute::from_handler -> Self constructor",
            method_id_by_name_and_body_substring(&db, "from_handler", "Self(Box::new")?,
        ),
        (
            "axum/src/boxed.rs:51 BoxedIntoRoute::clone -> Self constructor",
            method_id_by_name_body_and_file_suffix(
                &db,
                "clone",
                "Self(self.0.clone_box())",
                "axum/src/boxed.rs",
            )?,
        ),
    ];

    for (label, owner) in cases {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_path(&context, &["Self"]);

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
                label,
                owner,
                target,
                site_id: row.site.id,
                expected_edge_count: 1,
            },
        )?;
    }

    let callers = db.callers_for_target(target)?;
    let mut path_counts = std::collections::BTreeMap::new();
    for caller in &callers {
        *path_counts
            .entry(
                caller
                    .site
                    .path
                    .clone()
                    .expect("BoxedIntoRoute constructor caller should carry a path"),
            )
            .or_insert(0usize) += 1;
    }
    assert_eq!(
        path_counts,
        std::collections::BTreeMap::from([(path(&["BoxedIntoRoute"]), 1), (path(&["Self"]), 2),]),
        "BoxedIntoRoute target callers should include explicit and Self constructor rows"
    );
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "BoxedIntoRoute supported constructor callers",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_boxed_into_route_constructor_projects_proof_facts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix proof bridge:
    //   axum/src/boxed.rs:12 defines `BoxedIntoRoute<S, E>(...)`.
    //   axum/src/boxed.rs:23,38,51 call `Self(...)`,
    //   `BoxedIntoRoute(...)`, and `Self(...)`.
    // Expected proof traversal: target-centered projection stores the resolved
    // call_site, call_resolution, and call_edge facts for all three real
    // corpus constructor edges without adding blocker facts.
    let from_handler = method_id_by_name_and_body_substring(&db, "from_handler", "Self(Box::new")?;
    let map = method_id_by_name_and_body_substring(&db, "map", "BoxedIntoRoute(Box::new")?;
    let clone = method_id_by_name_body_and_file_suffix(
        &db,
        "clone",
        "Self(self.0.clone_box())",
        "axum/src/boxed.rs",
    )?;
    let target = struct_id_by_name(&db, "BoxedIntoRoute")?;
    let callers = db.callers_for_target(target)?;
    let expected = assert_proof_site_cases(
        &db,
        &callers,
        &[
            ProofSiteCase::path(
                from_handler,
                &["Self"],
                CallRelationKind::TupleStructConstructor,
                CallTargetKind::Struct,
            ),
            ProofSiteCase::path(
                map,
                &["BoxedIntoRoute"],
                CallRelationKind::TupleStructConstructor,
                CallTargetKind::Struct,
            ),
            ProofSiteCase::path(
                clone,
                &["Self"],
                CallRelationKind::TupleStructConstructor,
                CallTargetKind::Struct,
            ),
        ],
    )?;

    assert_target_proof_projection(
        &db,
        "real corpus BoxedIntoRoute constructor",
        "bd:corpus-axum-call-graph",
        target,
        &callers,
        &expected,
        "axum/src/boxed.rs",
        "type_resolution_missing",
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
                expected_edge_count: 1,
            },
        )?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        2,
        "HandleError::new should expose both resolved constructor callers: {callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "HandleError::new constructor callers",
    )?;

    // Matrix: user-facing service-extension dispatch.
    // Source chain:
    //   axum/src/routing/tests/handle_error.rs:86 calls
    //   `fallible_service.handle_error(...)`.
    // Current model gap: the user-facing `.handle_error(...)` receiver row is
    // visible but remains unsupported/targetless; the trait default body above
    // still reaches `HandleError::new`.
    let service_ext_owner = function_id_by_name_in_module(
        &db,
        &["crate", "routing", "tests", "handle_error"],
        "handler_service_ext",
    )?;
    let service_ext_context = db.call_context_for_owner(service_ext_owner)?;
    let service_ext_rows = service_ext_context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some("handle_error")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        service_ext_rows.len(),
        1,
        "handler_service_ext should expose one targetless .handle_error(...) row: {service_ext_context:#?}"
    );
    let service_ext_row = service_ext_rows[0];
    assert_targetless_status(service_ext_row, CallStatusKind::Unsupported);
    assert!(
        relations_for_site(&db, service_ext_row.site.id)?
            .rows
            .is_empty(),
        "user-facing .handle_error(...) row should have zero persisted call edges"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        service_ext_owner,
        service_ext_row.site.id,
        "axum/src/routing/tests/handle_error.rs:86 fallible_service.handle_error",
    )
}
