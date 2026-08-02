use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;

use super::super::*;
use super::common::*;
use super::source_lines::{
    SourceLineFanout, assert_targetless_method_line_fanout,
    assert_targetless_method_line_fanout_with_needle,
    assert_targetless_method_owner_kind_line_fanout,
    assert_targetless_method_result_field_line_fanout, assert_targetless_path_line_fanout,
};

const fn fanout(file_suffix: &'static str, lines: &'static [u32]) -> SourceLineFanout {
    SourceLineFanout { file_suffix, lines }
}

fn assert_method_edge(
    db: &Database,
    context: &[CallContextRow],
    owner: Uuid,
    target: Uuid,
    method: &str,
    receiver: &CallReceiver,
    label: &'static str,
) -> Result<Uuid, DbError> {
    let row = row_by_method_receiver(context, method, receiver);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    assert_one_edge_traversal(
        db,
        TraversalExpectation {
            label,
            owner,
            target,
            site_id: row.site.id,
            expected_edge_count: 1,
        },
    )?;
    Ok(row.site.id)
}

fn assert_path_edge(
    db: &Database,
    context: &[CallContextRow],
    owner: Uuid,
    target: Uuid,
    call_path: &[&str],
    relation: CallRelationKind,
    kind: CallTargetKind,
    label: &'static str,
) -> Result<Uuid, DbError> {
    let row = row_by_path(context, call_path);
    assert_resolved_target(row, target, relation, CallSiteKind::Path, kind);
    assert_one_edge_traversal(
        db,
        TraversalExpectation {
            label,
            owner,
            target,
            site_id: row.site.id,
            expected_edge_count: 1,
        },
    )?;
    Ok(row.site.id)
}

fn assert_method_row(
    db: &Database,
    row: &CallContextRow,
    target: Uuid,
    label: &str,
) -> Result<(), DbError> {
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    assert_eq!(
        relations_for_site(db, row.site.id)?.rows.len(),
        1,
        "{label} should preserve exactly one raw method edge"
    );
    Ok(())
}

fn exact_callers(
    db: &Database,
    target: Uuid,
    count: usize,
    label: &str,
) -> Result<Vec<ploke_db::CallCallerRow>, DbError> {
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        count,
        "{label} should expose exactly {count} callers: {callers:#?}"
    );
    assert_sites_match_callers(db, target, &callers, label)?;
    Ok(callers)
}

fn assert_caller_shape(
    caller: &ploke_db::CallCallerRow,
    target: Uuid,
    relation: CallRelationKind,
    source: CallSiteKind,
    kind: CallTargetKind,
    label: &str,
) {
    assert_eq!(caller.status.status, CallStatusKind::Resolved, "{label}");
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact),
        "{label}"
    );
    assert_eq!(caller.target.target_id, target, "{label}");
    assert_eq!(caller.target.relation, relation, "{label}");
    assert_eq!(caller.target.source_kind, source, "{label}");
    assert_eq!(caller.target.target_kind, kind, "{label}");
}

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
        assert!(
            expected_targets.contains(&expected_target),
            "{label} should declare a same-impl extract_with_state target"
        );
        assert_method_edge(
            &db,
            &context,
            owner,
            expected_target,
            "extract_with_state",
            &CallReceiver::SelfValue,
            "extract -> extract_with_state",
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
        let callers = exact_callers(&db, target, expected_count, label)?;
        assert_eq!(callers[0].status.status, CallStatusKind::Resolved);
        assert_eq!(callers[0].target.target_id, target);
    }

    Ok(())
}

#[test]
fn axum_real_target_request_extensions_mut_receiver_statuses_are_proof_backed()
-> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Request::new and all initialized/local req.extensions_mut rows retain exact external
    // targetless status, source fanout, and no-edge/no-traversal behavior.
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
            fanout("axum/src/extract/nested_path.rs", &[95, 103]),
            fanout("axum/src/routing/path_router.rs", &[336]),
            fanout("axum-core/src/extract/default_body_limit.rs", &[183, 225]),
            fanout("axum/src/extension.rs", &[184]),
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
        &[fanout("axum-core/src/ext_traits/request.rs", &[302])],
    )?;

    Ok(())
}

#[test]
fn axum_initialized_associated_constructor_receivers_resolve() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Three CountingCloneableState::new bindings prove the exact initialized receiver for
    // local clone/setup_done calls, and every source call traverses one edge.
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
        assert_path_edge(
            &db,
            &context,
            owner,
            new_target,
            &["CountingCloneableState", "new"],
            CallRelationKind::AssociatedFunction,
            CallTargetKind::Method,
            label,
        )?;

        for (method, target) in [("clone", clone_target), ("setup_done", setup_done_target)] {
            assert_method_edge(&db, &context, owner, target, method, &receiver, label)?;
        }
    }

    Ok(())
}

#[test]
fn axum_real_target_impl_trait_into_parameter_is_external_frontier() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // error.rs:14: impl Into<BoxError> dispatch remains an external targetless frontier.
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

    // body.rs:127: self.0 resolves through external BoxBody and remains edge-free.
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
        &[fanout("axum-core/src/body.rs", &[127])],
    )?;

    Ok(())
}

#[test]
fn axum_real_target_turbofish_method_receiver_rows_preserve_current_shapes() -> Result<(), DbError>
{
    let db = setup_axum_call_graph_db()?;

    // request_parts.rs:164/:186 preserve exact tuple-return/local receiver shapes and 2/0
    // generic counts while resolving to the same local extension method.
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
    assert_resolved_target(
        generic_row,
        target_owner,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
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
    assert_resolved_target(
        row,
        target_owner,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
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

    // All self.inner/self.0 poll_ready forwarding rows retain exact external targetless
    // fanout; tower Service dispatch must not fabricate a concrete local target.
    let inner_receiver = CallReceiver::SelfField {
        path: vec!["inner".to_string()],
    };
    let inner_cases = [
        (
            "axum-core/src/extract/default_body_limit.rs:220",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum-core/src/extract/default_body_limit.rs",
            )?,
        ),
        (
            "axum/src/extension.rs:180",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/extension.rs",
            )?,
        ),
        (
            "axum/src/extract/nested_path.rs:91",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/extract/nested_path.rs",
            )?,
        ),
        (
            "axum/src/middleware/from_extractor.rs:212",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/middleware/from_extractor.rs",
            )?,
        ),
        (
            "axum/src/middleware/from_fn.rs:280 or :358 projected owner",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/middleware/from_fn.rs",
            )?,
        ),
        (
            "axum/src/routing/strip_prefix.rs:36",
            method_id_by_name_body_and_file_suffix(
                &db,
                "poll_ready",
                "self.inner.poll_ready(cx)",
                "axum/src/routing/strip_prefix.rs",
            )?,
        ),
        (
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
        "axum/src/middleware/response_axum_body.rs:45",
    )?;

    let tuple_function_cases = [
        (
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
            fanout("axum-core/src/extract/default_body_limit.rs", &[220]),
            fanout("axum/src/extension.rs", &[180]),
            fanout("axum/src/extract/nested_path.rs", &[91]),
            fanout("axum/src/middleware/from_extractor.rs", &[212]),
            fanout("axum/src/middleware/from_fn.rs", &[358]),
            fanout("axum/src/routing/strip_prefix.rs", &[36]),
            fanout("axum/src/util.rs", &[69]),
        ],
    )?;
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "poll_ready",
        "SelfField",
        Some(&["0"]),
        CallStatusKind::External,
        &[fanout("axum/src/middleware/response_axum_body.rs", &[45])],
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
            fanout("axum/src/routing/tests/mod.rs", &[559]),
            fanout("axum/src/routing/tests/nest.rs", &[258]),
        ],
    )
}

#[test]
fn axum_real_target_router_new_and_router_clone_contracts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Preserve Router::new's exact 308/1/1 path split and 204 incoming candidates, plus all
    // 26 typed-local/self-field/method-result Router::clone callers and their raw edges.
    let target = method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?;
    let callers = exact_callers(&db, target, 310, "Router::new resolved corpus subset")?;
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
        assert_caller_shape(
            caller,
            target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
            "Router::new caller",
        );
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
    assert_path_edge(
        &db,
        &compile_context,
        compile_owner,
        target,
        &["Router", "new"],
        CallRelationKind::AssociatedFunction,
        CallTargetKind::Method,
        "axum/src/serve/mod.rs:756 Router::new",
    )?;

    let complex_owner = function_id_by_name_in_module(
        &db,
        &["crate", "routing", "method_routing", "tests"],
        "building_complex_router",
    )?;
    let complex_context = db.call_context_for_owner(complex_owner)?;
    assert_path_edge(
        &db,
        &complex_context,
        complex_owner,
        target,
        &["crate", "Router", "new"],
        CallRelationKind::AssociatedFunction,
        CallTargetKind::Method,
        "axum/src/routing/method_routing.rs:1494 crate::Router::new",
    )?;

    let axum_core_owner = function_id_by_name_in_module(
        &db,
        &["crate", "extract", "request_parts", "tests"],
        "extract_request_parts",
    )?;
    let axum_core_context = db.call_context_for_owner(axum_core_owner)?;
    let axum_core_site = assert_path_edge(
        &db,
        &axum_core_context,
        axum_core_owner,
        target,
        &["Router", "new"],
        CallRelationKind::AssociatedFunction,
        CallTargetKind::Method,
        "axum-core/src/extract/request_parts.rs:193 workspace-import Router::new",
    )?;

    let domain_id = "bd:corpus-axum-call-graph";
    let mut records = axum_domain_records(domain_id);
    records.push(ploke_test_utils::axum_router_new_dependency_record(
        domain_id,
        axum_core_site,
        axum_core_owner,
        target,
    ));
    db.upsert_proof_fact_values(&records)?;

    let proof_rows = db.proof_symbol_lookup(&target.to_string())?;
    assert_router_new_dependency_root_proof(
        &proof_rows,
        axum_core_site,
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

    let router_receiver = CallReceiver::TypedLocalBinding {
        name: "router".to_string(),
        type_path: path(&["Router"]),
    };
    let router_clone_cases = [
        (
            "axum/src/serve/mod.rs:769,770,772,776,780,785,791,797",
            compile_owner,
            8,
        ),
        (
            "axum/src/serve/mod.rs:692",
            function_id_by_name_in_module(
                &db,
                &["crate", "serve", "tests"],
                "test_serve_local_addr",
            )?,
            1,
        ),
        (
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
            assert_method_row(&db, row, clone_target, label)?;
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
    assert_method_row(
        &db,
        app_clone,
        clone_target,
        "axum/src/routing/tests/mod.rs:804",
    )?;
    clone_sites.insert(app_clone.site.id);
    clone_owners.insert(app_owner);

    let self_router_receiver = CallReceiver::SelfField {
        path: vec!["router".to_string()],
    };
    for (label, owner) in [
        (
            "axum/src/boxed.rs:134",
            method_id_by_name_body_and_file_suffix(
                &db,
                "clone",
                "router: self.router.clone()",
                "axum/src/boxed.rs",
            )?,
        ),
        (
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
        assert_method_row(&db, row, clone_target, label)?;
        clone_sites.insert(row.site.id);
        clone_owners.insert(owner);
    }

    let method_result_clone_cases = [
        (
            "axum/src/routing/tests/merge.rs:38-56",
            function_id_by_name_in_module(
                &db,
                &["crate", "routing", "tests", "merge"],
                "multiple_ors_balanced_differently",
            )?,
            12,
        ),
        (
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
            assert_method_row(&db, row, clone_target, label)?;
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
    let clone_callers = exact_callers(&db, clone_target, 26, "Router::clone callers")?;
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
    let caller_sites = clone_callers
        .iter()
        .map(|row| row.site.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        caller_sites, clone_sites,
        "target-centered Router::clone callers should match the owner-scoped source oracle"
    );
    for caller in clone_callers {
        assert_caller_shape(
            &caller,
            clone_target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
            "Router::clone caller",
        );
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

    // Result receivers retain their precise shape while Request::builder and tower
    // dispatch remain targetless external frontiers.
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
        "axum/src/routing/route.rs:57",
    )?;

    struct RequestBuilderCase {
        label: &'static str,
        module: &'static [&'static str],
        owner: &'static str,
    }

    let builder_cases = [
        RequestBuilderCase {
            label: "axum/src/extract/query.rs:104",
            module: &["crate", "extract", "query", "tests"],
            owner: "check",
        },
        RequestBuilderCase {
            label: "axum/src/extract/raw_form.rs:65",
            module: &["crate", "extract", "raw_form", "tests"],
            owner: "check_query",
        },
        RequestBuilderCase {
            label: "axum/src/form.rs:156",
            module: &["crate", "form", "tests"],
            owner: "check_query",
        },
        RequestBuilderCase {
            label: "axum/src/form.rs:164",
            module: &["crate", "form", "tests"],
            owner: "check_body",
        },
        RequestBuilderCase {
            label: "axum/src/form.rs:226",
            module: &["crate", "form", "tests"],
            owner: "test_incorrect_content_type",
        },
        RequestBuilderCase {
            label: "axum/src/routing/tests/mod.rs:1129",
            module: &["crate", "routing", "tests"],
            owner: "connect_going_to_custom_fallback",
        },
        RequestBuilderCase {
            label: "axum/src/routing/tests/mod.rs:1147",
            module: &["crate", "routing", "tests"],
            owner: "connect_going_to_default_fallback",
        },
        RequestBuilderCase {
            label: "axum/src/serve/mod.rs:799",
            module: &["crate", "serve", "tests"],
            owner: "serving_on_custom_io_type",
        },
        RequestBuilderCase {
            label: "axum-core/src/ext_traits/request.rs:375",
            module: &["crate", "ext_traits", "request", "tests"],
            owner: "extract_parts_without_state",
        },
        RequestBuilderCase {
            label: "axum-core/src/ext_traits/request.rs:388",
            module: &["crate", "ext_traits", "request", "tests"],
            owner: "extract_parts_with_state",
        },
        RequestBuilderCase {
            label: "axum/src/middleware/from_fn.rs:411",
            module: &["crate", "middleware", "from_fn", "tests"],
            owner: "basic",
        },
        RequestBuilderCase {
            label: "axum/src/routing/method_routing.rs:1697",
            module: &["crate", "routing", "method_routing", "tests"],
            owner: "call",
        },
        RequestBuilderCase {
            label: "axum/src/routing/tests/get_to_head.rs:22",
            module: &["crate", "routing", "tests", "get_to_head", "for_handlers"],
            owner: "get_handles_head",
        },
        RequestBuilderCase {
            label: "axum/src/routing/tests/get_to_head.rs:56",
            module: &["crate", "routing", "tests", "get_to_head", "for_services"],
            owner: "get_handles_head",
        },
    ];
    for case in builder_cases {
        let owner = function_id_by_name_in_module(&db, case.module, case.owner)?;
        assert_owner_path_targetless(
            &db,
            owner,
            &["Request", "builder"],
            CallStatusKind::External,
            case.label,
        )?;
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
            fanout("axum/src/extract/query.rs", &[104]),
            fanout("axum/src/extract/raw_form.rs", &[65]),
            fanout("axum/src/form.rs", &[156, 164, 226]),
            fanout("axum-core/src/ext_traits/request.rs", &[375, 388]),
            fanout("axum/src/middleware/from_fn.rs", &[411]),
            fanout("axum/src/routing/method_routing.rs", &[1697]),
            fanout("axum/src/routing/tests/get_to_head.rs", &[22, 56]),
            fanout("axum/src/routing/tests/mod.rs", &[1129, 1147]),
            fanout("axum/src/serve/mod.rs", &[799]),
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
        &[fanout("axum/src/routing/route.rs", &[51])],
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
        &[fanout("axum/src/routing/route.rs", &[57])],
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
            fanout("axum-core/src/ext_traits/request.rs", &[375, 388]),
            fanout("axum/src/extract/query.rs", &[104]),
            fanout("axum/src/extract/raw_form.rs", &[65, 71, 95]),
            fanout("axum/src/form.rs", &[156, 164, 226]),
            fanout("axum/src/middleware/from_fn.rs", &[411]),
            fanout("axum/src/response/mod.rs", &[450]),
            fanout("axum/src/routing/method_routing.rs", &[1697]),
            fanout("axum/src/routing/tests/get_to_head.rs", &[22, 56]),
            fanout("axum/src/routing/tests/mod.rs", &[1129, 1147]),
            fanout("axum/src/serve/mod.rs", &[799]),
        ],
        "body",
    )
}

#[test]
fn axum_real_target_await_result_receivers_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Awaited receiver shapes remain targetless; the nested async-block unwrap is
    // not flattened into its enclosing method owner.
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
    assert_targetless_method_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "unwrap",
        "AwaitResult",
        None,
        CallStatusKind::Unsupported,
        &[fanout("axum/src/extract/connect_info.rs", &[326, 367])],
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
        &[fanout("axum/src/extract/connect_info.rs", &[330, 371, 416])],
    )
}
