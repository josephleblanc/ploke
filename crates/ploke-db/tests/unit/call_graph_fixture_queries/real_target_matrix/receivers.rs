use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;

use super::super::*;
use super::common::*;
use super::source_lines::{
    SourceLineFanout, assert_targetless_method_line_fanout,
    assert_targetless_method_line_fanout_with_needle,
    assert_targetless_method_owner_kind_line_fanout,
    assert_targetless_method_result_field_line_fanout, assert_targetless_path_line_fanout,
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
    let request_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let parts_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request_parts.rs",
    )?;
    let expected_owner_targets = [
        (
            request_owner,
            request_target,
            "axum-core/src/ext_traits/request.rs:268",
        ),
        (
            parts_owner,
            parts_target,
            "axum-core/src/ext_traits/request_parts.rs:122",
        ),
    ];

    for (owner, expected_target, label) in expected_owner_targets {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_method_receiver(&context, "extract_with_state", &CallReceiver::SelfValue);
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(
            row.targets[0].target_id, expected_target,
            "{label} self.extract_with_state should resolve to the extract_with_state method in the same impl: {row:#?}"
        );
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

    let expected_target_callers = [
        (
            request_target,
            1,
            "RequestExt::extract_with_state caller at request.rs:268",
        ),
        (
            parts_target,
            3,
            "RequestPartsExt::extract_with_state callers at request_parts.rs:122, request_parts.rs:164, and request_parts.rs:186",
        ),
    ];
    for (target, expected_count, label) in expected_target_callers {
        let callers = db.callers_for_target(target)?;
        assert_eq!(
            callers.len(),
            expected_count,
            "{label} should expose every currently supported incoming method caller: {callers:#?}"
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
fn axum_real_target_request_extensions_mut_receiver_statuses_are_proof_backed()
-> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: local and parameter `req.extensions_mut` receiver rows.
    // Source chain:
    //   axum-core/src/ext_traits/request.rs:297 initializes
    //   `let mut req = Request::new(())`.
    //   axum-core/src/ext_traits/request.rs:302 calls `req.extensions_mut()`.
    //   axum/src/extension.rs:184 calls `req.extensions_mut()`.
    // The initializer path is an imported alias to external `http::Request`
    // and remains targetless. Parameter receiver calls with direct external,
    // single-reference, or request-routing receiver proof are now classified as
    // external targetless rows.
    let owner =
        method_id_by_name_and_body_substring(&db, "extract_parts_with_state", "Request::new(())")?;
    let context = db.call_context_for_owner(owner)?;
    let request_new = row_by_path(&context, &["Request", "new"]);
    assert_targetless_status(request_new, CallStatusKind::External);
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
    let initialized_req_receiver = CallReceiver::InitializedLocalBinding {
        name: "req".to_string(),
        init_path: path(&["Request", "new"]),
    };
    assert_owner_method_targetless(
        &db,
        owner,
        "extensions_mut",
        &initialized_req_receiver,
        CallStatusKind::External,
        "axum-core/src/ext_traits/request.rs:302",
    )?;

    let req_receiver = CallReceiver::LocalBinding {
        name: "req".to_string(),
    };
    let external_cases = [
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
        (
            // axum-core/src/extract/default_body_limit.rs:183
            // `DefaultBodyLimit::apply` has `req: &mut Request<B>`.
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
            // `DefaultBodyLimitService::call` uses `mut req: Request<B>`.
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
            // `AddExtension::call` uses `mut req: Request<ResBody>`.
            "axum/src/extension.rs:184",
            method_id_by_name_body_and_file_suffix(
                &db,
                "call",
                "req.extensions_mut().insert(self.value.clone())",
                "axum/src/extension.rs",
            )?,
            1,
        ),
    ];
    for (label, owner, count) in external_cases {
        assert_owner_method_targetless_count(
            &db,
            owner,
            "extensions_mut",
            &req_receiver,
            CallStatusKind::External,
            count,
            label,
        )?;
    }

    assert_targetless_method_rows(
        &db,
        "extensions_mut",
        "LocalBinding",
        Some(&["req"]),
        CallStatusKind::External,
        6,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "extensions_mut",
        "LocalBinding",
        Some(&["req"]),
        CallStatusKind::External,
        &[
            SourceLineFanout {
                file_suffix: "axum/src/extract/nested_path.rs",
                lines: &[95, 103],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/path_router.rs",
                lines: &[336],
            },
            SourceLineFanout {
                file_suffix: "axum-core/src/extract/default_body_limit.rs",
                lines: &[183, 225],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extension.rs",
                lines: &[184],
            },
        ],
    )?;
    assert_targetless_method_rows(
        &db,
        "extensions_mut",
        "InitializedLocalBinding",
        Some(&["req", "Request", "new"]),
        CallStatusKind::External,
        1,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "extensions_mut",
        "InitializedLocalBinding",
        Some(&["req", "Request", "new"]),
        CallStatusKind::External,
        &[SourceLineFanout {
            file_suffix: "axum-core/src/ext_traits/request.rs",
            lines: &[302],
        }],
    )?;

    Ok(())
}

#[test]
fn axum_initialized_associated_constructor_receivers_resolve() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: associated-constructor initialized local receiver rows.
    // Source chain:
    //   axum/src/test_helpers/counting_cloneable_state.rs:15 defines
    //   `CountingCloneableState::new() -> Self`.
    //   axum/src/routing/tests/mod.rs:1165 and :1187 plus
    //   axum/src/routing/tests/fallback.rs:396 bind
    //   `let state = CountingCloneableState::new()`.
    //   Those owners then call `state.clone()` and `state.setup_done()`.
    // Expected traversal: the binding initializer resolves to the local
    // associated function, its `Self` return type proves the receiver type, and
    // both later method calls traverse to the local impl methods.
    let new_target = method_id_by_name_body_and_file_suffix(
        &db,
        "new",
        "setup_done: AtomicBool::new(false)",
        "axum/src/test_helpers/counting_cloneable_state.rs",
    )?;
    let clone_target = method_id_by_name_body_and_file_suffix(
        &db,
        "clone",
        "state.count.fetch_add(1, Ordering::SeqCst)",
        "axum/src/test_helpers/counting_cloneable_state.rs",
    )?;
    let setup_done_target = method_id_by_name_body_and_file_suffix(
        &db,
        "setup_done",
        "self.state.setup_done.store(true, Ordering::SeqCst)",
        "axum/src/test_helpers/counting_cloneable_state.rs",
    )?;

    let cases = [
        (
            "axum/src/routing/tests/mod.rs:1165/1170/1174",
            function_id_by_name_in_module(
                &db,
                &["crate", "routing", "tests"],
                "state_isnt_cloned_too_much",
            )?,
        ),
        (
            "axum/src/routing/tests/mod.rs:1187/1190/1194",
            function_id_by_name_in_module(
                &db,
                &["crate", "routing", "tests"],
                "state_isnt_cloned_too_much_in_layer",
            )?,
        ),
        (
            "axum/src/routing/tests/fallback.rs:396/400/405",
            function_id_by_name_in_module(
                &db,
                &["crate", "routing", "tests", "fallback"],
                "state_isnt_cloned_too_much_with_fallback",
            )?,
        ),
    ];
    let receiver = CallReceiver::InitializedLocalBinding {
        name: "state".to_string(),
        init_path: path(&["CountingCloneableState", "new"]),
    };

    for (label, owner) in cases {
        let context = db.call_context_for_owner(owner)?;
        let new_row = row_by_path(&context, &["CountingCloneableState", "new"]);
        assert_resolved_target(
            new_row,
            new_target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
        );
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label,
                owner,
                target: new_target,
                site_id: new_row.site.id,
                expected_edge_count: 1,
            },
        )?;

        for (method, target) in [("clone", clone_target), ("setup_done", setup_done_target)] {
            let row = row_by_method_receiver(&context, method, &receiver);
            assert_resolved_target(
                row,
                target,
                CallRelationKind::Method,
                CallSiteKind::Method,
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
    }

    Ok(())
}

#[test]
fn axum_real_target_impl_trait_into_parameter_is_external_frontier() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: axum-core/src/error.rs:12-14
    // `Error::new(error: impl Into<BoxError>)` calls `error.into()`.
    // The parameter's `impl Into<BoxError>` bound is source-visible, but the
    // concrete callee is external/prelude dispatch, so the row remains
    // targetless and absent from local traversal.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "new",
        "inner: error.into()",
        "axum-core/src/error.rs",
    )?;
    let receiver = CallReceiver::LocalBinding {
        name: "error".to_string(),
    };
    assert_owner_method_targetless(
        &db,
        owner,
        "into",
        &receiver,
        CallStatusKind::External,
        "axum-core/src/error.rs:14 impl Into parameter",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_self_field_size_hint_is_external_frontier() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `self.0.size_hint` receiver row.
    // Source chain:
    //   axum-core/src/body.rs:127 calls `self.0.size_hint()`.
    // The tuple-field receiver resolves through `Body(BoxBody)`, where
    // `BoxBody` is a local type alias for an external http-body-util type.
    // The call is therefore an external frontier, not a traversable local edge.
    let owner = method_id_by_name_and_body_substring(&db, "size_hint", "self.0.size_hint()")?;
    assert_owner_method_targetless(
        &db,
        owner,
        "size_hint",
        &CallReceiver::SelfField {
            path: vec!["0".to_string()],
        },
        CallStatusKind::External,
        "axum-core/src/body.rs:127",
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "size_hint",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::External,
        &[SourceLineFanout {
            file_suffix: "axum-core/src/body.rs",
            lines: &[127],
        }],
    )?;

    Ok(())
}

#[test]
fn axum_real_target_turbofish_method_receiver_rows_preserve_current_shapes() -> Result<(), DbError>
{
    let db = setup_axum_call_graph_db()?;

    // Matrix: turbofish method call receiver row.
    // Source chain:
    //   axum-core/src/ext_traits/request_parts.rs:164 calls
    //   `parts.extract_with_state::<State<String>, String>(&state)`.
    // Current model: the turbofish row is projected, preserves the two
    // explicit method generic arguments, carries tuple-method-return proof
    // from `Request::new(()).into_parts()`, and resolves through the exact
    // external-return summary for `http::Request::into_parts` returning
    // `http::request::Parts`.
    let generic_owner = function_id_by_name_in_module(
        &db,
        &["crate", "ext_traits", "request_parts", "tests"],
        "extract_with_state",
    )?;
    let target_owner = method_id_by_name_and_body_substring(
        &db,
        "extract_with_state",
        "E::from_request_parts(self, state)",
    )?;
    let generic_context = db.call_context_for_owner(generic_owner)?;
    let generic_receiver = CallReceiver::TupleMethodReturn {
        name: "parts".to_string(),
        method_name: "into_parts".to_string(),
        method_span: (4640, 4669),
        index: 0,
    };
    let generic_row =
        row_by_method_receiver(&generic_context, "extract_with_state", &generic_receiver);
    assert_eq!(generic_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        generic_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(generic_row.targets.len(), 1);
    assert_eq!(generic_row.targets[0].target_id, target_owner);
    assert_eq!(generic_row.targets[0].relation, CallRelationKind::Method);
    assert_eq!(generic_row.targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(generic_row.targets[0].target_kind, CallTargetKind::Method);
    assert_eq!(
        generic_row.site.generic_arg_count,
        Some(2),
        "request_parts.rs:164 should preserve `<State<String>, String>`"
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-core/src/ext_traits/request_parts.rs:164 parts.extract_with_state::<State<String>, String>",
            owner: generic_owner,
            target: target_owner,
            site_id: generic_row.site.id,
            expected_edge_count: 1,
        },
    )?;
    db.project_call_proof_facts_for_owner(generic_owner, "bd:corpus-axum-call-graph")?;
    let generic_site = generic_row.site.id.to_string();
    let target = target_owner.to_string();
    let proof_rows = db.proof_graphrag_context(&generic_site)?;
    assert!(
        proof_rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(generic_site.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
                && proof.resolved_def_id.as_deref() == Some(target.as_str())
        }),
        "request_parts.rs:164 proof context should expose the resolved call_resolution row: {proof_rows:#?}"
    );

    // Source chain:
    //   axum-core/src/ext_traits/request_parts.rs:186 calls
    //   `parts.extract_with_state(state)`.
    // The local-binding receiver has parameter type `&mut Parts`; the resolver
    // now matches that imported external receiver type to the local extension
    // trait impl `impl RequestPartsExt for Parts`.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "from_request_parts",
        "parts.extract_with_state(state)",
        "axum-core/src/ext_traits/request_parts.rs",
    )?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_method_receiver(
        &context,
        "extract_with_state",
        &CallReceiver::LocalBinding {
            name: "parts".to_string(),
        },
    );
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target_owner);
    assert_eq!(row.targets[0].relation, CallRelationKind::Method);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "request_parts.rs:186 parts.extract_with_state local receiver",
            owner,
            target: target_owner,
            site_id: row.site.id,
            expected_edge_count: 1,
        },
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
                status_kind: "Resolved" @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "request_parts.rs:186 should project exactly one resolved local receiver row"
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
    // Current model boundary: external trait receiver dispatch is visible as
    // targetless external frontier rows. The resolver does not fabricate a
    // concrete `tower_service::Service` target.
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
            CallStatusKind::External,
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
        CallStatusKind::External,
        1,
        // axum/src/middleware/response_axum_body.rs:45
        "axum/src/middleware/response_axum_body.rs:45",
    )?;

    let tuple_function_cases = [
        (
            // axum/src/routing/tests/mod.rs:703
            // Local `CountMiddleware<S>(S)` impl inside the test function.
            "axum/src/routing/tests/mod.rs:703",
            local_item_owner_for_parent_with_label(
                &db,
                function_id_by_name_in_module(
                    &db,
                    &["crate", "routing", "tests"],
                    "middleware_still_run_for_unmatched_requests",
                )?,
                "local_impl_method:poll_ready",
            )?,
        ),
        (
            // axum/src/routing/tests/nest.rs:258
            // Local `SetUriExtension<S>(S)` impl inside the test function.
            "axum/src/routing/tests/nest.rs:258",
            local_item_owner_for_parent_with_label(
                &db,
                function_id_by_name_in_module(
                    &db,
                    &["crate", "routing", "tests", "nest"],
                    "outer_middleware_still_see_whole_url",
                )?,
                "local_impl_method:poll_ready",
            )?,
        ),
    ];
    for (label, owner) in tuple_function_cases {
        assert_owner_method_targetless_count(
            &db,
            owner,
            "poll_ready",
            &tuple_receiver,
            CallStatusKind::External,
            1,
            label,
        )?;
    }

    assert_targetless_method_rows(
        &db,
        "poll_ready",
        "SelfField",
        Some(&["inner"]),
        CallStatusKind::External,
        7,
    )?;
    assert_targetless_method_rows(
        &db,
        "poll_ready",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::External,
        3,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "poll_ready",
        "SelfField",
        Some(&["inner"]),
        CallStatusKind::External,
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
        CallStatusKind::External,
        &[SourceLineFanout {
            file_suffix: "axum/src/middleware/response_axum_body.rs",
            lines: &[45],
        }],
    )?;
    assert_targetless_method_owner_kind_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "poll_ready",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::External,
        "LocalItem",
        &[
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
    //   axum-core/src/extract/request_parts.rs:193 calls `Router::new()`
    //   through the `axum::Router` workspace dependency import.
    //   serve/mod.rs calls `router.clone...` in the router examples and
    //   local-address tests. `boxed.rs:134` and `routing/mod.rs:673` call
    //   `self.router.clone()` from local wrapper clone impls.
    //   routing/tests/mod.rs also contributes method-result local-binding
    //   clone rows from chained router builders such as `one.route(...).clone()`
    //   and `bar_or_baz.merge(...).clone()`.
    // Expected traversal: the current caller API exposes 308 resolved
    // `Router::new` rows, one explicit `crate::Router::new` row, and one
    // `Self::new` row, while target expansion traverses 204 incoming candidates
    // for the same target. Typed, self-field, and method-result router clone
    // receiver rows now reach the local `impl<S> Clone for Router<S>` method
    // at routing/mod.rs:90.
    let target = method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?;
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        310,
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
            (path(&["Router", "new"]), 308),
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

    let axum_core_owner = function_id_by_name_in_module(
        &db,
        &["crate", "extract", "request_parts", "tests"],
        "extract_request_parts",
    )?;
    let axum_core_context = db.call_context_for_owner(axum_core_owner)?;
    let axum_core_router_new = row_by_path(&axum_core_context, &["Router", "new"]);
    assert_resolved_target(
        axum_core_router_new,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-core/src/extract/request_parts.rs:193 workspace-import Router::new",
            owner: axum_core_owner,
            target,
            site_id: axum_core_router_new.site.id,
            expected_edge_count: 1,
        },
    )?;

    let domain_id = "bd:corpus-axum-call-graph";
    let mut records = axum_domain_records(domain_id);
    records.push(ploke_test_utils::axum_router_new_dependency_record(
        domain_id,
        axum_core_router_new.site.id,
        axum_core_owner,
        target,
    ));
    db.upsert_proof_fact_values(&records)?;

    let proof_rows = db.proof_symbol_lookup(&target.to_string())?;
    assert_router_new_dependency_root_proof(
        &proof_rows,
        axum_core_router_new.site.id,
        axum_core_owner,
        target,
        "axum-core/src/extract/request_parts.rs:193 workspace-import Router::new",
    );

    let clone_target = method_id_by_name_body_and_file_suffix(
        &db,
        "clone",
        "inner: Arc::clone(&self.inner)",
        "axum/src/routing/mod.rs",
    )?;

    // Matrix source: axum/src/serve/mod.rs:769,770,772,776,780,785,791,797
    // call `router.clone...` from the same typed local binding.
    let router_receiver = CallReceiver::TypedLocalBinding {
        name: "router".to_string(),
        type_path: path(&["Router"]),
    };
    let router_clone_cases = [
        (
            // axum/src/serve/mod.rs:769,770,772,776,780,785,791,797
            // `if_it_compiles_it_works` projects eight `router.clone` rows.
            "axum/src/serve/mod.rs:769,770,772,776,780,785,791,797",
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
    let mut clone_sites = std::collections::BTreeSet::new();
    let mut clone_owners = std::collections::BTreeSet::new();
    for (label, owner, count) in router_clone_cases {
        let context = db.call_context_for_owner(owner)?;
        let rows = context
            .iter()
            .filter(|row| {
                row.site.kind == CallSiteKind::Method
                    && row.site.method.as_deref() == Some("clone")
                    && row.site.receiver.as_ref() == Some(&router_receiver)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows.len(),
            count,
            "{label} should expose exactly {count} resolved Router::clone method row(s): {context:#?}"
        );
        for row in rows {
            assert_resolved_target(
                row,
                clone_target,
                CallRelationKind::Method,
                CallSiteKind::Method,
                CallTargetKind::Method,
            );
            assert_eq!(
                relations_for_site(&db, row.site.id)?.rows.len(),
                1,
                "{label} should preserve exactly one raw call_relation edge per Router::clone site"
            );
            clone_sites.insert(row.site.id);
            clone_owners.insert(owner);
        }
    }

    let app_owner = function_id_by_name_in_module(
        &db,
        &["crate", "routing", "tests"],
        "merging_with_overlapping_method_routes",
    )?;
    let app_receiver = CallReceiver::TypedLocalBinding {
        name: "app".to_string(),
        type_path: path(&["Router"]),
    };
    let app_context = db.call_context_for_owner(app_owner)?;
    let app_clone = row_by_method_receiver(&app_context, "clone", &app_receiver);
    assert_resolved_target(
        app_clone,
        clone_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    assert_eq!(
        relations_for_site(&db, app_clone.site.id)?.rows.len(),
        1,
        "axum/src/routing/tests/mod.rs:804 should preserve exactly one raw call_relation edge for Router::clone"
    );
    clone_sites.insert(app_clone.site.id);
    clone_owners.insert(app_owner);

    let self_router_receiver = CallReceiver::SelfField {
        path: vec!["router".to_string()],
    };
    for (label, owner) in [
        (
            // axum/src/boxed.rs:134
            "axum/src/boxed.rs:134",
            method_id_by_name_body_and_file_suffix(
                &db,
                "clone",
                "router: self.router.clone()",
                "axum/src/boxed.rs",
            )?,
        ),
        (
            // axum/src/routing/mod.rs:673
            "axum/src/routing/mod.rs:673",
            method_id_by_name_body_and_file_suffix(
                &db,
                "clone",
                "router: self.router.clone()",
                "axum/src/routing/mod.rs",
            )?,
        ),
    ] {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_method_receiver(&context, "clone", &self_router_receiver);
        assert_resolved_target(
            row,
            clone_target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );
        assert_eq!(
            relations_for_site(&db, row.site.id)?.rows.len(),
            1,
            "{label} should preserve exactly one raw call_relation edge for Router::clone"
        );
        clone_sites.insert(row.site.id);
        clone_owners.insert(owner);
    }

    let method_result_clone_cases = [
        (
            // axum/src/routing/tests/merge.rs:38-56
            // `multiple_ors_balanced_differently` clones intermediate router
            // results from `one.route(...)`, `two.route(...)`,
            // `three.route(...)`, and `four.route(...)`.
            "axum/src/routing/tests/merge.rs:38-56",
            function_id_by_name_in_module(
                &db,
                &["crate", "routing", "tests", "merge"],
                "multiple_ors_balanced_differently",
            )?,
            12,
        ),
        (
            // axum/src/routing/tests/merge.rs:81
            // `nested_or` clones `bar_or_baz` after `bar.merge(baz)`.
            "axum/src/routing/tests/merge.rs:81",
            function_id_by_name_in_module(
                &db,
                &["crate", "routing", "tests", "merge"],
                "nested_or",
            )?,
            1,
        ),
    ];
    for (label, owner, count) in method_result_clone_cases {
        let context = db.call_context_for_owner(owner)?;
        let rows = context
            .iter()
            .filter(|row| {
                row.site.kind == CallSiteKind::Method
                    && row.site.method.as_deref() == Some("clone")
                    && matches!(
                        row.site.receiver,
                        Some(CallReceiver::MethodResultLocalBinding { .. })
                    )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows.len(),
            count,
            "{label} should expose exactly {count} resolved Router::clone method-result row(s): {context:#?}"
        );
        for row in rows {
            assert_resolved_target(
                row,
                clone_target,
                CallRelationKind::Method,
                CallSiteKind::Method,
                CallTargetKind::Method,
            );
            assert_eq!(
                relations_for_site(&db, row.site.id)?.rows.len(),
                1,
                "{label} should preserve exactly one raw call_relation edge per method-result Router::clone site"
            );
            clone_sites.insert(row.site.id);
            clone_owners.insert(owner);
        }
    }

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
        204,
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

    assert_targetless_path_rows(&db, &["Router", "new"], CallStatusKind::Unsupported, 0)?;
    let clone_callers = db.callers_for_target(clone_target)?;
    assert_eq!(
        clone_callers.len(),
        26,
        "Router::clone should expose the projected typed-local, self-field, and method-result caller rows: {clone_callers:#?}"
    );
    assert_eq!(
        clone_callers
            .iter()
            .filter(|caller| matches!(
                caller.site.receiver,
                Some(CallReceiver::TypedLocalBinding { .. })
            ))
            .count(),
        11,
        "Router::clone should expose the typed-local caller rows: {clone_callers:#?}"
    );
    assert_eq!(
        clone_callers
            .iter()
            .filter(|caller| matches!(caller.site.receiver, Some(CallReceiver::SelfField { .. })))
            .count(),
        2,
        "Router::clone should expose the self-field caller rows: {clone_callers:#?}"
    );
    assert_eq!(
        clone_callers
            .iter()
            .filter(|caller| matches!(
                caller.site.receiver,
                Some(CallReceiver::MethodResultLocalBinding { .. })
            ))
            .count(),
        13,
        "Router::clone should expose the method-result local-binding caller rows: {clone_callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        clone_target,
        &clone_callers,
        "Router::clone typed-local callers",
    )?;
    let caller_sites = clone_callers
        .iter()
        .map(|row| row.site.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        caller_sites, clone_sites,
        "target-centered Router::clone callers should match the owner-scoped source oracle"
    );
    for caller in clone_callers {
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller.target.target_id, clone_target);
        assert_eq!(caller.target.relation, CallRelationKind::Method);
        assert_eq!(caller.target.source_kind, CallSiteKind::Method);
        assert_eq!(caller.target.target_kind, CallTargetKind::Method);
    }

    let clone_incoming = db.expand_call_context(
        CallContextSeed::Target(clone_target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 64,
            ..CallContextOptions::default()
        },
    )?;
    for owner in clone_owners {
        assert!(
            clone_incoming.iter().any(|candidate| {
                candidate.node_id == owner
                    && candidate.relation == ploke_db::CallContextRelation::IncomingCaller
                    && candidate.target_id == clone_target
                    && candidate.distance == 1
                    && clone_sites.contains(&candidate.call_site_id)
            }),
            "Router::clone target expansion should include owner {owner}: {clone_incoming:#?}"
        );
    }

    Ok(())
}

fn assert_router_new_dependency_root_proof(
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
                && row.target_kind.as_deref() == Some("workspace_inherent_method")
                && row.target_name.as_deref() == Some("axum::routing::Router::new")
                && row.target_root.as_deref() == Some("axum/src/routing/mod.rs")
                && row.status.as_deref() == Some("admitted")
                && row.evidence_use.as_deref() == Some("proof_only")
        }),
        "{label} should expose an admitted Router::new dependency-root proof row: {rows:#?}"
    );
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
    // Current model split: path-call and method-call result receivers are
    // structurally projected but remain targetless unless the nested local
    // associated function is already in the resolved subset. `Request::builder`
    // rows whose `Request` segment resolves to the axum-core
    // `Request = http::Request` alias are external frontiers with no local
    // traversal target. Route `oneshot` receiver rows are also external
    // frontiers because the receiver type and `tower::ServiceExt` import are
    // source-visible, but concrete tower dispatch is still not traversed.
    let route_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "oneshot_inner",
        "self.0.clone().oneshot(req)",
        "axum/src/routing/route.rs",
    )?;
    assert_owner_method_result_field_targetless(
        &db,
        route_owner,
        "oneshot",
        "clone",
        &["0"],
        CallStatusKind::External,
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
        CallStatusKind::External,
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
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum-core/src/ext_traits/request.rs:388
            label: "axum-core/src/ext_traits/request.rs:388",
            module_path: &["crate", "ext_traits", "request", "tests"],
            owner: "extract_parts_with_state",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/middleware/from_fn.rs:411
            label: "axum/src/middleware/from_fn.rs:411",
            module_path: &["crate", "middleware", "from_fn", "tests"],
            owner: "basic",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/routing/method_routing.rs:1697
            label: "axum/src/routing/method_routing.rs:1697",
            module_path: &["crate", "routing", "method_routing", "tests"],
            owner: "call",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/routing/tests/get_to_head.rs:22
            label: "axum/src/routing/tests/get_to_head.rs:22",
            module_path: &["crate", "routing", "tests", "get_to_head", "for_handlers"],
            owner: "get_handles_head",
            status: CallStatusKind::External,
        },
        RequestBuilderCase {
            // axum/src/routing/tests/get_to_head.rs:56
            label: "axum/src/routing/tests/get_to_head.rs:56",
            module_path: &["crate", "routing", "tests", "get_to_head", "for_services"],
            owner: "get_handles_head",
            status: CallStatusKind::External,
        },
    ];
    for case in builder_cases {
        let owner = function_id_by_name_in_module(&db, case.module_path, case.owner)?;
        assert_owner_path_targetless(&db, owner, &["Request", "builder"], case.status, case.label)?;
    }

    let from_fn_owner =
        function_id_by_name_in_module(&db, &["crate", "middleware", "from_fn", "tests"], "basic")?;
    let builder_site = assert_owner_path_targetless(
        &db,
        from_fn_owner,
        &["Request", "builder"],
        CallStatusKind::External,
        "axum/src/middleware/from_fn.rs:411",
    )?;
    let projected =
        db.project_call_proof_facts_for_owner(from_fn_owner, "bd:corpus-axum-call-graph")?;
    assert!(
        projected >= 2,
        "from_fn::tests::basic should project Request::builder call_site and call_resolution proof rows: {projected}"
    );
    let site = builder_site.to_string();
    let missing_before = db.proof_graphrag_context("external_dependency_summary_missing")?;
    assert!(
        missing_before.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
        }),
        "Request::builder should start as a fail-closed external-summary frontier: {missing_before:#?}"
    );
    db.upsert_proof_fact_values(&ploke_test_utils::axum_request_builder_summary_records(
        builder_site,
    ))?;
    let blockers = db.proof_blockers()?;
    assert!(
        !blockers.iter().any(|proof| {
            proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.reason == "external_dependency_summary_missing"
        }),
        "linked admitted Request::builder summary should discharge the missing-summary blocker: {blockers:#?}"
    );
    let summary_id = ploke_test_utils::AXUM_REQUEST_BUILDER_SUMMARY_ID;
    let summary_rows = db.proof_graphrag_context(summary_id)?;
    assert!(
        summary_rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.summary_class.as_deref() == Some("audited_no_process_effects")
                && proof.status.as_deref() == Some("admitted")
                && proof.allowed_effects == ["external_summary_boundary".to_string()]
        }),
        "summary-id lookup should expose the admitted Request::builder summary artifact: {summary_rows:#?}"
    );
    assert!(
        summary_rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.resolution_state.as_deref() == Some("externally_summarized")
                && proof.blocker_reason.is_none()
        }),
        "summary-id lookup should expose the externally summarized Request::builder resolution without a blocker: {summary_rows:#?}"
    );
    let context_after = db.call_context_for_owner(from_fn_owner)?;
    let builder_after = row_by_path(&context_after, &["Request", "builder"]);
    assert_targetless_status(builder_after, CallStatusKind::External);
    assert!(
        relations_for_site(&db, builder_after.site.id)?
            .rows
            .is_empty(),
        "proof summary admission must not create a Request::builder call edge"
    );
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
    assert_targetless_path_rows(&db, &["Request", "builder"], CallStatusKind::External, 14)?;
    assert_targetless_path_rows(&db, &["Request", "builder"], CallStatusKind::Unresolved, 0)?;
    assert_targetless_path_rows(&db, &["Request", "builder"], CallStatusKind::Unsupported, 0)?;
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
        CallStatusKind::Unresolved,
        &[],
    )?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Request", "builder"],
        CallStatusKind::Unsupported,
        &[],
    )?;
    assert_targetless_method_result_field_rows(
        &db,
        "oneshot",
        "clone",
        &["0"],
        CallStatusKind::External,
        1,
    )?;
    assert_targetless_method_result_field_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "oneshot",
        "clone",
        &["0"],
        CallStatusKind::External,
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
        CallStatusKind::External,
        1,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "oneshot",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::External,
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
    )?;
    assert_targetless_method_line_fanout_with_needle(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "unwrap",
        "MethodCallResult",
        Some(&["body"]),
        CallStatusKind::Unsupported,
        &[
            SourceLineFanout {
                file_suffix: "axum-core/src/ext_traits/request.rs",
                lines: &[375, 388],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/query.rs",
                lines: &[104],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/raw_form.rs",
                lines: &[65, 71, 95],
            },
            SourceLineFanout {
                file_suffix: "axum/src/form.rs",
                lines: &[156, 164, 226],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/from_fn.rs",
                lines: &[411],
            },
            SourceLineFanout {
                file_suffix: "axum/src/response/mod.rs",
                lines: &[450],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/method_routing.rs",
                lines: &[1697],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/get_to_head.rs",
                lines: &[22, 56],
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
        "body",
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
    // Current contract: awaited-result receiver shapes are visible in the
    // corpus and stay targetless unless a direct inner call gives enough proof
    // to classify the row as an external frontier. The listener row proves
    // `self.sem` is an external `Arc<Semaphore>`, so the
    // `clone().acquire_owned().await.unwrap()` chain is external but still has
    // no local traversal edge. The test-client async-block `unwrap()` row is
    // owned by the nested async executable instead of being flattened into the
    // enclosing method owner.
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
        &CallReceiver::AwaitMethodCallResult {
            method_name: "acquire_owned".to_string(),
        },
        CallStatusKind::External,
        "axum/src/serve/listener.rs:143",
    )?;

    let test_client_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "into_future",
        "self.builder.send().await.unwrap()",
        "axum/src/test_helpers/test_client.rs",
    )?;
    let test_client_context = db.call_context_for_owner(test_client_owner)?;
    assert!(
        test_client_context
            .iter()
            .all(|row| row.site.method.as_deref() != Some("unwrap")),
        "axum/src/test_helpers/test_client.rs:134 async-block unwrap should stay absent under RequestBuilder::into_future: {test_client_context:#?}"
    );

    assert_targetless_method_rows(
        &db,
        "unwrap",
        "AwaitResult",
        None,
        CallStatusKind::Unsupported,
        2,
    )?;
    // Matrix: awaited-result receiver rows from
    // `docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md`.
    // Source chain examples:
    //   axum/src/serve/listener.rs:143
    //     `self.sem.clone().acquire_owned().await.unwrap()`
    //   axum/src/test_helpers/test_client.rs:134
    //     `self.builder.send().await.unwrap()` is represented by a nested
    //     async-block owner, not by the enclosing method owner.
    // Coarse `AwaitResult` rows are targetless: the receiver value is the
    // result of an awaited expression, so the call graph must not fabricate a
    // concrete callee edge. Awaited method-call receivers preserve the inner
    // method name as proof payload instead of collapsing into this coarse
    // bucket. Rows whose inner method target is visible but not exact stay
    // targetless `Unresolved`; rows without enough receiver proof stay
    // targetless `Unsupported`.
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "unwrap",
        "AwaitResult",
        None,
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum/src/extract/connect_info.rs",
            lines: &[326, 367],
        }],
    )?;

    assert_targetless_method_rows(
        &db,
        "unwrap",
        "AwaitMethodCallResult",
        Some(&["acquire_owned"]),
        CallStatusKind::External,
        1,
    )?;

    for (method_name, expected_count) in [("collect", 3), ("oneshot", 5)] {
        assert_targetless_method_rows(
            &db,
            "unwrap",
            "AwaitMethodCallResult",
            Some(&[method_name]),
            CallStatusKind::Unresolved,
            expected_count,
        )?;
    }

    for (method_name, expected_count) in [
        ("bytes", 1),
        ("call", 1),
        ("chunk", 1),
        ("chunk_text", 8),
        ("extract", 3),
        ("extract_parts", 2),
        ("extract_parts_with_state", 2),
        ("extract_with_state", 2),
        ("json", 1),
        ("ready", 1),
        ("send", 4),
        ("send_request", 1),
        ("text", 4),
        ("write_all", 2),
    ] {
        assert_targetless_method_rows(
            &db,
            "unwrap",
            "AwaitMethodCallResult",
            Some(&[method_name]),
            CallStatusKind::Unsupported,
            expected_count,
        )?;
    }

    assert_targetless_method_rows(
        &db,
        "send",
        "MethodCallResult",
        Some(&["get"]),
        CallStatusKind::Unsupported,
        3,
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "send",
        "MethodCallResult",
        Some(&["get"]),
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum/src/extract/connect_info.rs",
            lines: &[330, 371, 416],
        }],
    )
}
