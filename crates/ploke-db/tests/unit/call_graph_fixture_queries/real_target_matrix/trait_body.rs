use super::super::*;
use super::common::*;
use super::source_lines::*;
use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;

#[test]
fn axum_real_target_trait_associated_paths_reach_trait_methods() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: trait-associated extraction calls.
    // Source chain:
    //   axum-core/src/ext_traits/request.rs:279 calls `E::from_request`.
    //   axum-core/src/extract/mod.rs:127 calls `T::from_request`.
    //   axum-core/src/ext_traits/request.rs:305 and
    //   ext_traits/request_parts.rs:133 call `E::from_request_parts`.
    //   axum-core/src/extract/mod.rs:115 calls `T::from_request_parts`.
    // Intermediate bindings:
    //   axum-core/src/extract/mod.rs:79 declares trait `FromRequest`.
    //   axum-core/src/extract/mod.rs:85 declares `FromRequest::from_request`.
    //   axum-core/src/extract/mod.rs:53 declares trait `FromRequestParts`.
    //   axum-core/src/extract/mod.rs:59 declares
    //   `FromRequestParts::from_request_parts`.
    // Expected traversal: bounded type-parameter associated paths resolve to
    // the trait method binding in one local-exact associated-function edge.
    // Concrete runtime impl dispatch remains type-parameter dependent and is
    // not guessed by this query.
    let from_request = method_id_by_trait_name(&db, "FromRequest", "from_request")?;
    let from_request_parts =
        method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?;

    let cases = [
        (
            "axum-core/src/ext_traits/request.rs:279",
            method_id_by_name_body_and_file_suffix(
                &db,
                "extract_with_state",
                "E::from_request(self, state)",
                "axum-core/src/ext_traits/request.rs",
            )?,
            &["E", "from_request"][..],
            from_request,
        ),
        (
            "axum-core/src/extract/mod.rs:127",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request",
                "T::from_request(req, state).await",
                "axum-core/src/extract/mod.rs",
            )?,
            &["T", "from_request"][..],
            from_request,
        ),
        (
            "axum-core/src/ext_traits/request.rs:305",
            method_id_by_name_body_and_file_suffix(
                &db,
                "extract_parts_with_state",
                "E::from_request_parts(&mut parts, state).await",
                "axum-core/src/ext_traits/request.rs",
            )?,
            &["E", "from_request_parts"][..],
            from_request_parts,
        ),
        (
            "axum-core/src/extract/mod.rs:115",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request_parts",
                "T::from_request_parts(parts, state).await",
                "axum-core/src/extract/mod.rs",
            )?,
            &["T", "from_request_parts"][..],
            from_request_parts,
        ),
        (
            "axum-core/src/ext_traits/request_parts.rs:133",
            method_id_by_name_body_and_file_suffix(
                &db,
                "extract_with_state",
                "E::from_request_parts(self, state)",
                "axum-core/src/ext_traits/request_parts.rs",
            )?,
            &["E", "from_request_parts"][..],
            from_request_parts,
        ),
    ];
    for (label, owner, path, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_path(&context, path);
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

    let from_request_callers = db.callers_for_target(from_request)?;
    assert_eq!(
        from_request_callers.len(),
        2,
        "FromRequest::from_request should expose both inspected bounded callers: {from_request_callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        from_request,
        &from_request_callers,
        "FromRequest::from_request real-corpus callers",
    )?;

    let from_request_parts_callers = db.callers_for_target(from_request_parts)?;
    assert_eq!(
        from_request_parts_callers.len(),
        3,
        "FromRequestParts::from_request_parts should expose all inspected bounded callers: {from_request_parts_callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        from_request_parts,
        &from_request_parts_callers,
        "FromRequestParts::from_request_parts real-corpus callers",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_from_ref_same_crate_paths_reach_trait_method() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: same-crate `FromRef::from_ref` bounded calls.
    // Source chain:
    //   axum-core/src/ext_traits/mod.rs:25 calls
    //   `InnerState::from_ref(state)`.
    //   axum-core/src/ext_traits/mod.rs:45 calls `String::from_ref(state)`.
    // Intermediate binding:
    //   axum-core/src/extract/from_ref.rs:13 declares trait `FromRef`.
    //   axum-core/src/extract/from_ref.rs:15 declares `FromRef::from_ref`.
    // Expected traversal: explicit where-predicate bounds such as
    // `InnerState: FromRef<OuterState>` and `String: FromRef<S>` resolve to
    // the trait method binding in one local-exact associated-function edge.
    // Concrete runtime impl dispatch remains type-dependent and is not guessed.
    let target = method_id_by_trait_name(&db, "FromRef", "from_ref")?;

    let cases = [
        (
            "axum-core/src/ext_traits/mod.rs:25",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request_parts",
                "InnerState::from_ref(state)",
                "axum-core/src/ext_traits/mod.rs",
            )?,
            &["InnerState", "from_ref"][..],
            target,
        ),
        (
            "axum-core/src/ext_traits/mod.rs:45",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request_parts",
                "String::from_ref(state)",
                "axum-core/src/ext_traits/mod.rs",
            )?,
            &["String", "from_ref"][..],
            target,
        ),
    ];
    for (label, owner, path, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_path(&context, path);
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
        "FromRef::from_ref should expose the two same-crate bounded associated-path callers: {callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "FromRef::from_ref same-crate real-corpus callers",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_from_ref_dependency_root_bounds_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: dependency-root `FromRef::from_ref` bounded calls.
    // Source chain:
    //   axum/src/extract/state.rs:1 imports `axum_core::extract::FromRef`.
    //   axum/src/extract/state.rs:309 calls `InnerState::from_ref(state)`.
    //   axum/src/middleware/from_extractor.rs:306 imports
    //   `axum_core::extract::FromRef`.
    //   axum/src/middleware/from_extractor.rs:328 calls
    //   `Secret::from_ref(state)` from a test function.
    // Current model gap: type resolution treats dependency roots as external
    // even when the same fixture also parsed the dependency crate. These rows
    // stay visible, unsupported, and targetless rather than guessing that
    // `axum_core::extract::FromRef` is the local axum-core trait node.
    let state_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "from_request_parts",
        "InnerState::from_ref(state)",
        "axum/src/extract/state.rs",
    )?;
    assert_owner_path_targetless(
        &db,
        state_owner,
        &["InnerState", "from_ref"],
        CallStatusKind::Unsupported,
        "axum/src/extract/state.rs:309",
    )?;

    let middleware_owner = function_id_by_name(&db, "test_from_extractor")?;
    assert_owner_path_targetless(
        &db,
        middleware_owner,
        &["Secret", "from_ref"],
        CallStatusKind::Unsupported,
        "axum/src/middleware/from_extractor.rs:328",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_self_accept_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Self::accept(self).await` listener row.
    // Source chain:
    //   axum/src/serve/listener.rs:41 and :61 use `Self::accept`.
    // Current model gap: the visible `Self::accept` path row is unsupported and
    // targetless; it should not be treated as recursive trait dispatch.
    // The current fixture projects the line-41 owner only; line 61 remains part
    // of the same body-owner completeness gap.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "accept",
        "Self::accept(self).await",
        "axum/src/serve/listener.rs",
    )?;
    assert_owner_path_targetless(
        &db,
        owner,
        &["Self", "accept"],
        CallStatusKind::Unsupported,
        "axum/src/serve/listener.rs:41",
    )?;
    assert_targetless_path_rows(&db, &["Self", "accept"], CallStatusKind::Unsupported, 1)?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Self", "accept"],
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum/src/serve/listener.rs",
            lines: &[41],
        }],
    )
}

#[test]
fn axum_real_target_header_value_from_static_external_paths_are_targetless() -> Result<(), DbError>
{
    let db = setup_axum_call_graph_db()?;

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   const initializer and external `HeaderValue::from_static` rows.
    //
    // Source chain:
    //   axum/src/extract/ws.rs:382 and :384 include local const
    //   initializers inside `WebSocketUpgrade::on_upgrade`.
    //   axum/src/routing/route.rs:202 includes
    //   `HeaderValue::from_static("0")` in a local const initializer.
    //   axum-core/src/response/into_response.rs:196,207,232,320,
    //   axum/src/json.rs:208,217, and axum/src/response/mod.rs:47 call the
    //   same external associated function from response conversion bodies.
    // Current DB contract: all projected rows stay external and targetless.
    // Local const initializer rows are absent rather than flattened under the
    // enclosing function owner. No row is owned by `CallBodyOwnerId::Const`
    // yet.
    assert_no_method_owner_by_body_and_file_suffix(
        &db,
        "on_upgrade",
        "const UPGRADE: HeaderValue = HeaderValue::from_static(\"upgrade\")",
        "axum/src/extract/ws.rs",
        "axum/src/extract/ws.rs:382 and :384",
    )?;

    let route_owner =
        function_id_by_name_in_module(&db, &["crate", "routing", "route"], "set_content_length")?;
    let route_context = db.call_context_for_owner(route_owner)?;
    assert!(
        route_context.iter().all(|row| {
            row.site
                .path
                .as_ref()
                .is_none_or(|path| path != &["HeaderValue", "from_static"])
        }),
        "axum/src/routing/route.rs:202 local const HeaderValue::from_static should remain absent under set_content_length: {route_context:#?}"
    );

    let json_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "into_response",
        "serde_json::to_writer(&mut buf, &self.0)",
        "axum/src/json.rs",
    )?;
    assert_owner_path_targetless_count(
        &db,
        json_owner,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        2,
        "axum/src/json.rs:208 and :217 HeaderValue::from_static",
    )?;

    let html_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "into_response",
        "mime::TEXT_HTML_UTF_8.as_ref()",
        "axum/src/response/mod.rs",
    )?;
    assert_owner_path_targetless(
        &db,
        html_owner,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        "axum/src/response/mod.rs:47 HeaderValue::from_static",
    )?;

    let core_cases = [
        (
            "axum-core/src/response/into_response.rs:196 HeaderValue::from_static",
            "mime::TEXT_PLAIN_UTF_8.as_ref()",
            1,
        ),
        (
            "axum-core/src/response/into_response.rs:207 and :320 HeaderValue::from_static",
            "Body::from(self).into_response();res.headers_mut().insert(header::CONTENT_TYPE,HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref())",
            2,
        ),
        (
            "axum-core/src/response/into_response.rs:232 HeaderValue::from_static",
            "BytesChainBody",
            1,
        ),
    ];
    for (label, body_marker, expected_owners) in core_cases {
        let owners = method_ids_by_name_body_and_file_suffix(
            &db,
            "into_response",
            body_marker,
            "axum-core/src/response/into_response.rs",
        )?;
        assert_eq!(
            owners.len(),
            expected_owners,
            "{label} should resolve the expected into_response owner set"
        );
        for owner in owners {
            assert_owner_path_targetless(
                &db,
                owner,
                &["HeaderValue", "from_static"],
                CallStatusKind::External,
                label,
            )?;
        }
    }

    let const_rows = db.raw_query(
        r#"?[site_id] :=
            *const { id: owner_id @ 'NOW' },
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Path",
                path: ["HeaderValue", "from_static"] @ 'NOW'
            }"#,
    )?;
    assert!(
        const_rows.rows.is_empty(),
        "HeaderValue::from_static should not be modeled as const-owned until const body ownership lands: {const_rows:#?}"
    );

    assert_targetless_path_rows(
        &db,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        7,
    )?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        &[
            SourceLineFanout {
                file_suffix: "axum-core/src/response/into_response.rs",
                lines: &[196, 207, 232, 320],
            },
            SourceLineFanout {
                file_suffix: "axum/src/json.rs",
                lines: &[208, 217],
            },
            SourceLineFanout {
                file_suffix: "axum/src/response/mod.rs",
                lines: &[47],
            },
        ],
    )
}

#[test]
fn axum_real_target_trait_object_dispatch_rows_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: trait-object dispatch rows.
    // Source chains:
    //   axum/src/error_handling/mod.rs:251 calls
    //   `self.project().future.poll(cx)` on
    //   `Pin<Box<dyn Future<...>>>` from error_handling/mod.rs:240.
    //   The semantic target is trait-object `Future::poll`, with no concrete
    //   runtime future available in the current call graph.
    //   axum/src/serve/mod.rs:485, middleware/from_fn.rs:375,
    //   middleware/map_request.rs:345, and middleware/map_response.rs:333
    //   project the current `as_mut().poll(cx)` targetless receiver bucket.
    //   axum-core/src/body.rs:32 calls
    //   `<dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k)`.
    // Current model gap: dyn Future dispatch rows stay targetless; the
    // qualified `<dyn Any>::downcast_mut` syntax is not projected as a path row
    // yet.
    assert_targetless_method_rows(
        &db,
        "poll",
        "MethodCallResult",
        Some(&["as_mut"]),
        CallStatusKind::Unsupported,
        4,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "poll",
        "MethodCallResult",
        Some(&["as_mut"]),
        CallStatusKind::Unsupported,
        &[
            SourceLineFanout {
                file_suffix: "axum/src/serve/mod.rs",
                lines: &[485],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/from_fn.rs",
                lines: &[375],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/map_request.rs",
                lines: &[345],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/map_response.rs",
                lines: &[333],
            },
        ],
    )?;
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "poll",
        "self.project().future.poll(cx)",
        "axum/src/error_handling/mod.rs",
    )?;
    let context = db.call_context_for_owner(owner)?;
    let project = row_by_method_receiver(&context, "project", &CallReceiver::SelfValue);
    assert_targetless_status(project, CallStatusKind::Unresolved);
    assert!(
        relations_for_site(&db, project.site.id)?.rows.is_empty(),
        "axum/src/error_handling/mod.rs:251 self.project() should have zero persisted call edges"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        owner,
        project.site.id,
        "axum/src/error_handling/mod.rs:251 self.project() receiver setup",
    )?;
    let poll = row_by_method_receiver(&context, "poll", &CallReceiver::Unsupported);
    assert_targetless_status(poll, CallStatusKind::Unsupported);
    assert!(
        relations_for_site(&db, poll.site.id)?.rows.is_empty(),
        "axum/src/error_handling/mod.rs:251 dyn Future::poll should have zero persisted call edges"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        owner,
        poll.site.id,
        "axum/src/error_handling/mod.rs:251 dyn Future::poll receiver dispatch",
    )?;
    for (label, owner) in [
        (
            "axum-core/src/body.rs:29 <dyn Any>::downcast_mut",
            function_id_by_name_in_module(&db, &["crate", "body"], "try_downcast")?,
        ),
        (
            "axum/src/util.rs:105 <dyn Any>::downcast_mut",
            function_id_by_name_in_module(&db, &["crate", "util"], "try_downcast")?,
        ),
    ] {
        let context = db.call_context_for_owner(owner)?;
        assert!(
            context.iter().all(|row| {
                row.site.method.as_deref() != Some("downcast_mut")
                    && row.site.path.as_ref() != Some(&path(&["dyn", "Any", "downcast_mut"]))
            }),
            "{label} should remain absent until qualified dyn paths are projected: {context:#?}"
        );
    }

    assert_no_path_rows(&db, &["dyn", "Any", "downcast_mut"])
}

#[test]
fn axum_real_target_blanket_via_parts_self_path_is_absent_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `FromRequest` ViaParts blanket inner call.
    // Source chain:
    //   axum-core/src/extract/mod.rs:103 calls
    //   `Self::from_request_parts(parts, state).await`.
    // Current model gap: this async blanket-impl body does not project a
    // `Self::from_request_parts` path row in the axum fixture yet.
    assert_no_path_rows(&db, &["Self", "from_request_parts"])
}

#[test]
fn axum_real_target_handler_macro_extraction_paths_are_absent_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: generated `Handler::call` extraction rows.
    // Source chain:
    //   axum/src/handler/mod.rs:242 calls
    //   `$ty::from_request_parts(&mut parts, &state).await`.
    //   handler/mod.rs:250 calls `$last::from_request(req, &state).await`.
    // Current model gap: these macro-template associated paths are not
    // projected as stable call_site rows in the axum fixture yet.
    assert_no_path_rows(&db, &["ty", "from_request_parts"])?;
    assert_no_path_rows(&db, &["last", "from_request"])
}

#[test]
fn axum_real_target_handler_async_block_body_calls_are_absent_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: async block body boundary.
    // Source chains:
    //   axum/src/handler/mod.rs:217 calls
    //   `Box::pin(async move { self().await.into_response() })`.
    //   axum/src/handler/mod.rs:240 starts the generated `Handler::call`
    //   async block whose inner calls are separately documented by the
    //   `$ty::from_request_parts` and `$last::from_request` matrix rows.
    // Current model gap: async-block bodies do not yet receive independent
    // call-body ownership, so inner `self(...)` and `into_response()` calls
    // must not be flattened into the enclosing `Handler::call` owner.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "call",
        "self().await.into_response()",
        "axum/src/handler/mod.rs",
    )?;
    let context = db.call_context_for_owner(owner)?;

    assert!(
        context
            .iter()
            .all(|row| row.site.kind != CallSiteKind::Dynamic),
        "axum/src/handler/mod.rs:217 inner async-block `self()` should remain absent until nested async ownership lands: {context:#?}"
    );
    assert!(
        context
            .iter()
            .all(|row| row.site.method.as_deref() != Some("into_response")),
        "axum/src/handler/mod.rs:217 inner async-block `into_response()` should remain absent until nested async ownership lands: {context:#?}"
    );

    Ok(())
}
