use super::super::*;
use super::common::*;
use super::source_lines::*;
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
    // re-export traversal, and direct imports through the public
    // `test_helpers` glob re-export make 105 projected rows resolve to the
    // gated local test helper target. The remaining 62 rows stay unsupported
    // and targetless because their import evidence is still outside these
    // exact re-export paths.
    let target =
        assert_resolved_path_target_count(&db, &["TestClient", "new"], 105, "TestClient::new")?;
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        105,
        "TestClient::new should expose the resolved real-corpus caller subset: {callers:#?}"
    );
    assert_sites_match_callers(&db, target, &callers, "TestClient::new resolved subset")?;
    assert_targetless_path_rows(&db, &["TestClient", "new"], CallStatusKind::Unsupported, 62)?;

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

    assert_path_module_fanout(
        &db,
        &["TestClient", "new"],
        CallStatusKind::Unsupported,
        &[
            (&["crate", "extract", "request_parts", "tests"], 1),
            (&["crate", "routing", "tests", "fallback"], 25),
            (&["crate", "routing", "tests", "handle_error"], 5),
            (&["crate", "routing", "tests", "merge"], 16),
            (&["crate", "routing", "tests", "nest"], 15),
        ],
    )?;
    // File-level projection oracle for the same high-fanout matrix:
    //   the oracle lists 172 selected-member text callsites. The DB currently
    //   projects 167 structural rows. The absent source rows are
    //   axum/src/extract/multipart.rs:{383,423,449},
    //   one routing/tests/mod.rs row, and one routing/tests/nest.rs row.
    //   The remaining unsupported projected rows are targetless, so traversal
    //   edge count is 0 for this subset.
    assert_path_file_fanout(
        &db,
        &["TestClient", "new"],
        CallStatusKind::Unsupported,
        &[
            ("axum-core/src/extract/request_parts.rs", 1),
            ("axum/src/routing/tests/fallback.rs", 25),
            ("axum/src/routing/tests/handle_error.rs", 5),
            ("axum/src/routing/tests/merge.rs", 16),
            ("axum/src/routing/tests/nest.rs", 15),
        ],
    )?;
    // Source-line projection oracle for the same matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // These projected rows remain unsupported boundaries after the exact
    // nested-glob and direct re-export-import subsets above resolve. The three
    // multipart rows at axum/src/extract/multipart.rs:{383,423,449} are still
    // absent in the current fixture, as are the closure-body row at
    // axum/src/routing/tests/mod.rs:1073 and the macro-template row at
    // axum/src/routing/tests/nest.rs:371.
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["TestClient", "new"],
        CallStatusKind::Unsupported,
        &[
            SourceLineFanout {
                file_suffix: "axum-core/src/extract/request_parts.rs",
                lines: &[193],
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
                file_suffix: "axum/src/routing/tests/nest.rs",
                lines: &[
                    41, 65, 135, 159, 182, 193, 210, 229, 280, 298, 309, 328, 408, 431, 489,
                ],
            },
        ],
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
fn axum_real_target_boxed_into_route_self_constructors_are_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `BoxedIntoRoute` tuple-struct constructor rows.
    // Source chains:
    //   axum/src/boxed.rs:12 defines `BoxedIntoRoute<S, E>(...)`.
    //   axum/src/boxed.rs:23 calls `Self(Box::new(MakeErasedHandler { ... }))`
    //   from `BoxedIntoRoute::from_handler`.
    //   axum/src/boxed.rs:51 calls `Self(self.0.clone_box())` from
    //   `Clone for BoxedIntoRoute::clone`.
    // Current model gap: each `Self(...)` constructor call is structurally
    // visible but unsupported and targetless. The explicit
    // `BoxedIntoRoute(...)` row above is the only current tuple-struct
    // constructor traversal for this target.
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

        assert_targetless_status(row, CallStatusKind::Unsupported);
        assert!(
            relations_for_site(&db, row.site.id)?.rows.is_empty(),
            "{label} should not have persisted call_relation targets"
        );
        assert_no_traversal_candidates_for_site(&db, owner, row.site.id, label)?;
    }

    let callers = db.callers_for_target(target)?;
    assert!(
        callers
            .iter()
            .all(|caller| caller.site.path != Some(path(&["Self"]))),
        "BoxedIntoRoute target callers should not include unsupported Self constructor rows: {callers:#?}"
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
    //   axum/src/boxed.rs:38 calls `BoxedIntoRoute(Box::new(...))`.
    // Expected proof traversal: target-centered projection stores the resolved
    // call_site, call_resolution, and call_edge facts for the real corpus
    // constructor edge without adding blocker facts.
    let owner = method_id_by_name_and_body_substring(&db, "map", "BoxedIntoRoute(Box::new")?;
    let target = struct_id_by_name(&db, "BoxedIntoRoute")?;
    let callers = db.callers_for_target(target)?;
    let expected = assert_proof_site_cases(
        &db,
        &callers,
        &[ProofSiteCase::path(
            owner,
            &["BoxedIntoRoute"],
            CallRelationKind::TupleStructConstructor,
            CallTargetKind::Struct,
        )],
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
    // not projected yet, even though the trait default body reaches
    // `HandleError::new`.
    assert_no_method_rows(&db, "handle_error")
}
