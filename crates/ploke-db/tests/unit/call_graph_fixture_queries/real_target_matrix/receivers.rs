use super::super::*;
use super::common::*;

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
    //   axum-core/src/ext_traits/request.rs:302 calls `req.extensions_mut()`.
    //   axum/src/extension.rs:184 calls `req.extensions_mut()`.
    // Current model gap: both are projected as method callsites on a local
    // binding named `req`, but they remain unresolved external receiver calls.
    assert_targetless_method_rows(
        &db,
        "extensions_mut",
        "LocalBinding",
        Some(&["req"]),
        CallStatusKind::Unresolved,
        6,
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
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_method_receiver(
        &context,
        "size_hint",
        &CallReceiver::SelfField {
            path: vec!["0".to_string()],
        },
    );
    assert_targetless_status(row, CallStatusKind::Unsupported);

    Ok(())
}

#[test]
fn axum_real_target_turbofish_local_receiver_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: turbofish method call receiver row.
    // Source chain:
    //   axum-core/src/ext_traits/request_parts.rs:164 calls
    //   `parts.extract_with_state::<String, _>(&state)`.
    // Current model gap: the local receiver row is projected, but it does not
    // resolve back to the `RequestPartsExt::extract_with_state` impl yet.
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
    )
}

#[test]
fn axum_real_target_router_new_and_router_clone_contracts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Router::new` / typed `router.clone`.
    // Source chain:
    //   axum/src/routing/mod.rs:162 defines `Router::new`.
    //   axum/src/serve/mod.rs:756 calls `Router::new()`.
    //   serve/mod.rs:769,770,772,776,780,785 call `router.clone...`.
    // Expected traversal: the current caller API exposes 142 resolved
    // `Router::new` rows, while target expansion traverses 121 incoming
    // candidates for the same target. Typed router clone receiver rows remain
    // targetless because Clone dispatch is not modeled yet.
    let target = method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?;
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        142,
        "Router::new should expose the current resolved corpus subset: {callers:#?}"
    );
    for caller in &callers {
        assert_eq!(caller.site.path, Some(path(&["Router", "new"])));
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Method);
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
        121,
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
    assert_targetless_method_rows(
        &db,
        "clone",
        "TypedLocalBinding",
        Some(&["app", "Router"]),
        CallStatusKind::Unresolved,
        1,
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
    assert_targetless_path_rows(&db, &["Request", "builder"], CallStatusKind::External, 8)?;
    assert_targetless_path_rows(&db, &["Request", "builder"], CallStatusKind::Unsupported, 6)?;
    assert_targetless_method_rows(
        &db,
        "oneshot",
        "MethodCallResult",
        Some(&["clone"]),
        CallStatusKind::Unsupported,
        1,
    )?;
    assert_targetless_method_rows(
        &db,
        "oneshot",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::Unsupported,
        1,
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
