use super::super::*;
use super::common::*;
use super::source_lines::{
    SourceLineFanout, assert_targetless_dynamic_line_fanout_by_method_arg_count,
    assert_targetless_method_owner_kind_line_fanout,
};
use ploke_db::CallPathOptions;
use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;

#[test]
fn axum_closure_body_call_is_documented_unsupported_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum-macros/src/from_ref.rs:23
    //   .map(|(idx, field)| expand_field(&item.ident, idx, field))
    //
    // Contract after regenerating the axum fixture with closure owners:
    // `expand_field(...)` is projected on the nested closure executable owner,
    // not flattened into the parent `from_ref::expand` function.
    let owner = function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand")?;
    let target = function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand_field")?;

    let context = db.call_context_for_owner(owner)?;
    assert!(
        context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&path(&["expand_field"]))),
        "closure-body expand_field call should remain owned by the nested closure, not by from_ref::expand: {context:#?}"
    );

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        1,
        "expand_field should expose the one closure-body caller from axum-macros/src/from_ref.rs:23: {callers:#?}"
    );
    let caller = &callers[0];
    assert_eq!(
        owner_kind_for_call_body_owner(&db, caller.site.owner_id)?,
        "Closure",
        "expand_field call should be owned by the nested closure executable owner"
    );
    assert_eq!(caller.site.path.as_ref(), Some(&path(&["expand_field"])));
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.relation, CallRelationKind::Function);
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(caller.target.target_kind, CallTargetKind::Function);
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "axum-macros/src/from_ref.rs:23 closure-body expand_field call",
    )?;
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-macros/src/from_ref.rs:23 closure-body expand_field",
            owner: caller.site.owner_id,
            target,
            site_id: caller.site.id,
            expected_edge_count: 1,
        },
    )?;

    Ok(())
}

#[derive(Clone, Copy)]
struct DynamicGap {
    method_name: &'static str,
    body_marker: &'static str,
    source_line: u32,
    expected_args: u32,
    expected_path: &'static [&'static str],
    source: &'static str,
}

#[test]
fn axum_dynamic_callable_fields_preserve_supported_and_unsupported_boundaries()
-> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum/src/boxed.rs:85  (self.into_route)(self.handler, state)
    //     `into_route: fn(H, S) -> Route` is initialized by the unique
    //     `MakeErasedHandler { into_route: |handler, state| ... }` closure in
    //     `BoxedIntoRoute::from_handler` at boxed.rs:23-25.
    //   axum/src/boxed.rs:120 (self.into_route)(self.router, state)
    //     construction site not found in selected axum/src.
    //   axum/src/boxed.rs:159 (self.layer)(self.inner.into_route(state))
    //   axum/src/boxed.rs:163 (self.layer)(self.inner.into_route(state)).call(request)
    //     boxed dynamic `LayerFn` trait object supplied by
    //     `BoxedIntoRoute::map(self, f)`. The finite visible candidate set is
    //     supplied by the recorded method-body closure bindings and the
    //     transparent `map_inner!` source expression:
    //       axum/src/routing/method_routing.rs:1026 `layer_fn`
    //       axum/src/routing/method_routing.rs:1070 `layer_fn`
    //       axum/src/routing/mod.rs:307 `|route| route.layer(layer)`
    //   axum/src/serve/listener.rs:236 (self.tap_fn)(&mut io)
    //     generic `FnMut` field supplied by caller.
    let supported_owner = method_id_by_name_and_body_substring(
        &db,
        "into_route",
        "(self.into_route)(self.handler, state)",
    )?;
    let supported_context = db.call_context_for_owner(supported_owner)?;
    let supported_rows = supported_context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic)
        .collect::<Vec<_>>();
    assert_eq!(
        supported_rows.len(),
        1,
        "axum/src/boxed.rs:85 should project one dynamic function-pointer field call: {supported_context:#?}"
    );
    let supported = supported_rows[0];
    assert_eq!(
        supported.site.path.as_ref(),
        Some(&path(&["self", "into_route"]))
    );
    assert_eq!(supported.site.arg_count, Some(2));
    assert_eq!(supported.status.status, CallStatusKind::Resolved);
    assert_eq!(
        supported.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    let closure = supported
        .targets
        .first()
        .map(|target| target.target_id)
        .unwrap_or_else(|| {
            panic!(
                "axum/src/boxed.rs:85 should resolve to the MakeErasedHandler initializer closure: {supported:#?}"
            )
        });
    assert_resolved_target(
        supported,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    assert_eq!(
        relations_for_site(&db, supported.site.id)?.rows.len(),
        1,
        "axum/src/boxed.rs:85 should persist exactly one dynamic closure edge"
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/boxed.rs:85 self.into_route closure field",
            owner: supported_owner,
            target: closure,
            site_id: supported.site.id,
            expected_edge_count: 1,
        },
    )?;

    let method_router_layer = method_id_by_name_and_body_substring(
        &db,
        "layer",
        "let layer_fn = move |route: Route<E>| route.layer(layer.clone());",
    )?;
    let method_router_route_layer = method_id_by_name_and_body_substring(
        &db,
        "route_layer",
        "let layer_fn = move |svc| Route::new(layer.layer(svc));",
    )?;
    let router_layer = method_id_by_name_body_and_file_suffix(
        &db,
        "layer",
        "catch_all_fallback: this.catch_all_fallback.map(|route| route.layer(layer))",
        "axum/src/routing/mod.rs",
    )?;
    let mut expected_layer_candidates = vec![
        closure_owner_for_method_parent(&db, method_router_layer)?,
        closure_owner_for_method_parent(&db, method_router_route_layer)?,
        closure_owner_for_method_parent(&db, router_layer)?,
    ];
    expected_layer_candidates.sort_unstable();

    for (method_name, body_marker, source_line) in [
        (
            "into_route",
            "(self.layer)(self.inner.into_route(state))",
            159,
        ),
        (
            "call_with_state",
            "(self.layer)(self.inner.into_route(state)).call(request)",
            163,
        ),
    ] {
        let owner = method_id_by_name_and_body_substring(&db, method_name, body_marker)?;
        let context = db.call_context_for_owner(owner)?;
        let dynamic_rows = context
            .iter()
            .filter(|row| row.site.kind == CallSiteKind::Dynamic)
            .collect::<Vec<_>>();

        assert_eq!(
            dynamic_rows.len(),
            1,
            "axum/src/boxed.rs:{source_line} should project one dynamic layer callable field call: {context:#?}"
        );
        let row = dynamic_rows[0];
        assert_eq!(row.site.path.as_ref(), Some(&path(&["self", "layer"])));
        assert_eq!(row.site.arg_count, Some(1));
        assert_eq!(row.status.status, CallStatusKind::Ambiguous);
        assert_eq!(row.status.resolution, None);
        assert_eq!(
            row.targets.len(),
            expected_layer_candidates.len(),
            "axum/src/boxed.rs:{source_line} should expose the finite recorded layer closure candidates: {row:#?}"
        );
        assert!(row.targets.iter().all(|target| {
            target.relation == CallRelationKind::DynamicClosure
                && target.source_kind == CallSiteKind::Dynamic
                && target.target_kind == CallTargetKind::Closure
        }));
        let mut actual = row
            .targets
            .iter()
            .map(|target| target.target_id)
            .collect::<Vec<_>>();
        actual.sort_unstable();
        assert_eq!(
            actual, expected_layer_candidates,
            "axum/src/boxed.rs:{source_line} should preserve the reviewed layer closure candidates"
        );
        assert_eq!(
            relations_for_site(&db, row.site.id)?.rows.len(),
            expected_layer_candidates.len(),
            "axum/src/boxed.rs:{source_line} should persist the ambiguous layer closure candidates"
        );
        assert_no_traversal_candidates_for_site(
            &db,
            owner,
            row.site.id,
            &format!("axum/src/boxed.rs:{source_line} self.layer ambiguous candidates"),
        )?;
        let facts = db.call_proof_facts_for_owner(owner, "bd:corpus-axum-call-graph")?;
        let site = row.site.id.to_string();
        let resolution = facts
            .iter()
            .find(|fact| {
                fact.get("fact_kind") == Some(&serde_json::json!("call_resolution"))
                    && fact.get("call_site_id") == Some(&serde_json::json!(site))
            })
            .unwrap_or_else(|| {
                panic!(
                    "axum/src/boxed.rs:{source_line} self.layer should project a call_resolution proof fact: {facts:#?}"
                )
            });
        assert!(
            resolution.get("resolution_state") == Some(&serde_json::json!("ambiguous"))
                && resolution
                    .get("candidate_def_ids")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|candidates| candidates.len() == expected_layer_candidates.len()),
            "axum/src/boxed.rs:{source_line} self.layer should project ambiguous proof candidates: {resolution:#?}"
        );
    }

    let unsupported_cases = [
        DynamicGap {
            method_name: "into_route",
            body_marker: "(self.into_route)(self.router, state)",
            source_line: 120,
            expected_args: 2,
            expected_path: &["self", "into_route"],
            source: "axum/src/boxed.rs:120",
        },
        DynamicGap {
            method_name: "accept",
            body_marker: "(self.tap_fn)(&mut io)",
            source_line: 236,
            expected_args: 1,
            expected_path: &["self", "tap_fn"],
            source: "axum/src/serve/listener.rs:236",
        },
    ];

    for case in unsupported_cases {
        let owner = method_id_by_name_and_body_substring(&db, case.method_name, case.body_marker)?;
        let context = db.call_context_for_owner(owner)?;
        let dynamic_rows = context
            .iter()
            .filter(|row| row.site.kind == CallSiteKind::Dynamic)
            .collect::<Vec<_>>();

        assert_eq!(
            dynamic_rows.len(),
            1,
            "matrix source line {} should project one dynamic callable field call: {context:#?}",
            case.source_line
        );
        let row = dynamic_rows[0];
        assert_eq!(
            row.site.path.as_ref(),
            Some(&path(case.expected_path)),
            "dynamic callable row at matrix source line {} should preserve the self-field callee path",
            case.source_line
        );
        assert_eq!(
            row.site.arg_count,
            Some(case.expected_args),
            "dynamic callable row at matrix source line {} should preserve argument count",
            case.source_line
        );
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert!(
            row.targets.is_empty(),
            "unsupported dynamic callable field call should remain targetless: {dynamic_rows:#?}"
        );
        assert!(
            relations_for_site(&db, row.site.id)?.rows.is_empty(),
            "dynamic callable field row at matrix source line {} should have zero persisted call edges",
            case.source_line
        );
        let label = format!("axum dynamic callable matrix line {}", case.source_line);
        assert_no_traversal_candidates_for_site(&db, owner, row.site.id, &label)?;

        let field = case
            .expected_path
            .last()
            .copied()
            .expect("dynamic callable field path");
        db.upsert_proof_fact_values(&[
            ploke_test_utils::axum_callable_field_runtime_dispatch_blocker(
                row.site.id,
                field,
                case.source,
            ),
        ])?;
        let needs = db.runtime_dispatch_needs_for_owner(
            owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 16,
            },
        )?;
        let need = needs
            .iter()
            .find(|need| need.call_site.site.id == row.site.id)
            .unwrap_or_else(|| {
                panic!(
                    "{} should be listed as an owner-scoped runtime-dispatch need: {needs:#?}",
                    case.source
                )
            });
        assert!(
            need.paths_to_owner.is_empty(),
            "{} is a direct dynamic callable frontier and should not need an intermediate path: {need:#?}",
            case.source
        );
        assert!(
            need.blocker_reasons
                .iter()
                .any(|reason| reason == "dynamic_dispatch_unbounded"),
            "{} should retain the dynamic dispatch blocker: {need:#?}",
            case.source
        );
        assert!(
            relations_for_site(&db, row.site.id)?.rows.is_empty(),
            "{} runtime-dispatch proof queue must not fabricate a callable-field edge",
            case.source
        );

        db.upsert_proof_fact_values(&[
            ploke_test_utils::axum_callable_field_runtime_dispatch_summary(
                row.site.id,
                field,
                case.source,
            ),
        ])?;
        let after = db.runtime_dispatch_needs_for_owner(
            owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 16,
            },
        )?;
        assert!(
            after
                .iter()
                .all(|need| need.call_site.site.id != row.site.id),
            "{} admitted runtime-dispatch summary should discharge the proof-authoring need: {after:#?}",
            case.source
        );
        assert!(
            relations_for_site(&db, row.site.id)?.rows.is_empty(),
            "{} admitted runtime-dispatch summary must not fabricate a callable-field edge",
            case.source
        );
    }

    assert_targetless_dynamic_line_fanout_by_method_arg_count(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "into_route",
        2,
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum/src/boxed.rs",
            lines: &[120],
        }],
        "(self.into_route)",
    )?;
    assert_targetless_dynamic_line_fanout_by_method_arg_count(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "accept",
        1,
        CallStatusKind::Unsupported,
        &[SourceLineFanout {
            file_suffix: "axum/src/serve/listener.rs",
            lines: &[236],
        }],
        "(self.tap_fn)",
    )
}

#[test]
fn axum_map_inner_source_rows_are_visible_and_fail_closed() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Ground truth:
    //   axum/src/routing/mod.rs:129-138 defines `map_inner!`.
    //   axum/src/routing/mod.rs:304-308 invokes it from `Router::layer`.
    //   The reviewed source input includes
    //   `catch_all_fallback: this.catch_all_fallback.map(|route| route.layer(layer))`.
    //
    // Current model contract: bounded transparent-source extraction visits the
    // inspected `$expr`, so the `map(...)` receiver row and nested
    // `route.layer(layer)` closure row are visible. The receiver for
    // `this.catch_all_fallback.map(...)` still lacks a local binding/type proof,
    // so both rows stay targetless and contribute no traversal edge.
    let router_layer = method_id_by_name_body_and_file_suffix(
        &db,
        "layer",
        "catch_all_fallback: this.catch_all_fallback.map(|route| route.layer(layer))",
        "axum/src/routing/mod.rs",
    )?;
    assert_owner_method_targetless(
        &db,
        router_layer,
        "map",
        &CallReceiver::Unsupported,
        CallStatusKind::Unsupported,
        "axum/src/routing/mod.rs:307 map_inner Router::layer catch_all_fallback.map",
    )?;

    assert_targetless_method_owner_kind_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "layer",
        "LocalBinding",
        Some(&["route"]),
        CallStatusKind::Unsupported,
        "Closure",
        &[SourceLineFanout {
            file_suffix: "axum/src/routing/mod.rs",
            lines: &[307],
        }],
    )
}

#[test]
fn axum_macro_callback_rows_are_visible_or_explicitly_absent() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: macro callback rows.
    // Source chain:
    //   axum-macros/src/lib.rs:724 calls
    //   `expand(syn::parse(input).and_then(f))`.
    //   lib.rs:734-738 immediately invokes an IIFE closure whose body calls
    //   `f(attr, input)`.
    //   axum-macros/src/from_request/mod.rs:200-203 immediately invokes an
    //   IIFE closure while deriving enum state.
    // Current model contract: the `syn::parse` path and `and_then(f)` receiver
    // are projected in `expand_with`. The `and_then(f)` call exposes finite
    // method-callback candidates from known `expand_with` callers without
    // admitting resolved traversal. `expand_attr_with` projects its IIFE dynamic
    // call as a closure target, and the closure-owned callable-parameter call
    // `f(attr, input)` is visible as an ambiguous finite closure-candidate row
    // without admitting resolved traversal. The `from_request::expand`
    // enum-state IIFE also resolves to its closure target.
    let expand_with = function_id_by_name_in_module(&db, &["crate"], "expand_with")?;
    let expand_with_context = db.call_context_for_owner(expand_with)?;

    let parse_row = row_by_path(&expand_with_context, &["syn", "parse"]);
    assert_external_targetless(parse_row);
    assert_no_traversal_candidates_for_site(
        &db,
        expand_with,
        parse_row.site.id,
        "axum-macros/src/lib.rs:724 syn::parse external callback setup",
    )?;

    let and_then = row_by_method_receiver(
        &expand_with_context,
        "and_then",
        &CallReceiver::PathCallResult {
            path: path(&["syn", "parse"]),
        },
    );
    assert_eq!(and_then.status.status, CallStatusKind::Ambiguous);
    assert_eq!(and_then.status.resolution, None);
    assert_eq!(
        and_then.targets.len(),
        4,
        "axum-macros/src/lib.rs:724 and_then(f) should expose four finite method-callback candidates: {and_then:#?}"
    );
    let from_ref_expand = function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand")?;
    assert!(
        and_then.targets.iter().any(|target| {
            target.relation == CallRelationKind::MethodCallbackFunction
                && target.source_kind == CallSiteKind::Method
                && target.target_kind == CallTargetKind::Function
                && target.target_id == from_ref_expand
        }),
        "axum-macros/src/lib.rs:724 and_then(f) should include from_ref::expand from lib.rs:715: {and_then:#?}"
    );
    assert_eq!(
        and_then
            .targets
            .iter()
            .filter(|target| {
                target.relation == CallRelationKind::MethodCallbackClosure
                    && target.source_kind == CallSiteKind::Method
                    && target.target_kind == CallTargetKind::Closure
            })
            .count(),
        3,
        "axum-macros/src/lib.rs:724 and_then(f) should include the three closure arguments from lib.rs:377,426,665: {and_then:#?}"
    );
    assert_eq!(
        relations_for_site(&db, and_then.site.id)?.rows.len(),
        4,
        "axum-macros/src/lib.rs:724 and_then(f) should persist four ambiguous method-callback candidates"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        expand_with,
        and_then.site.id,
        "axum-macros/src/lib.rs:724 and_then(f)",
    )?;
    let expand_with_facts =
        db.call_proof_facts_for_owner(expand_with, "bd:corpus-axum-call-graph")?;
    let and_then_site = and_then.site.id.to_string();
    let and_then_resolution = expand_with_facts
        .iter()
        .find(|fact| {
            fact.get("fact_kind") == Some(&serde_json::json!("call_resolution"))
                && fact.get("call_site_id") == Some(&serde_json::json!(and_then_site))
        })
        .unwrap_or_else(|| {
            panic!("axum-macros/src/lib.rs:724 and_then(f) should project a call_resolution proof fact: {expand_with_facts:#?}")
        });
    assert!(
        and_then_resolution.get("resolution_state") == Some(&serde_json::json!("ambiguous"))
            && and_then_resolution
                .get("candidate_def_ids")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|candidates| candidates.len() == 4),
        "axum-macros/src/lib.rs:724 and_then(f) should project an ambiguous proof row with four candidates: {and_then_resolution:#?}"
    );

    let expand_attr_with = function_id_by_name_in_module(&db, &["crate"], "expand_attr_with")?;
    let expand_attr_context = db.call_context_for_owner(expand_attr_with)?;
    let iife = resolved_dynamic_iife_row(&expand_attr_context, "expand_attr IIFE");
    let iife_target = iife.targets[0].target_id;
    assert_resolved_target(
        iife,
        iife_target,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-macros/src/lib.rs:734-738 IIFE dynamic call",
            owner: expand_attr_with,
            target: iife_target,
            site_id: iife.site.id,
            expected_edge_count: 1,
        },
    )?;
    let iife_context = db.call_context_for_owner(iife_target)?;
    let callback = row_by_path(&iife_context, &["f"]);
    assert_eq!(
        owner_kind_for_call_body_owner(&db, callback.site.owner_id)?,
        "Closure"
    );
    assert_eq!(callback.site.kind, CallSiteKind::Path);
    assert_eq!(callback.site.arg_count, Some(2));
    assert_eq!(callback.status.status, CallStatusKind::Ambiguous);
    assert_eq!(callback.status.resolution, None);
    assert_eq!(
        callback.targets.len(),
        2,
        "axum-macros/src/lib.rs:737 f(attr, input) should expose two finite closure candidates: {callback:#?}"
    );
    assert!(callback.targets.iter().all(|target| {
        target.relation == CallRelationKind::Closure
            && target.source_kind == CallSiteKind::Path
            && target.target_kind == CallTargetKind::Closure
    }));
    assert_eq!(
        relations_for_site(&db, callback.site.id)?.rows.len(),
        2,
        "axum-macros/src/lib.rs:737 f(attr, input) should persist two ambiguous closure candidates"
    );
    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(iife_target),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        outgoing
            .iter()
            .all(|candidate| candidate.call_site_id != callback.site.id),
        "axum-macros/src/lib.rs:737 f(attr, input) should not admit resolved traversal candidates: {outgoing:#?}"
    );
    let facts = db.call_proof_facts_for_owner(iife_target, "bd:corpus-axum-call-graph")?;
    let callback_site = callback.site.id.to_string();
    let resolution = facts
        .iter()
        .find(|fact| {
            fact.get("fact_kind") == Some(&serde_json::json!("call_resolution"))
                && fact.get("call_site_id") == Some(&serde_json::json!(callback_site))
        })
        .unwrap_or_else(|| {
            panic!("axum-macros/src/lib.rs:737 f(attr, input) should project a call_resolution proof fact: {facts:#?}")
        });
    assert!(
        resolution.get("resolution_state") == Some(&serde_json::json!("ambiguous"))
            && resolution
                .get("candidate_def_ids")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|candidates| candidates.len() == 2),
        "axum-macros/src/lib.rs:737 f(attr, input) should project an ambiguous proof row with two candidates: {resolution:#?}"
    );

    let from_request_expand =
        function_id_by_name_in_module(&db, &["crate", "from_request"], "expand")?;
    let from_request_context = db.call_context_for_owner(from_request_expand)?;
    let enum_state_iife =
        resolved_dynamic_iife_row(&from_request_context, "from_request enum-state IIFE");
    let enum_state_target = enum_state_iife.targets[0].target_id;
    assert_resolved_target(
        enum_state_iife,
        enum_state_target,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-macros/src/from_request/mod.rs:200-203 IIFE dynamic call",
            owner: from_request_expand,
            target: enum_state_target,
            site_id: enum_state_iife.site.id,
            expected_edge_count: 1,
        },
    )?;

    Ok(())
}

fn resolved_dynamic_iife_row<'a>(
    context: &'a [ploke_db::CallContextRow],
    label: &str,
) -> &'a ploke_db::CallContextRow {
    let rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic && row.site.arg_count == Some(0))
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        1,
        "{label} should expose one zero-argument dynamic IIFE row: {context:#?}"
    );
    rows[0]
}
