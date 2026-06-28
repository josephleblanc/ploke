use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;

use super::super::*;
use super::common::*;
use super::source_lines::{
    SourceLineFanout, assert_targetless_method_line_fanout, assert_targetless_path_line_fanout,
};

#[test]
fn axum_core_extract_self_methods_reach_same_impl_methods() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum-core/src/ext_traits/request.rs:268 self.extract_with_state(&())
    //   axum-core/src/ext_traits/request_parts.rs:122 self.extract_with_state(&())
    let owners =
        method_ids_by_name_and_body_substring(&db, "extract", "self.extract_with_state(&())")?;
    assert_eq!(
        owners.len(),
        2,
        "axum should expose both RequestExt and RequestPartsExt extract methods"
    );

    let request_target = method_id_by_name_and_body_substring(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
    )?;
    let parts_target = method_id_by_name_and_body_substring(
        &db,
        "extract_with_state",
        "E::from_request_parts(self, state)",
    )?;
    let expected_targets = [request_target, parts_target];

    for owner in owners {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_method_receiver(&context, "extract_with_state", &CallReceiver::SelfValue);
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert!(
            expected_targets.contains(&row.targets[0].target_id),
            "self.extract_with_state should resolve to one of the same-impl extract_with_state methods: {row:#?}"
        );
        assert_eq!(row.targets[0].relation, CallRelationKind::Method);
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
        assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: "extract -> extract_with_state",
                owner,
                target: row.targets[0].target_id,
                site_id: row.site.id,
                expected_edge_count: 1,
            },
        )?;
    }

    for target in expected_targets {
        let callers = db.callers_for_target(target)?;
        assert_eq!(
            callers.len(),
            1,
            "each extract_with_state impl should have exactly one inspected self-method caller"
        );
        assert_sites_match_callers(
            &db,
            target,
            &callers,
            "extract_with_state self-method caller",
        )?;
        assert_eq!(callers[0].status.status, CallStatusKind::Resolved);
        assert_eq!(callers[0].target.target_id, target);
    }

    Ok(())
}

#[test]
fn axum_real_target_request_extensions_mut_receivers_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: local and parameter `req.extensions_mut` receiver rows.
    // Source chain:
    //   axum-core/src/ext_traits/request.rs:297 initializes
    //   `let mut req = Request::new(())`.
    //   axum-core/src/ext_traits/request.rs:302 calls `req.extensions_mut()`.
    //   axum/src/extension.rs:184 calls `req.extensions_mut()`.
    // Current model gap: the initializer path is unresolved and targetless; the
    // receiver calls are projected on a local binding named `req`, but they
    // remain unresolved external receiver calls.
    let owner =
        method_id_by_name_and_body_substring(&db, "extract_parts_with_state", "Request::new(())")?;
    let context = db.call_context_for_owner(owner)?;
    let request_new = row_by_path(&context, &["Request", "new"]);
    assert_targetless_status(request_new, CallStatusKind::Unresolved);
    assert!(
        relations_for_site(&db, request_new.site.id)?
            .rows
            .is_empty(),
        "axum-core/src/ext_traits/request.rs:297 Request::new should not have raw call_relation targets"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        owner,
        request_new.site.id,
        "axum-core/src/ext_traits/request.rs:297 Request::new setup for req.extensions_mut",
    )?;

    let req_receiver = CallReceiver::LocalBinding {
        name: "req".to_string(),
    };
    let cases = [
        (
            // axum-core/src/extract/default_body_limit.rs:183
            // `DefaultBodyLimit::apply` calls `req.extensions_mut().insert(...)`.
            "axum-core/src/extract/default_body_limit.rs:183",
            method_id_by_name_body_and_file_suffix(
                &db,
                "apply",
                "req.extensions_mut().insert(self.kind)",
                "axum-core/src/extract/default_body_limit.rs",
            )?,
            1,
        ),
        (
            // axum-core/src/extract/default_body_limit.rs:225
            // `DefaultBodyLimitService::call` calls the same external receiver.
            "axum-core/src/extract/default_body_limit.rs:225",
            method_id_by_name_body_and_file_suffix(
                &db,
                "call",
                "req.extensions_mut().insert(self.kind)",
                "axum-core/src/extract/default_body_limit.rs",
            )?,
            1,
        ),
        (
            // axum/src/extension.rs:184
            // `AddExtension::call` uses the parameter receiver from
            // `mut req: Request<ResBody>` at axum/src/extension.rs:183.
            "axum/src/extension.rs:184",
            method_id_by_name_body_and_file_suffix(
                &db,
                "call",
                "req.extensions_mut().insert(self.value.clone())",
                "axum/src/extension.rs",
            )?,
            1,
        ),
        (
            // axum/src/extract/nested_path.rs:95 and :103
            // `SetNestedPath::call` has two `req.extensions_mut()` callsites
            // in the same owner body, so the oracle preserves count 2.
            "axum/src/extract/nested_path.rs:95 and :103",
            method_id_by_name_body_and_file_suffix(
                &db,
                "call",
                "req.extensions_mut().get_mut::<NestedPath>()",
                "axum/src/extract/nested_path.rs",
            )?,
            2,
        ),
        (
            // axum/src/routing/path_router.rs:336
            // `PathRouter::call_with_state` inserts `OriginalUri` before
            // routing to an endpoint.
            "axum/src/routing/path_router.rs:336",
            method_id_by_name_body_and_file_suffix(
                &db,
                "call_with_state",
                "req.extensions_mut().insert(original_uri)",
                "axum/src/routing/path_router.rs",
            )?,
            1,
        ),
    ];
    for (label, owner, count) in cases {
        assert_owner_method_targetless_count(
            &db,
            owner,
            "extensions_mut",
            &req_receiver,
            CallStatusKind::Unresolved,
            count,
            label,
        )?;
    }

    assert_targetless_method_rows(
        &db,
        "extensions_mut",
        "LocalBinding",
        Some(&["req"]),
        CallStatusKind::Unresolved,
        6,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "extensions_mut",
        "LocalBinding",
        Some(&["req"]),
        CallStatusKind::Unresolved,
        &[
            SourceLineFanout {
                file_suffix: "axum-core/src/extract/default_body_limit.rs",
                lines: &[183, 225],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extension.rs",
                lines: &[184],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/nested_path.rs",
                lines: &[95, 103],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/path_router.rs",
                lines: &[336],
            },
        ],
    )?;

    Ok(())
}

#[test]
fn axum_real_target_self_field_size_hint_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `self.0.size_hint` receiver row.
    // Source chain:
    //   axum-core/src/body.rs:127 calls `self.0.size_hint()`.
    // Current model gap: the tuple-field receiver shape is visible but remains
    // unsupported and targetless.
    let owner = method_id_by_name_and_body_substring(&db, "size_hint", "self.0.size_hint()")?;
    assert_owner_method_targetless(
        &db,
        owner,
        "size_hint",
        &CallReceiver::SelfField {
            path: vec!["0".to_string()],
        },
        CallStatusKind::Unsupported,
        "axum-core/src/body.rs:127",
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "size_hint",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum-core/src/body.rs",
            lines: &[127],
        }],
    )?;

    Ok(())
}

#[test]
fn axum_real_target_turbofish_local_receiver_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: turbofish method call receiver row.
    // Source chain:
    //   axum-core/src/ext_traits/request_parts.rs:164 calls
    //   `parts.extract_with_state::<State<String>, String>(&state)`.
    // Current model gap: that turbofish row is absent in the current DB
    // fixture. The one projected targetless `parts.extract_with_state` row is
    // the blanket-helper call at request_parts.rs:186, and it does not resolve
    // back to the `RequestPartsExt::extract_with_state` impl.
    let _target_owner = method_id_by_name_and_body_substring(
        &db,
        "extract_with_state",
        "E::from_request_parts(self, state)",
    )?;
    assert_targetless_method_rows(
        &db,
        "extract_with_state",
        "LocalBinding",
        Some(&["parts"]),
        CallStatusKind::Unresolved,
        1,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "extract_with_state",
        "LocalBinding",
        Some(&["parts"]),
        CallStatusKind::Unresolved,
        &[SourceLineFanout {
            file_suffix: "axum-core/src/ext_traits/request_parts.rs",
            lines: &[186],
        }],
    )?;

    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "method".to_string(),
        cozo::DataValue::from("extract_with_state"),
    );
    params.insert(
        "receiver_kind".to_string(),
        cozo::DataValue::from("LocalBinding"),
    );
    params.insert(
        "receiver_path".to_string(),
        cozo::DataValue::List(vec![cozo::DataValue::from("parts")]),
    );
    let rows = db.raw_query_params(
        r#"?[generic_arg_count] :=
            *call_site {
                id: site_id,
                call_kind: "Method",
                method_name: $method,
                receiver_kind: $receiver_kind,
                receiver_path: $receiver_path,
                generic_arg_count @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Method",
                status_kind: "Unresolved" @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "request_parts.rs:186 should project exactly one targetless local receiver row; the request_parts.rs:164 turbofish source row remains absent"
    );
    assert_eq!(
        rows.rows[0][0],
        cozo::DataValue::Num(cozo::Num::Int(0)),
        "request_parts.rs:186 local receiver row should carry no method turbofish args"
    );

    Ok(())
}

#[test]
fn axum_real_target_poll_ready_forwarding_receivers_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: service forwarding receiver rows.
    // Source chain:
    //   axum/src/extension.rs:180 calls `self.inner.poll_ready(cx)`.
    //   Other service wrappers use the same `SelfField` forwarding shape, and
    //   tuple wrappers project as `self.0.poll_ready(...)`.
    // Current model gap: external trait receiver dispatch is visible but
    // targetless.
    let inner_receiver = CallReceiver::SelfField {
        path: vec!["inner".to_string()],
    };
    let inner_cases = [
        (
            // axum-core/src/extract/default_body_limit.rs:220
            // `DefaultBodyLimitService::poll_ready` forwards to `S: Service`.
            "axum-core/src/extract/default_body_limit.rs:220",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum-core/src/extract/default_body_limit.rs",
            )?,
        ),
        (
            // axum/src/extension.rs:180
            // `AddExtension::poll_ready` forwards through `self.inner`.
            "axum/src/extension.rs:180",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/extension.rs",
            )?,
        ),
        (
            // axum/src/extract/nested_path.rs:91
            // `SetNestedPath::poll_ready` forwards through `self.inner`.
            "axum/src/extract/nested_path.rs:91",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/extract/nested_path.rs",
            )?,
        ),
        (
            // axum/src/middleware/from_extractor.rs:212
            // `FromExtractor::poll_ready` forwards through `self.inner`.
            "axum/src/middleware/from_extractor.rs:212",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/middleware/from_extractor.rs",
            )?,
        ),
        (
            // axum/src/middleware/from_fn.rs:280 / :358
            // The current fixture projects one `self.inner.poll_ready(cx)`
            // owner from this file; it remains external-trait targetless.
            "axum/src/middleware/from_fn.rs:280 or :358 projected owner",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/middleware/from_fn.rs",
            )?,
        ),
        (
            // axum/src/routing/strip_prefix.rs:36
            // `StripPrefix::poll_ready` forwards through `self.inner`.
            "axum/src/routing/strip_prefix.rs:36",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/routing/strip_prefix.rs",
            )?,
        ),
        (
            // axum/src/util.rs:69
            // `MapFuture::poll_ready` forwards through `self.inner`.
            "axum/src/util.rs:69",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/util.rs",
            )?,
        ),
    ];
    for (label, owner) in inner_cases {
        assert_owner_method_targetless_count(
            &db,
            owner,
            "poll_ready",
            &inner_receiver,
            CallStatusKind::Unsupported,
            1,
            label,
        )?;
    }

    let tuple_receiver = CallReceiver::SelfField {
        path: vec!["0".to_string()],
    };
    let tuple_method_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "poll_ready",
        "self.0.poll_ready(cx)",
        "axum/src/middleware/response_axum_body.rs",
    )?;
    assert_owner_method_targetless_count(
        &db,
        tuple_method_owner,
        "poll_ready",
        &tuple_receiver,
        CallStatusKind::Unsupported,
        1,
        // axum/src/middleware/response_axum_body.rs:45
        "axum/src/middleware/response_axum_body.rs:45",
    )?;

    let tuple_function_cases = [
        (
            // axum/src/routing/tests/mod.rs:703
            // Local `CountMiddleware<S>(S)` impl inside the test function.
            "axum/src/routing/tests/mod.rs:703",
            function_id_by_name_in_module(
                &db,
                &["crate", "routing", "tests"],
                "middleware_still_run_for_unmatched_requests",
            )?,
        ),
        (
            // axum/src/routing/tests/nest.rs:258
            // Local `SetUriExtension<S>(S)` impl inside the test function.
            "axum/src/routing/tests/nest.rs:258",
            function_id_by_name_in_module(
                &db,
                &["crate", "routing", "tests", "nest"],
                "outer_middleware_still_see_whole_url",
            )?,
        ),
    ];
    for (label, owner) in tuple_function_cases {
        assert_owner_method_targetless_count(
            &db,
            owner,
            "poll_ready",
            &tuple_receiver,
            CallStatusKind::Unsupported,
            1,
            label,
        )?;
    }

    assert_targetless_method_rows(
        &db,
        "poll_ready",
        "SelfField",
        Some(&["inner"]),
        CallStatusKind::Unsupported,
        7,
    )?;
    assert_targetless_method_rows(
        &db,
        "poll_ready",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::Unsupported,
        3,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "poll_ready",
        "SelfField",
        Some(&["inner"]),
        CallStatusKind::Unsupported,
        &[
            SourceLineFanout {
                file_suffix: "axum-core/src/extract/default_body_limit.rs",
                lines: &[220],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extension.rs",
                lines: &[180],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/nested_path.rs",
                lines: &[91],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/from_extractor.rs",
                lines: &[212],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/from_fn.rs",
                lines: &[358],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/strip_prefix.rs",
                lines: &[36],
            },
            SourceLineFanout {
                file_suffix: "axum/src/util.rs",
                lines: &[69],
            },
        ],
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "poll_ready",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::Unsupported,
        &[
            SourceLineFanout {
                file_suffix: "axum/src/middleware/response_axum_body.rs",
                lines: &[45],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/mod.rs",
                lines: &[559],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/nest.rs",
                lines: &[258],
            },
        ],
    )
}

#[test]
fn axum_real_target_router_new_and_router_clone_contracts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Router::new` / typed `router.clone`.
    // Source chain:
    //   axum/src/routing/mod.rs:162 defines `Router::new`.
    //   axum/src/routing/mod.rs:109 calls `Self::new()` from
    //   `Default for Router`.
    //   axum/src/serve/mod.rs:756 calls `Router::new()`.
    //   axum/src/routing/method_routing.rs:1494 calls
    //   `crate::Router::new()`.
    //   serve/mod.rs:769,770,772,776,780,785 call `router.clone...`.
    // Expected traversal: the current caller API exposes 142 resolved
    // `Router::new` rows, one explicit `crate::Router::new` row, and one
    // `Self::new` row, while target expansion traverses 123 incoming candidates
    // for the same target. Typed router clone receiver rows remain targetless
    // because Clone dispatch is not modeled yet.
    let target = method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?;
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        144,
        "Router::new should expose the current resolved corpus subset: {callers:#?}"
    );
    assert_sites_match_callers(&db, target, &callers, "Router::new resolved corpus subset")?;
    let mut path_counts = std::collections::BTreeMap::new();
    for caller in &callers {
        *path_counts
            .entry(
                caller
                    .site
                    .path
                    .clone()
                    .expect("Router::new caller should carry a path"),
            )
            .or_insert(0usize) += 1;
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Method);
    }
    assert_eq!(
        path_counts,
        std::collections::BTreeMap::from([
            (path(&["Router", "new"]), 142),
            (path(&["crate", "Router", "new"]), 1),
            (path(&["Self", "new"]), 1),
        ]),
        "Router::new callers should split into literal Router::new, crate::Router::new, and Default::default Self::new rows"
    );

    let compile_owner = function_id_by_name_in_module(
        &db,
        &["crate", "serve", "tests"],
        "if_it_compiles_it_works",
    )?;
    let compile_context = db.call_context_for_owner(compile_owner)?;
    let router_new = row_by_path(&compile_context, &["Router", "new"]);
    assert_resolved_target(
        router_new,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/serve/mod.rs:756 Router::new",
            owner: compile_owner,
            target,
            site_id: router_new.site.id,
            expected_edge_count: 1,
        },
    )?;

    let complex_owner = function_id_by_name_in_module(
        &db,
        &["crate", "routing", "method_routing", "tests"],
        "building_complex_router",
    )?;
    let complex_context = db.call_context_for_owner(complex_owner)?;
    let crate_router_new = row_by_path(&complex_context, &["crate", "Router", "new"]);
    assert_resolved_target(
        crate_router_new,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/routing/method_routing.rs:1494 crate::Router::new",
            owner: complex_owner,
            target,
            site_id: crate_router_new.site.id,
            expected_edge_count: 1,
        },
    )?;

    // Matrix source: axum/src/serve/mod.rs:574,575,577,581,585,590,595,601
    // call `router.clone...` from the same typed local binding. The clone
    // dispatch itself is still targetless and must not traverse to `Router`.
    let router_receiver = CallReceiver::TypedLocalBinding {
        name: "router".to_string(),
        type_path: path(&["Router"]),
    };
    let router_clone_cases = [
        (
            // axum/src/serve/mod.rs:574,575,577,581,585,590,595,601
            // `if_it_compiles_it_works` projects eight `router.clone` rows.
            "axum/src/serve/mod.rs:574,575,577,581,585,590,595,601",
            compile_owner,
            8,
        ),
        (
            // axum/src/serve/mod.rs:692
            "axum/src/serve/mod.rs:692",
            function_id_by_name_in_module(
                &db,
                &["crate", "serve", "tests"],
                "test_serve_local_addr",
            )?,
            1,
        ),
        (
            // axum/src/serve/mod.rs:704
            "axum/src/serve/mod.rs:704",
            function_id_by_name_in_module(
                &db,
                &["crate", "serve", "tests"],
                "test_with_graceful_shutdown_local_addr",
            )?,
            1,
        ),
    ];
    for (label, owner, count) in router_clone_cases {
        assert_owner_method_targetless_count(
            &db,
            owner,
            "clone",
            &router_receiver,
            CallStatusKind::Unresolved,
            count,
            label,
        )?;
    }

    let app_owner = function_id_by_name_in_module(
        &db,
        &["crate", "routing", "tests"],
        "merging_with_overlapping_method_routes",
    )?;
    assert_owner_method_targetless_count(
        &db,
        app_owner,
        "clone",
        &CallReceiver::TypedLocalBinding {
            name: "app".to_string(),
            type_path: path(&["Router"]),
        },
        CallStatusKind::Unresolved,
        1,
        // axum/src/routing/tests/mod.rs:804
        "axum/src/routing/tests/mod.rs:804",
    )?;

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 256,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(
        incoming.len(),
        123,
        "Router::new target expansion should traverse the current incoming candidate subset"
    );
    let caller_sites = callers
        .iter()
        .map(|caller| caller.site.id)
        .collect::<std::collections::HashSet<_>>();
    for candidate in &incoming {
        assert!(
            caller_sites.contains(&candidate.call_site_id),
            "Router::new target expansion should only return known caller sites: {candidate:#?}"
        );
        assert_eq!(candidate.target_id, target);
        assert_eq!(candidate.distance, 1);
    }

    assert_targetless_path_rows(&db, &["Router", "new"], CallStatusKind::Unsupported, 158)?;
    assert_targetless_method_rows(
        &db,
        "clone",
        "TypedLocalBinding",
        Some(&["router", "Router"]),
        CallStatusKind::Unresolved,
        10,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "clone",
        "TypedLocalBinding",
        Some(&["router", "Router"]),
        CallStatusKind::Unresolved,
        &[SourceLineFanout {
            file_suffix: "axum/src/serve/mod.rs",
            lines: &[574, 575, 577, 581, 585, 590, 595, 601, 692, 704],
        }],
    )?;
    assert_targetless_method_rows(
        &db,
        "clone",
        "TypedLocalBinding",
        Some(&["app", "Router"]),
        CallStatusKind::Unresolved,
        1,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "clone",
        "TypedLocalBinding",
        Some(&["app", "Router"]),
        CallStatusKind::Unresolved,
        &[SourceLineFanout {
            file_suffix: "axum/src/routing/tests/mod.rs",
            lines: &[660],
        }],
    )
}

#[test]
fn axum_real_target_result_receiver_chains_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: result receiver chains.
    // Source chains:
    //   axum/src/middleware/from_fn.rs:411 calls
    //   `Request::builder().uri(\"/\").body(Body::empty()).unwrap()`.
    //   axum/src/routing/route.rs:51 calls
    //   `self.0.clone().oneshot(req)`.
    // Current model gap: path-call and method-call result receivers are
    // structurally projected but remain targetless unless the nested local
    // associated function is already in the resolved subset.
    let route_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "oneshot_inner",
        "self.0.clone().oneshot(req)",
        "axum/src/routing/route.rs",
    )?;
    assert_owner_method_targetless(
        &db,
        route_owner,
        "oneshot",
        &CallReceiver::MethodCallResult {
            method_name: "clone".to_string(),
        },
        CallStatusKind::Unsupported,
        "axum/src/routing/route.rs:51",
    )?;
    let route_owned_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "oneshot_inner_owned",
        "self.0.oneshot(req)",
        "axum/src/routing/route.rs",
    )?;
    assert_owner_method_targetless(
        &db,
        route_owned_owner,
        "oneshot",
        &CallReceiver::SelfField {
            path: vec!["0".to_string()],
        },
        CallStatusKind::Unsupported,
        // axum/src/routing/route.rs:57
        "axum/src/routing/route.rs:57",
    )?;

    struct RequestBuilderCase {
        label: &'static str,
        module_path: &'static [&'static str],
        owner: &'static str,
        status: CallStatusKind,
    }

    let builder_cases = [
        RequestBuilderCase {
            // axum/src/extract/query.rs:104
            label: "axum/src/extract/query.rs:104",
            module_path: &["crate", "extract", "query", "tests"],
            owner: "check",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/extract/raw_form.rs:65
            label: "axum/src/extract/raw_form.rs:65",
            module_path: &["crate", "extract", "raw_form", "tests"],
            owner: "check_query",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/form.rs:156
            label: "axum/src/form.rs:156",
            module_path: &["crate", "form", "tests"],
            owner: "check_query",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/form.rs:164
            label: "axum/src/form.rs:164",
            module_path: &["crate", "form", "tests"],
            owner: "check_body",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/form.rs:226
            label: "axum/src/form.rs:226",
            module_path: &["crate", "form", "tests"],
            owner: "test_incorrect_content_type",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/routing/tests/mod.rs:1129
            label: "axum/src/routing/tests/mod.rs:1129",
            module_path: &["crate", "routing", "tests"],
            owner: "connect_going_to_custom_fallback",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/routing/tests/mod.rs:1147
            label: "axum/src/routing/tests/mod.rs:1147",
            module_path: &["crate", "routing", "tests"],
            owner: "connect_going_to_default_fallback",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/serve/mod.rs:799
            label: "axum/src/serve/mod.rs:799",
            module_path: &["crate", "serve", "tests"],
            owner: "serving_on_custom_io_type",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum-core/src/ext_traits/request.rs:375
            label: "axum-core/src/ext_traits/request.rs:375",
            module_path: &["crate", "ext_traits", "request", "tests"],
            owner: "extract_parts_without_state",
            status: CallStatusKind::Unsupported,
        },
        RequestBuilderCase {
            // axum-core/src/ext_traits/request.rs:388
            label: "axum-core/src/ext_traits/request.rs:388",
            module_path: &["crate", "ext_traits", "request", "tests"],
            owner: "extract_parts_with_state",
            status: CallStatusKind::Unsupported,
        },
        RequestBuilderCase {
            // axum/src/middleware/from_fn.rs:411
            label: "axum/src/middleware/from_fn.rs:411",
            module_path: &["crate", "middleware", "from_fn", "tests"],
            owner: "basic",
            status: CallStatusKind::Unsupported,
        },
        RequestBuilderCase {
            // axum/src/routing/method_routing.rs:1697
            label: "axum/src/routing/method_routing.rs:1697",
            module_path: &["crate", "routing", "method_routing", "tests"],
            owner: "call",
            status: CallStatusKind::Unsupported,
        },
        RequestBuilderCase {
            // axum/src/routing/tests/get_to_head.rs:22
            label: "axum/src/routing/tests/get_to_head.rs:22",
            module_path: &["crate", "routing", "tests", "get_to_head", "for_handlers"],
            owner: "get_handles_head",
            status: CallStatusKind::Unsupported,
        },
        RequestBuilderCase {
            // axum/src/routing/tests/get_to_head.rs:56
            label: "axum/src/routing/tests/get_to_head.rs:56",
            module_path: &["crate", "routing", "tests", "get_to_head", "for_services"],
            owner: "get_handles_head",
            status: CallStatusKind::Unsupported,
        },
    ];
    for case in builder_cases {
        let owner = function_id_by_name_in_module(&db, case.module_path, case.owner)?;
        assert_owner_path_targetless(&db, owner, &["Request", "builder"], case.status, case.label)?;
    }

    let from_fn_owner =
        function_id_by_name_in_module(&db, &["crate", "middleware", "from_fn", "tests"], "basic")?;
    assert_owner_path_targetless(
        &db,
        from_fn_owner,
        &["Body", "empty"],
        CallStatusKind::External,
        "axum/src/middleware/from_fn.rs:411 Body::empty",
    )?;
    assert_owner_method_targetless(
        &db,
        from_fn_owner,
        "unwrap",
        &CallReceiver::MethodCallResult {
            method_name: "body".to_string(),
        },
        CallStatusKind::Unsupported,
        "axum/src/middleware/from_fn.rs:411",
    )?;
    assert_targetless_path_rows(&db, &["Request", "builder"], CallStatusKind::External, 8)?;
    assert_targetless_path_rows(&db, &["Request", "builder"], CallStatusKind::Unsupported, 6)?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Request", "builder"],
        CallStatusKind::External,
        &[
            SourceLineFanout {
                file_suffix: "axum/src/extract/query.rs",
                lines: &[104],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/raw_form.rs",
                lines: &[65],
            },
            SourceLineFanout {
                file_suffix: "axum/src/form.rs",
                lines: &[156, 164, 226],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/mod.rs",
                lines: &[1129, 1147],
            },
            SourceLineFanout {
                file_suffix: "axum/src/serve/mod.rs",
                lines: &[799],
            },
        ],
    )?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Request", "builder"],
        CallStatusKind::Unsupported,
        &[
            SourceLineFanout {
                file_suffix: "axum-core/src/ext_traits/request.rs",
                lines: &[375, 388],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/from_fn.rs",
                lines: &[411],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/method_routing.rs",
                lines: &[1697],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/get_to_head.rs",
                lines: &[22, 56],
            },
        ],
    )?;
    assert_targetless_method_rows(
        &db,
        "oneshot",
        "MethodCallResult",
        Some(&["clone"]),
        CallStatusKind::Unsupported,
        1,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "oneshot",
        "MethodCallResult",
        Some(&["clone"]),
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum/src/routing/route.rs",
            lines: &[51],
        }],
    )?;
    assert_targetless_method_rows(
        &db,
        "oneshot",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::Unsupported,
        1,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "oneshot",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum/src/routing/route.rs",
            lines: &[57],
        }],
    )?;
    assert_targetless_method_rows(
        &db,
        "unwrap",
        "MethodCallResult",
        Some(&["body"]),
        CallStatusKind::Unsupported,
        17,
    )
}

#[test]
fn axum_real_target_await_result_receivers_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: await result receiver rows.
    // Source chain:
    //   axum/src/test_helpers/test_client.rs:134 calls
    //   `self.builder.send().await.unwrap()` inside an async block.
    //   axum/src/serve/listener.rs:143 calls
    //   `self.sem.clone().acquire_owned().await.unwrap()`.
    // Current model gap: awaited-result receiver shapes are visible in the
    // corpus, but they stay targetless. The test-client async block row remains
    // part of the broader nested async-owner gap rather than a local edge.
    let listener_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "accept",
        "self.sem.clone().acquire_owned().await.unwrap()",
        "axum/src/serve/listener.rs",
    )?;
    assert_owner_method_targetless(
        &db,
        listener_owner,
        "unwrap",
        &CallReceiver::AwaitResult,
        CallStatusKind::Unsupported,
        "axum/src/serve/listener.rs:143",
    )?;

    assert_no_method_owner_by_body_and_file_suffix(
        &db,
        "into_future",
        "self.builder.send().await.unwrap()",
        "axum/src/test_helpers/test_client.rs",
        "axum/src/test_helpers/test_client.rs:134",
    )?;

    assert_targetless_method_rows(
        &db,
        "unwrap",
        "AwaitResult",
        None,
        CallStatusKind::Unsupported,
        39,
    )?;
    assert_targetless_method_rows(
        &db,
        "send",
        "MethodCallResult",
        Some(&["get"]),
        CallStatusKind::Unsupported,
        3,
    )
}
