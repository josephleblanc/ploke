use super::super::*;
use super::common::*;
use super::source_lines::*;
use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;
use uuid::Uuid;

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
    //   axum/src/middleware/from_extractor.rs:220 calls
    //   `E::from_request_parts` from an async-block owner nested inside
    //   `FromExtractor::call`.
    //   axum-core/src/extract/mod.rs:103 calls `Self::from_request_parts`
    //   from an async-block owner nested inside the ViaParts blanket impl.
    // Intermediate bindings:
    //   axum-core/src/extract/mod.rs:79 declares trait `FromRequest`.
    //   axum-core/src/extract/mod.rs:85 declares `FromRequest::from_request`.
    //   axum-core/src/extract/mod.rs:53 declares trait `FromRequestParts`.
    //   axum-core/src/extract/mod.rs:59 declares
    //   `FromRequestParts::from_request_parts`.
    //   axum/src/error_handling/mod.rs:207-222 generated HandleError
    //   service impls call `Tn::from_request_parts` for extractor prefixes.
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
        (
            "axum/src/middleware/from_extractor.rs:220",
            async_block_owner_for_method_parent(
                &db,
                method_id_by_name_body_and_file_suffix(
                    &db,
                    "call",
                    "E::from_request_parts(&mut parts, &state).await",
                    "axum/src/middleware/from_extractor.rs",
                )?,
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
        18,
        "FromRequest::from_request should expose both hand-written bounded callers plus generated Handler arity callers: {from_request_callers:#?}"
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
        261,
        "FromRequestParts::from_request_parts should expose all inspected bounded callers plus generated Handler and HandleError service extractor-prefix callers: {from_request_parts_callers:#?}"
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
fn axum_real_target_from_ref_bounded_paths_reach_trait_method() -> Result<(), DbError> {
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
    // The target-centered caller query also includes the axum
    // `extract/state.rs:309` dependency-root bound resolved by the workspace
    // proof slice below.
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
        4,
        "FromRef::from_ref should expose the two same-crate callers plus both axum dependency-root callers: {callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "FromRef::from_ref real-corpus callers",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_from_ref_dependency_root_bound_reaches_workspace_trait_method()
-> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: dependency-root `FromRef::from_ref` bounded calls.
    // Source chain:
    //   axum/src/extract/state.rs:1 imports `axum_core::extract::FromRef`.
    //   axum/src/extract/state.rs:309 calls `InnerState::from_ref(state)`.
    //   axum/src/middleware/from_extractor.rs:306 imports
    //   `axum_core::extract::FromRef`.
    //   axum/src/middleware/from_extractor.rs:328 calls
    //   `Secret::from_ref(state)` from a test function.
    // Expected traversal: the top-level State extractor row is owned by the
    // impl method whose where predicate imports `axum_core::extract::FromRef`.
    // The workspace-aware call resolver uses the path dependency proof
    // `axum -> axum-core` before projection, so this row resolves to the
    // parsed axum-core `FromRef::from_ref` trait method binding in one
    // associated-function edge.
    let target = method_id_by_trait_name(&db, "FromRef", "from_ref")?;
    let state_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "from_request_parts",
        "InnerState::from_ref(state)",
        "axum/src/extract/state.rs",
    )?;
    let state_context = db.call_context_for_owner(state_owner)?;
    let state_row = row_by_path(&state_context, &["InnerState", "from_ref"]);
    assert_resolved_target(
        state_row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/extract/state.rs:309",
            owner: state_owner,
            target,
            site_id: state_row.site.id,
            expected_edge_count: 1,
        },
    )?;

    // The nested middleware test helper row is owned by the function-local impl
    // method body, not the enclosing async test function. Its local impl
    // where-bound `Secret: FromRef<S>` now resolves through the parsed
    // workspace dependency proof to the axum-core trait method binding.
    let middleware_test = function_id_by_name(&db, "test_from_extractor")?;
    let middleware_owner = local_item_owner_for_parent_with_label(
        &db,
        middleware_test,
        "local_impl_method:from_request_parts",
    )?;
    let middleware_context = db.call_context_for_owner(middleware_owner)?;
    let middleware_row = row_by_path(&middleware_context, &["Secret", "from_ref"]);
    assert_resolved_target(
        middleware_row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/middleware/from_extractor.rs:328",
            owner: middleware_owner,
            target,
            site_id: middleware_row.site.id,
            expected_edge_count: 1,
        },
    )?;

    let domain_id = "bd:corpus-axum-call-graph";
    let mut records = axum_domain_records(domain_id);
    records.extend([
        ploke_test_utils::axum_dependency_record(domain_id, state_row.site.id, state_owner, target),
        ploke_test_utils::axum_dependency_record(
            domain_id,
            middleware_row.site.id,
            middleware_owner,
            target,
        ),
    ]);
    db.upsert_proof_fact_values(&records)?;

    let target_id = target.to_string();
    let proof_rows = db.proof_symbol_lookup(&target_id)?;
    assert_root_proof(
        &proof_rows,
        state_row.site.id,
        state_owner,
        target,
        "axum/src/extract/state.rs:309 dependency-root proof",
    );
    assert_root_proof(
        &proof_rows,
        middleware_row.site.id,
        middleware_owner,
        target,
        "axum/src/middleware/from_extractor.rs:328 dependency-root proof",
    );

    Ok(())
}

fn assert_root_proof(
    rows: &[ProofGraphContextRow],
    site_id: Uuid,
    caller_id: Uuid,
    target_id: Uuid,
    label: &str,
) {
    let site_id = site_id.to_string();
    let caller_id = caller_id.to_string();
    let target_id = target_id.to_string();
    assert!(
        rows.iter().any(|row| {
            row.kind == "dependency_root"
                && row.call_site_id.as_deref() == Some(site_id.as_str())
                && row.caller_def_id.as_deref() == Some(caller_id.as_str())
                && row.resolved_def_id.as_deref() == Some(target_id.as_str())
                && row.target_kind.as_deref() == Some("workspace_trait_method")
                && row.target_name.as_deref() == Some("axum_core::extract::FromRef::from_ref")
                && row.target_root.as_deref() == Some("axum-core/src/extract/from_ref.rs")
                && row.status.as_deref() == Some("admitted")
                && row.evidence_use.as_deref() == Some("proof_only")
        }),
        "{label} should expose an admitted dependency-root proof row: {rows:#?}"
    );
}

#[test]
fn axum_real_target_self_accept_rows_are_external_frontiers() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Self::accept(self).await` listener row.
    // Source chain:
    //   axum/src/serve/listener.rs:41 and :61 use `Self::accept`.
    // Contract: both visible `Self::accept` path rows are external and targetless;
    // it should not be treated as recursive trait dispatch.
    let owners = method_ids_by_name_body_and_file_suffix(
        &db,
        "accept",
        "Self::accept(self).await",
        "axum/src/serve/listener.rs",
    )?;
    assert_eq!(
        owners.len(),
        2,
        "expected TcpListener and UnixListener accept method bodies to be visible under linux cfg"
    );
    for owner in owners {
        assert_owner_path_targetless(
            &db,
            owner,
            &["Self", "accept"],
            CallStatusKind::External,
            "axum/src/serve/listener.rs Self::accept",
        )?;
    }
    assert_targetless_path_rows(&db, &["Self", "accept"], CallStatusKind::External, 2)?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Self", "accept"],
        CallStatusKind::External,
        &[SourceLineFanout {
            file_suffix: "axum/src/serve/listener.rs",
            lines: &[41, 61],
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
    //   axum/src/json.rs:208 and :217 include `HeaderValue::from_static`
    //   inside the nested local `make_response` function.
    //   axum-core/src/response/into_response.rs:196,207,232,320,
    //   axum/src/json.rs:208,217, and axum/src/response/mod.rs:47 call the
    //   same external associated function from response conversion bodies.
    // Current DB contract: all projected rows stay external and targetless.
    // The local const and local function rows are owned by executable
    // `LocalItem` owners, not flattened under enclosing function/method owners
    // and not modeled as item-level `Const` owners. The websocket local const
    // initializer rows remain absent in this fixture.
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
    let json_context = db.call_context_for_owner(json_owner)?;
    assert!(
        json_context.iter().all(|row| {
            row.site
                .path
                .as_ref()
                .is_none_or(|path| path != &["HeaderValue", "from_static"])
        }),
        "axum/src/json.rs:208 and :217 nested local fn HeaderValue::from_static rows should remain absent under outer Json::into_response method: {json_context:#?}"
    );
    assert_targetless_path_owner_kind_rows(
        &db,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        "LocalItem",
        3,
        "axum local const HeaderValue::from_static rows",
    )?;
    assert_targetless_path_owner_kind_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        "LocalItem",
        &[
            SourceLineFanout {
                file_suffix: "axum/src/json.rs",
                lines: &[208, 217],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/route.rs",
                lines: &[202],
            },
        ],
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
        "HeaderValue::from_static should not be modeled as item-level const-owned rows: {const_rows:#?}"
    );

    assert_targetless_path_rows(
        &db,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        8,
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
    // Current model split: dyn Future dispatch rows stay targetless; qualified
    // `<dyn Any>::downcast_mut` rows project as std-root external frontiers.
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
            "axum-core/src/body.rs:32 <dyn Any>::downcast_mut",
            function_id_by_name_in_module(&db, &["crate", "body"], "try_downcast")?,
        ),
        (
            "axum/src/util.rs:105 <dyn Any>::downcast_mut",
            function_id_by_name_in_module(&db, &["crate", "util"], "try_downcast")?,
        ),
    ] {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_path(&context, &["std", "any", "Any", "downcast_mut"]);
        assert_external_targetless(row);
        assert_eq!(row.site.arg_count, Some(1), "{label} arg count");
        assert_eq!(
            row.site.generic_arg_count,
            Some(1),
            "{label} generic arg count"
        );
        assert_no_traversal_candidates_for_site(&db, owner, row.site.id, label)?;
    }

    assert_targetless_path_rows(
        &db,
        &["std", "any", "Any", "downcast_mut"],
        CallStatusKind::External,
        2,
    )?;
    Ok(())
}

#[test]
fn axum_real_target_blanket_via_parts_self_path_reaches_trait_method() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?;

    // Matrix: `FromRequest` ViaParts blanket inner call.
    // Source chain:
    //   axum-core/src/extract/mod.rs:103 calls
    //   `Self::from_request_parts(parts, state).await`.
    // Expected traversal: the nested async block owns the structural path row,
    // but `Self` associated-function resolution climbs to the parent blanket
    // impl method and uses the `T: FromRequestParts<S>` bound to reach the
    // trait method binding.
    let callers = db.callers_for_target(target)?;
    let caller = callers
        .iter()
        .find(|caller| {
            caller.site.kind == CallSiteKind::Path
                && caller.site.path.as_deref() == Some(&path(&["Self", "from_request_parts"]))
                && matches!(
                    owner_kind_for_call_body_owner(&db, caller.site.owner_id),
                    Ok(kind) if kind == "AsyncBlock"
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "FromRequestParts::from_request_parts should include the async-block Self::from_request_parts caller: {callers:#?}"
            )
        });
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.target_id, target);
    assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-core/src/extract/mod.rs:103 async-block Self::from_request_parts",
            owner: caller.site.owner_id,
            target,
            site_id: caller.site.id,
            expected_edge_count: 1,
        },
    )?;
    Ok(())
}

#[test]
fn axum_real_target_handler_macro_extraction_paths_project_generated_rows() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: generated `Handler::call` extraction rows.
    // Source chain:
    //   axum/src/handler/mod.rs:242 calls
    //   `$ty::from_request_parts(&mut parts, &state).await`.
    //   handler/mod.rs:250 calls `$last::from_request(req, &state).await`.
    // Expected traversal: the bounded `all_the_tuples!(impl_handler)`
    // generated item model projects concrete generic names (`T1`, `T2`, ...)
    // instead of unstable macro metavariables, preserves async-block ownership,
    // and uses the generated impl where-clause proof to reach the
    // `FromRequest` / `FromRequestParts` trait method bindings.
    let one_param = method_id_by_name_and_body_substring(
        &db,
        "call",
        "T1 :: from_request (req , & state) . await",
    )?;
    assert_method_owner_impl_trait(&db, one_param, "Handler", "generated arity-1 Handler::call")?;
    let one_param_body = async_block_owner_for_method_parent(&db, one_param)?;
    let one_param_context = db.call_context_for_owner(one_param_body)?;
    let from_request = method_id_by_trait_name(&db, "FromRequest", "from_request")?;
    let from_request_parts =
        method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?;
    let last_row = row_by_path(&one_param_context, &["T1", "from_request"]);
    assert_resolved_target(
        last_row,
        from_request,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/handler/mod.rs generated arity-1 T1::from_request",
            owner: one_param_body,
            target: from_request,
            site_id: last_row.site.id,
            expected_edge_count: 1,
        },
    )?;

    let two_param = method_id_by_name_and_body_substring(
        &db,
        "call",
        "T2 :: from_request (req , & state) . await",
    )?;
    assert_method_owner_impl_trait(&db, two_param, "Handler", "generated arity-2 Handler::call")?;
    let two_param_body = async_block_owner_for_method_parent(&db, two_param)?;
    let two_param_context = db.call_context_for_owner(two_param_body)?;
    let parts_row = row_by_path(&two_param_context, &["T1", "from_request_parts"]);
    assert_resolved_target(
        parts_row,
        from_request_parts,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/handler/mod.rs generated arity-2 T1::from_request_parts",
            owner: two_param_body,
            target: from_request_parts,
            site_id: parts_row.site.id,
            expected_edge_count: 1,
        },
    )?;
    let last_row = row_by_path(&two_param_context, &["T2", "from_request"]);
    assert_resolved_target(
        last_row,
        from_request,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/handler/mod.rs generated arity-2 T2::from_request",
            owner: two_param_body,
            target: from_request,
            site_id: last_row.site.id,
            expected_edge_count: 1,
        },
    )?;
    assert_no_path_rows(&db, &["ty", "from_request_parts"])?;
    assert_no_path_rows(&db, &["last", "from_request"])
}

#[test]
fn axum_error_handling_impl_service_paths_project_generated_rows() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: generated `HandleError<_, _, T>::call` extraction rows.
    // Source chain:
    //   axum/src/error_handling/mod.rs:152-205 defines local
    //   `impl_service!`.
    //   error_handling/mod.rs:207-222 invokes it for arities one through
    //   sixteen. Each generated `Service::call` body calls
    //   `$ty::from_request_parts(&mut parts, &()).await` before rebuilding
    //   the request and calling `inner.oneshot(req).await`.
    // Expected traversal: the bounded, module-specific generated item model
    // projects the sixteen generated `call` owners with the extractor path
    // rows needed for proof. It preserves the nested async-block owner and
    // uses the generated `FromRequestParts<()>` where-clause proof to reach
    // the trait method binding without applying the unrelated `impl_service!`
    // templates in middleware modules.
    let owners = method_ids_by_name_body_and_file_suffix(
        &db,
        "call",
        "from_request_parts(&mut parts, &()).await",
        "axum/src/error_handling/mod.rs",
    )?;
    assert_eq!(
        owners.len(),
        16,
        "error_handling::impl_service! should generate exactly sixteen Service::call methods"
    );

    let arity_sixteen = method_id_by_name_body_and_file_suffix(
        &db,
        "call",
        "T16::from_request_parts(&mut parts, &()).await",
        "axum/src/error_handling/mod.rs",
    )?;
    let async_owner = async_block_owner_for_method_parent(&db, arity_sixteen)?;
    let context = db.call_context_for_owner(async_owner)?;
    let target = method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?;

    let target_sites = db.call_sites_for_target(target)?;
    let mut site_ids = Vec::new();
    for index in 1..=16 {
        let ty = format!("T{index}");
        let expected_path = path(&[ty.as_str(), "from_request_parts"]);
        let row = context
            .iter()
            .find(|row| {
                row.site.kind == CallSiteKind::Path
                    && row.site.path.as_ref() == Some(&expected_path)
            })
            .unwrap_or_else(|| {
                panic!(
                    "generated arity-16 async owner should expose {ty}::from_request_parts: {context:#?}"
                )
            });
        assert_resolved_target(
            row,
            target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
        );
        assert_eq!(
            relations_for_site(&db, row.site.id)?.rows.len(),
            1,
            "axum/src/error_handling/mod.rs generated arity-16 {ty}::from_request_parts should persist one call edge"
        );
        assert!(
            target_sites
                .iter()
                .any(|site| site.owner_id == async_owner && site.id == row.site.id),
            "target-centered call_sites_for_target should include generated {ty}::from_request_parts: {target_sites:#?}"
        );
        site_ids.push(row.site.id);
    }

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(async_owner),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        outgoing.iter().any(|candidate| {
            candidate.node_id == target
                && candidate.target_id == target
                && candidate.relation == ploke_db::CallContextRelation::OutgoingTarget
                && candidate.distance == 1
                && site_ids.contains(&candidate.call_site_id)
        }),
        "generated arity-16 async owner should traverse to FromRequestParts::from_request_parts through one of its extractor rows: {outgoing:#?}"
    );

    Ok(())
}

#[test]
fn axum_real_target_handler_async_block_body_calls_are_async_block_owned() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: async block body boundary.
    // Source chains:
    //   axum/src/handler/mod.rs:217 calls
    //   `Box::pin(async move { self().await.into_response() })`.
    //   axum/src/handler/mod.rs:240 starts the generated `Handler::call`
    //   async block whose inner calls are separately documented by the
    //   `$ty::from_request_parts` and `$last::from_request` matrix rows.
    // Expected traversal: the concrete non-macro async block has its own
    // executable owner. The generic callable `self()` and awaited
    // `into_response()` receiver stay targetless, but they must be owned by the
    // async-block body and must not be flattened into the enclosing
    // `Handler::call` owner.
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
        "axum/src/handler/mod.rs:217 inner async-block `self()` must not be flattened into Handler::call: {context:#?}"
    );
    assert!(
        context
            .iter()
            .all(|row| row.site.method.as_deref() != Some("into_response")),
        "axum/src/handler/mod.rs:217 inner async-block `into_response()` must not be flattened into Handler::call: {context:#?}"
    );

    let async_owner = async_block_owner_for_method_parent(&db, owner)?;
    let async_context = db.call_context_for_owner(async_owner)?;
    let self_row = row_by_path(&async_context, &["self"]);
    assert_targetless_status(self_row, CallStatusKind::Unsupported);
    let into_response_receiver = CallReceiver::AwaitPathCallResult {
        path: path(&["self"]),
    };
    let into_response =
        row_by_method_receiver(&async_context, "into_response", &into_response_receiver);
    assert_targetless_status(into_response, CallStatusKind::Unsupported);
    assert_no_traversal_candidates_for_site(
        &db,
        async_owner,
        self_row.site.id,
        "axum/src/handler/mod.rs:217 Handler::call async-block self()",
    )?;
    assert_no_traversal_candidates_for_site(
        &db,
        async_owner,
        into_response.site.id,
        "axum/src/handler/mod.rs:217 Handler::call async-block into_response()",
    )?;

    assert!(
        db.project_call_proof_facts_for_owner(async_owner, "bd:corpus-axum-call-graph")? >= 4,
        "Handler::call async block should project targetless self()/into_response() proof rows"
    );
    db.upsert_proof_fact_values(&[
        ploke_test_utils::axum_handler_async_block_poll_resume_blocker(self_row.site.id, "self"),
        ploke_test_utils::axum_handler_async_block_poll_resume_blocker(
            into_response.site.id,
            "into_response",
        ),
    ])?;

    let self_site = self_row.site.id.to_string();
    let into_response_site = into_response.site.id.to_string();
    let blockers = db.proof_blockers()?;
    for (site, label) in [
        (self_site.as_str(), "Handler::call async-block self()"),
        (
            into_response_site.as_str(),
            "Handler::call async-block into_response()",
        ),
    ] {
        assert!(
            blockers.iter().any(|proof| {
                proof.call_site_id.as_deref() == Some(site)
                    && proof.reason == "dynamic_dispatch_unbounded"
                    && proof.status == "blocked"
            }),
            "{label} should expose the async poll/resume blocker: {blockers:#?}"
        );
    }

    let proof_rows = db.proof_graphrag_context("async poll/resume")?;
    for (site, label) in [
        (self_site.as_str(), "Handler::call async-block self()"),
        (
            into_response_site.as_str(),
            "Handler::call async-block into_response()",
        ),
    ] {
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "proof_blocker"
                    && proof.call_site_id.as_deref() == Some(site)
                    && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
            }),
            "{label} proof context should expose the async poll/resume blocker: {proof_rows:#?}"
        );
    }

    Ok(())
}

fn async_block_owner_for_method_parent(db: &Database, parent: Uuid) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[id] :=
            parent = to_uuid("{parent}"),
            *call_body_owner {{
                id,
                owner_kind: "AsyncBlock",
                parent_id: parent,
                parent_kind: "Method",
                label: "async_block" @ 'NOW'
            }}"#
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one async_block call_body_owner row for method parent {parent}: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][0])
}

fn assert_method_owner_impl_trait(
    db: &Database,
    method: Uuid,
    trait_name: &str,
    label: &str,
) -> Result<(), DbError> {
    let rows = db.raw_query(&format!(
        r#"?[impl_id, trait_type_id, trait_target_id] :=
            method = to_uuid("{method}"),
            *method {{ id: method, owner_id: impl_id @ 'NOW' }},
            *impl {{ id: impl_id, trait_type: trait_type_id @ 'NOW' }},
            *type_use {{
                owner_id: impl_id,
                root_type_id: trait_type_id,
                role: "ImplTrait" @ 'NOW'
            }},
            *type_relation {{
                source_id: trait_type_id,
                target_id: trait_target_id,
                relation_kind: "Trait" @ 'NOW'
            }},
            *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "{label} should be recorded as an impl of trait {trait_name}: {:#?}",
        rows.rows
    );
    Ok(())
}
