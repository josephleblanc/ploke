use std::borrow::Cow;

use ploke_core::rag_types::{CallCalleeInfo, CallContextInfo, CallStatusKind, CallTargetKind};
use ploke_db::ProofGraphStore;
use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
    get_code_edges::{CodeItemEdges, EdgesParams},
};
use uuid::Uuid;

use crate::call_graph_tool_support::{
    AmbiguousDynamicToolCase, AmbiguousDynamicToolFixture, DynamicToolCase, DynamicToolFixture,
    IfuncToolCase, IfuncToolFixture, PathToolCase, PathToolFixture, ReceiverToolCase,
    ReceiverToolFixture, assert_admitted_external_summary_effect,
    assert_admitted_external_summary_proof, assert_admitted_macro_boundary_summary_proof,
    assert_ambiguous_candidate_proof, assert_ambiguous_dynamic_candidates_with_relation,
    assert_dynamic_context, assert_dynamic_proof,
    assert_handle_error_future_poll_producer_flow_payload, assert_ifunc_context,
    assert_ifunc_proof, assert_ifunc_unsafe_calls, assert_method_context, assert_method_proof,
    assert_path_blocker_proof, assert_path_context, assert_path_context_absent,
    assert_path_resolution_proof, assert_resolved_method_context, assert_resolved_method_proof,
    assert_resolved_path_context_count, assert_resolved_path_context_target,
    assert_resolved_path_proof, assert_runtime_dispatch_blocker,
    assert_self_field_binding_evidence, request_parts_extract_target, ui_field,
};

#[tokio::test]
async fn code_item_lookup_returns_dynamic_targetless_real_corpus_rows() {
    for case in DynamicToolCase::AXUM {
        let fixture = DynamicToolFixture::new(case).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("dynamic-targetless-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");
        let runtime_needs = payload
            .get("runtime_dispatch_needs")
            .and_then(serde_json::Value::as_array)
            .expect("runtime_dispatch_needs array");
        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chains:
        //   axum/src/boxed.rs:120 calls `(self.into_route)(self.router, state)`.
        //   axum/src/serve/listener.rs:236 calls `(self.tap_fn)(&mut io)`.
        // Expected traversal: exact owner lookup exposes the structural
        // dynamic call_site and blocked call_resolution rows, with zero callee
        // targets until callable-field proof exists.
        let site_id = assert_dynamic_context(
            call_context,
            fixture.owner,
            fixture.case.expected_path,
            fixture.case.expected_arg_count,
            fixture.case.label,
            "lookup",
        );
        assert_dynamic_proof(
            proof_context,
            fixture.owner,
            site_id,
            fixture.case.build_domain(),
            fixture.case.label,
            "lookup",
        );
        if let Some(expected_path) = fixture.case.expected_path {
            assert_self_field_binding_evidence(
                proof_context,
                fixture.owner,
                site_id,
                fixture.case.build_domain(),
                expected_path,
                "blocked",
                fixture.case.label,
                "lookup",
            );
        }
        if fixture.case.expects_runtime_dispatch_blocker() {
            assert_runtime_dispatch_blocker(proof_context, site_id, fixture.case.label, "lookup");
            assert_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "lookup");
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing dynamic targetless call context"
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 2,
            "code_item_lookup should surface dynamic targetless proof rows"
        );
    }
}

#[tokio::test]
async fn code_item_lookup_omits_admitted_runtime_dispatch_summary_needs() {
    for case in DynamicToolCase::AXUM {
        let fixture = DynamicToolFixture::new(case).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let initial = CodeItemLookup::execute(
            params.clone(),
            fixture.ctx("dynamic-targetless-summary-before"),
        )
        .await
        .unwrap_or_else(|err| panic!("{} initial code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&initial.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let runtime_needs = payload
            .get("runtime_dispatch_needs")
            .and_then(serde_json::Value::as_array)
            .expect("runtime_dispatch_needs array");
        let site_id = assert_dynamic_context(
            call_context,
            fixture.owner,
            fixture.case.expected_path,
            fixture.case.expected_arg_count,
            fixture.case.label,
            "lookup",
        );
        assert_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "lookup");

        let expected_path = fixture
            .case
            .expected_path
            .expect("dynamic callable field path");
        let field = expected_path
            .last()
            .copied()
            .expect("dynamic callable field");
        fixture
            .state
            .db
            .upsert_proof_fact_values(&[
                ploke_test_utils::axum_callable_field_runtime_dispatch_summary(
                    site_id,
                    field,
                    fixture.case.label,
                ),
            ])
            .unwrap_or_else(|err| {
                panic!(
                    "{} runtime dispatch summary insert: {err}",
                    fixture.case.label
                )
            });

        let result =
            CodeItemLookup::execute(params, fixture.ctx("dynamic-targetless-summary-after"))
                .await
                .unwrap_or_else(|err| {
                    panic!("{} summary code_item_lookup: {err}", fixture.case.label)
                });
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let runtime_needs = payload
            .get("runtime_dispatch_needs")
            .and_then(serde_json::Value::as_array)
            .expect("runtime_dispatch_needs array");
        assert_dynamic_context(
            call_context,
            fixture.owner,
            fixture.case.expected_path,
            fixture.case.expected_arg_count,
            fixture.case.label,
            "lookup",
        );
        assert_no_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "lookup");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert_eq!(ui_field(ui, "runtime_dispatch_needs"), "0");
    }
}

#[tokio::test]
async fn code_item_lookup_returns_real_corpus_dynamic_candidate_rows() {
    for case in AmbiguousDynamicToolCase::AXUM_LAYER
        .into_iter()
        .chain(AmbiguousDynamicToolCase::MEMCHR_SELF_FIELD)
    {
        let fixture = AmbiguousDynamicToolFixture::new(case).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(fixture.case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(fixture.case.owner_type)),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("dynamic-candidate-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chains:
        // Source chains are listed on the case matrix in `targetless.rs`.
        // Expected traversal: exact owner lookup exposes candidate-only
        // dynamic ambiguity and does not fabricate a resolved traversal edge.
        let site_id = assert_ambiguous_dynamic_candidates_with_relation(
            call_context,
            fixture.owner,
            Some(fixture.case.expected_path),
            fixture.case.expected_arg_count,
            &fixture.candidates,
            fixture.case.expected_relation.clone(),
            fixture.case.label,
            "lookup",
        );
        assert_ambiguous_candidate_proof(
            proof_context,
            fixture.owner,
            site_id,
            fixture.case.build_domain(),
            &fixture.candidates,
            fixture.case.label,
            "lookup",
        );
        if fixture.case.expects_self_field_binding_evidence() {
            assert_self_field_binding_evidence(
                proof_context,
                fixture.owner,
                site_id,
                fixture.case.build_domain(),
                fixture.case.expected_path,
                "ambiguous",
                fixture.case.label,
                "lookup",
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing dynamic candidate context"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_lookup_returns_route_oneshot_targetless_real_corpus_rows() {
    for case in ReceiverToolCase::ROUTE_ONESHOT {
        let fixture = ReceiverToolFixture::new(case.clone()).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("axum-route-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");
        let summary_needs = payload
            .get("external_summary_needs")
            .and_then(serde_json::Value::as_array)
            .expect("external_summary_needs array");
        let reach_effects = payload
            .get("call_reach_effects")
            .and_then(serde_json::Value::as_array)
            .expect("call_reach_effects array");

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chains:
        //   axum/src/routing/route.rs:51 calls
        //   `self.0.clone().oneshot(req)`.
        //   axum/src/routing/route.rs:57 calls `self.0.oneshot(req)`.
        // Expected traversal: exact owner lookup exposes both structural
        // Route::oneshot receiver rows as external frontiers. The admitted
        // summary discharges the missing-summary need and exposes a
        // proof-derived effect without adding local callee targets.
        let callee = fixture.case.callee();
        let site_id = assert_method_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
            fixture.case.generic_arg_count,
            fixture.case.label,
            "lookup",
        );
        if let Some(summary) = fixture.case.admitted_external_summary() {
            assert_admitted_external_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "lookup",
            );
            assert_no_external_summary_need(summary_needs, site_id, fixture.case.label, "lookup");
            assert_admitted_external_summary_effect(
                reach_effects,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "lookup",
            );
        } else {
            assert_method_proof(
                proof_context,
                fixture.owner,
                site_id,
                &fixture.case.status,
                fixture.case.label,
                "lookup",
            );
            assert_external_summary_need(summary_needs, site_id, fixture.case.label, "lookup");
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing Route::oneshot targetless call context"
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 2,
            "code_item_lookup should surface Route::oneshot targetless proof rows"
        );
        assert_eq!(
            ui_field(ui, "external_summary_needs"),
            summary_needs.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_lookup_returns_size_hint_external_real_corpus_row() {
    for case in ReceiverToolCase::SIZE_HINT {
        let fixture = ReceiverToolFixture::new(case.clone()).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("axum-size-hint-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");
        let summary_needs = payload
            .get("external_summary_needs")
            .and_then(serde_json::Value::as_array)
            .expect("external_summary_needs array");
        let reach_effects = payload
            .get("call_reach_effects")
            .and_then(serde_json::Value::as_array)
            .expect("call_reach_effects array");

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chain:
        //   axum-core/src/body.rs:127 calls `self.0.size_hint()`.
        // Expected traversal: exact owner lookup exposes the structural
        // self-field receiver row as an external frontier. The row remains
        // targetless because external http-body-util dispatch is not traversed.
        let callee = fixture.case.callee();
        let site_id = assert_method_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
            fixture.case.generic_arg_count,
            fixture.case.label,
            "lookup",
        );
        if let Some(summary) = fixture.case.admitted_external_summary() {
            assert_admitted_external_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "lookup",
            );
            assert_no_external_summary_need(summary_needs, site_id, fixture.case.label, "lookup");
            assert_admitted_external_summary_effect(
                reach_effects,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "lookup",
            );
        } else {
            assert_method_proof(
                proof_context,
                fixture.owner,
                site_id,
                &fixture.case.status,
                fixture.case.label,
                "lookup",
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing size_hint external frontier call context"
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 2,
            "code_item_lookup should surface size_hint external frontier proof rows"
        );
    }
}

#[tokio::test]
async fn code_item_lookup_returns_unsupported_receiver_targetless_real_corpus_rows() {
    for case in ReceiverToolCase::REQUEST_PARTS_TURBOFISH
        .into_iter()
        .chain(ReceiverToolCase::FUTURE_POLL)
    {
        let fixture = ReceiverToolFixture::new(case.clone()).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result =
            CodeItemLookup::execute(params.clone(), fixture.ctx("axum-request-parts-lookup"))
                .await
                .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");
        let runtime_needs = payload
            .get("runtime_dispatch_needs")
            .and_then(serde_json::Value::as_array)
            .expect("runtime_dispatch_needs array");
        let future_poll_flows = payload
            .get("future_poll_field_producer_flows")
            .and_then(serde_json::Value::as_array)
            .expect("future_poll_field_producer_flows array");

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chain:
        //   axum-core/src/ext_traits/request_parts.rs:159 binds
        //   `parts` from `Request::new(()).into_parts()`.
        //   axum-core/src/ext_traits/request_parts.rs:164 calls
        //   `parts.extract_with_state::<State<String>, String>(&state)`.
        //   axum/src/error_handling/mod.rs:238 defines `HandleErrorFuture`.
        //   axum/src/error_handling/mod.rs:251 calls
        //   `self.project().future.poll(cx)` through a boxed dyn Future.
        // Expected traversal: exact owner lookup preserves method-generic
        // arguments and precise receiver shapes. The request-parts row now
        // resolves through the exact external-return summary; the dyn Future
        // row remains targetless until async poll/resume or runtime
        // trait-object dispatch proof is available.
        let callee = fixture.case.callee();
        if fixture.case.status == CallStatusKind::Resolved {
            let target = request_parts_extract_target(fixture.state.db.as_ref());
            let site_id = assert_resolved_method_context(
                call_context,
                fixture.owner,
                &callee,
                target,
                fixture.case.generic_arg_count,
                fixture.case.label,
                "lookup",
            );
            assert_resolved_method_proof(
                proof_context,
                fixture.owner,
                site_id,
                target,
                fixture.case.label,
                "lookup",
            );
        } else {
            let site_id = assert_method_context(
                call_context,
                fixture.owner,
                &callee,
                &fixture.case.status,
                fixture.case.generic_arg_count,
                fixture.case.label,
                "lookup",
            );
            assert_method_proof(
                proof_context,
                fixture.owner,
                site_id,
                &fixture.case.status,
                fixture.case.label,
                "lookup",
            );
            if fixture.case.expects_runtime_dispatch_blocker() {
                assert_runtime_dispatch_blocker(
                    proof_context,
                    site_id,
                    fixture.case.label,
                    "lookup",
                );
                assert_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "lookup");
                assert_handle_error_future_poll_producer_flow_payload(
                    future_poll_flows,
                    fixture.owner,
                    "code_item_lookup",
                );
                fixture
                    .state
                    .db
                    .upsert_proof_fact_values(&[
                        ploke_test_utils::axum_dyn_future_poll_runtime_dispatch_summary(site_id),
                    ])
                    .unwrap_or_else(|err| {
                        panic!(
                            "{} runtime dispatch summary insert: {err}",
                            fixture.case.label
                        )
                    });

                let after = CodeItemLookup::execute(
                    params,
                    fixture.ctx("axum-future-poll-summary-after-lookup"),
                )
                .await
                .unwrap_or_else(|err| {
                    panic!("{} summary code_item_lookup: {err}", fixture.case.label)
                });
                let payload: serde_json::Value =
                    serde_json::from_str(&after.content).expect("deserialize ConciseContext");
                let call_context = payload
                    .get("call_context")
                    .and_then(serde_json::Value::as_array)
                    .expect("call_context array");
                let runtime_needs = payload
                    .get("runtime_dispatch_needs")
                    .and_then(serde_json::Value::as_array)
                    .expect("runtime_dispatch_needs array");
                assert_method_context(
                    call_context,
                    fixture.owner,
                    &callee,
                    &fixture.case.status,
                    fixture.case.generic_arg_count,
                    fixture.case.label,
                    "lookup",
                );
                assert_no_runtime_dispatch_need(
                    runtime_needs,
                    site_id,
                    fixture.case.label,
                    "lookup",
                );
                let ui = after.ui_payload.as_ref().expect("ui payload");
                assert_eq!(ui_field(ui, "runtime_dispatch_needs"), "0");
            }
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing unsupported receiver targetless call context"
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 2,
            "code_item_lookup should surface unsupported receiver proof rows"
        );
        assert_eq!(
            ui_field(ui, "runtime_dispatch_needs"),
            runtime_needs.len().to_string()
        );
        assert_eq!(
            ui_field(ui, "future_poll_field_producer_flows"),
            future_poll_flows.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_lookup_returns_from_ref_dependency_root_path_rows() {
    for case in PathToolCase::FROM_REF_DEP_ROOT {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("axum-from-ref-path-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let _proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chain:
        //   axum/src/middleware/from_extractor.rs:328 calls
        //   `Secret::from_ref(state)`.
        // Expected traversal: exact item lookup can resolve the enclosing
        // `test_from_extractor` function, but it must not flatten the nested
        // local impl method body row into that parent function. DB/RAG tests
        // pin the resolved `local_impl_method:from_request_parts` owner;
        // exact item tools do not currently accept executable body owners as
        // `node_kind` values. The parent item can still carry non-call proof
        // rows, so this assertion is scoped to call-context non-flattening.
        let callee = fixture.case.callee();
        assert_path_context_absent(
            call_context,
            fixture.owner,
            &callee,
            fixture.case.label,
            "lookup",
        );

        let _ui = result.ui_payload.as_ref().expect("ui payload");
    }
}

#[tokio::test]
async fn code_item_lookup_preserves_shadowed_get_resolved_setup_boundary() {
    for case in PathToolCase::SHADOWED_GET {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("axum-shadowed-get-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chain:
        //   axum/src/routing/tests/mod.rs:412-413 calls imported routing `get`.
        //   axum/src/routing/tests/mod.rs:418 binds a local closure named `get`.
        //   axum/src/routing/tests/mod.rs:423-434 calls that closure inside
        //   `assert_eq!` macro arguments.
        // Expected traversal: exact owner lookup exposes only the two setup
        // `get(...)` path rows, resolves those setup helper calls, and does
        // not fabricate edges from the later shadowed macro-argument calls to
        // routing `get`.
        let callee = fixture.case.callee();
        assert_resolved_path_context_count(
            call_context,
            fixture.owner,
            &callee,
            CallTargetKind::Function,
            2,
            fixture.case.label,
            "lookup",
        );
        assert_macro_blocker_count(
            proof_context,
            fixture.owner,
            "macro_expansion_not_available",
            11,
            fixture.case.label,
            "lookup",
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 2,
            "code_item_lookup should surface both shadowed get setup rows"
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
    }
}

#[tokio::test]
async fn code_item_lookup_returns_request_builder_alias_external_path_rows() {
    for case in PathToolCase::REQUEST_BUILDER_ALIAS
        .into_iter()
        .chain(PathToolCase::STD_MEM_REPLACE)
    {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("axum-request-builder-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");
        let summary_needs = payload
            .get("external_summary_needs")
            .and_then(serde_json::Value::as_array)
            .expect("external_summary_needs array");
        let reach_effects = payload
            .get("call_reach_effects")
            .and_then(serde_json::Value::as_array)
            .expect("call_reach_effects array");

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chains:
        //   axum/src/middleware/from_fn.rs:411 calls
        //   `Request::builder().uri("/").body(Body::empty()).unwrap()`.
        //   axum/src/response/sse.rs:449 calls
        //   `std::mem::replace(&mut self.data_written, true)`.
        // Expected traversal: exact item lookup exposes both path rows as
        // external frontiers. The rows remain targetless because they leave
        // the selected local workspace.
        let callee = fixture.case.callee();
        let site_id = assert_path_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
            fixture.case.label,
            "lookup",
        );
        if let Some(summary) = fixture.case.admitted_external_summary() {
            assert_admitted_external_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "lookup",
            );
            assert_no_external_summary_need(summary_needs, site_id, fixture.case.label, "lookup");
            assert_admitted_external_summary_effect(
                reach_effects,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "lookup",
            );
        } else {
            assert_path_blocker_proof(
                proof_context,
                fixture.owner,
                site_id,
                "bd:corpus-axum-call-graph",
                "external_dependency_summary_missing",
                fixture.case.label,
                "lookup",
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing external frontier call context for {}",
            fixture.case.label
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 2,
            "code_item_lookup should surface external frontier proof rows for {}",
            fixture.case.label
        );
        assert_eq!(
            ui_field(ui, "external_summary_needs"),
            summary_needs.len().to_string()
        );
        assert_eq!(
            ui_field(ui, "reach_effects"),
            reach_effects.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_lookup_returns_memchr_callable_trait_object_path_rows() {
    for case in PathToolCase::MEMCHR_CALLABLE_TRAIT_OBJECT {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("memchr-callable-path-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chains:
        //   memchr/src/tests/substring/mod.rs:73-77 defines `Runner` with
        //   `fwd` and `rev` boxed `dyn FnMut` fields.
        //   memchr/src/tests/substring/mod.rs:94 calls
        //   `fwd(t.haystack.as_bytes(), t.needle.as_bytes())`.
        //   memchr/src/tests/substring/mod.rs:110 calls
        //   `rev(t.haystack.as_bytes(), t.needle.as_bytes())`.
        // Expected traversal: exact owner lookup exposes both structural path
        // rows as unsupported targetless callsites. The rows remain targetless
        // because local callable binding and trait-object dispatch proof are
        // not yet modeled.
        let callee = fixture.case.callee();
        let site_id = assert_path_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
            fixture.case.label,
            "lookup",
        );
        assert_path_blocker_proof(
            proof_context,
            fixture.owner,
            site_id,
            fixture.case.build_domain(),
            "type_resolution_missing",
            fixture.case.label,
            "lookup",
        );
        assert_runtime_dispatch_blocker(proof_context, site_id, fixture.case.label, "lookup");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing callable trait-object path context for {}",
            fixture.case.label
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 2,
            "code_item_lookup should surface callable trait-object proof rows for {}",
            fixture.case.label
        );
    }
}

#[tokio::test]
async fn code_item_lookup_returns_memchr_ifunc_generated_transmute_frontiers() {
    for case in IfuncToolCase::MEMCHR {
        let fixture = IfuncToolFixture::new(case).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("memchr-ifunc-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");
        let unsafe_calls = payload
            .get("unsafe_block_calls")
            .and_then(serde_json::Value::as_array)
            .expect("unsafe_block_calls array");

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chain:
        //   memchr/src/arch/x86_64/memchr.rs:153 generates
        //   `core::mem::transmute::<Fn, RealFn>(fun)(...)`.
        //   The `unsafe_ifunc!` instantiations at :180, :203, :227, :252,
        //   :278, :305, and :326 expand that arbitrary-expression callee.
        // Expected traversal: exact owner lookup exposes both generated
        // frontiers, the inner external path row and the outer external
        // returned-path dynamic row, without fabricating a function-pointer
        // target or local traversal edge.
        let sites = assert_ifunc_context(
            call_context,
            fixture.owner,
            fixture.case.expected_arg_count,
            fixture.case.label,
            "lookup",
        );
        assert_ifunc_proof(
            proof_context,
            fixture.owner,
            sites,
            fixture.case.build_domain(),
            fixture.case.label,
            "lookup",
        );
        assert_ifunc_unsafe_calls(
            unsafe_calls,
            fixture.owner,
            sites,
            fixture.case.expected_arg_count,
            fixture.case.label,
            "lookup",
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 2,
            "code_item_lookup should surface both generated ifunc frontier rows for {}",
            fixture.case.label
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 4,
            "code_item_lookup should surface generated ifunc proof rows for {}",
            fixture.case.label
        );
        assert_eq!(ui_field(ui, "unsafe_block_calls"), "2");
    }
}

#[tokio::test]
async fn code_item_lookup_omits_admitted_memchr_runtime_dispatch_summary_needs() {
    for case in PathToolCase::MEMCHR_CALLABLE_TRAIT_OBJECT {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let initial = CodeItemLookup::execute(
            params.clone(),
            fixture.ctx("memchr-callable-summary-before"),
        )
        .await
        .unwrap_or_else(|err| panic!("{} initial code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&initial.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let runtime_needs = payload
            .get("runtime_dispatch_needs")
            .and_then(serde_json::Value::as_array)
            .expect("runtime_dispatch_needs array");

        // Same memchr boxed dyn FnMut source oracle as the path-row lookup
        // test above. Admitting a runtime-dispatch summary should remove the
        // exact authoring need without creating a local traversal target.
        let site_id = assert_path_context(
            call_context,
            fixture.owner,
            &fixture.case.callee(),
            &fixture.case.status,
            fixture.case.label,
            "lookup",
        );
        assert_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "lookup");

        fixture
            .state
            .db
            .upsert_proof_fact_values(&[
                ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_summary(site_id),
            ])
            .unwrap_or_else(|err| {
                panic!(
                    "{} runtime dispatch summary insert: {err}",
                    fixture.case.label
                )
            });

        let result = CodeItemLookup::execute(params, fixture.ctx("memchr-callable-summary-after"))
            .await
            .unwrap_or_else(|err| panic!("{} summary code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let runtime_needs = payload
            .get("runtime_dispatch_needs")
            .and_then(serde_json::Value::as_array)
            .expect("runtime_dispatch_needs array");
        assert_path_context(
            call_context,
            fixture.owner,
            &fixture.case.callee(),
            &fixture.case.status,
            fixture.case.label,
            "lookup",
        );
        assert_no_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "lookup");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert_eq!(ui_field(ui, "runtime_dispatch_needs"), "0");
    }
}

#[tokio::test]
async fn code_item_lookup_returns_generated_macro_boundary_path_rows() {
    for case in PathToolCase::INTO_SERVICE_FUTURE_NEW
        .into_iter()
        .chain(PathToolCase::ROUTING_POST)
        .chain(PathToolCase::ROUTING_GET_SERVICE)
    {
        let fixture = PathToolFixture::new(case.clone()).await;
        let boundary = fixture
            .case
            .admitted_macro_boundary_summary()
            .expect("generated-boundary fixture");
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result =
            CodeItemLookup::execute(params, fixture.ctx("axum-into-service-future-new-lookup"))
                .await
                .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let proof_context = payload
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("proof_context array");
        let reach = payload
            .get("call_reach")
            .and_then(serde_json::Value::as_object)
            .expect("call_reach object");

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chains:
        //   axum/src/handler/service.rs:155 binds
        //   `type Future = super::future::IntoServiceFuture<H::Future>`.
        //   axum/src/handler/service.rs:174 calls
        //   `super::future::IntoServiceFuture::new(future)`.
        //   axum/src/handler/future.rs:11-18 and axum/src/macros.rs:19-20
        //   generate the concrete inherent constructor.
        //   axum/src/json.rs:248 calls generated `routing::post(...)`.
        //   axum/src/routing/tests/get_to_head.rs:46 calls generated
        //   `routing::get_service(...)`.
        // Expected traversal: the bounded `opaque_future!` expansion resolves
        // the generated constructor edge, while bounded routing macro
        // expansions resolve representative generated handler/service helpers.
        let callee = fixture.case.callee();
        let ui = result.ui_payload.as_ref().expect("ui payload");
        if fixture.case.status == CallStatusKind::Resolved {
            let (site_id, target) = assert_resolved_path_context_target(
                call_context,
                fixture.owner,
                &callee,
                fixture.case.expected_resolved_relation(),
                fixture.case.label,
                "lookup",
            );
            assert_resolved_path_proof(
                proof_context,
                fixture.owner,
                site_id,
                target,
                fixture.case.label,
                "lookup",
            );
            assert_admitted_macro_boundary_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
                boundary,
                fixture.case.label,
                "lookup",
            );
            let direct_count = assert_resolved_direct_call_site_reach(
                reach,
                fixture.owner,
                &fixture.case,
                target,
                fixture.case.expected_resolved_relation(),
                "lookup",
            );
            assert_eq!(
                ui_field(ui, "reach_direct_call_sites"),
                direct_count.to_string()
            );
        } else {
            let site_id = assert_path_context(
                call_context,
                fixture.owner,
                &callee,
                &fixture.case.status,
                fixture.case.label,
                "lookup",
            );
            assert_path_resolution_proof(
                proof_context,
                fixture.owner,
                site_id,
                "bd:corpus-axum-call-graph",
                boundary.expected_state,
                boundary
                    .expected_blocker
                    .expect("blocked macro boundary should carry a blocker"),
                fixture.case.label,
                "lookup",
            );
            assert_admitted_macro_boundary_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
                boundary,
                fixture.case.label,
                "lookup",
            );
            let (frontier_ui_field, frontier_count, ambiguous_count) =
                assert_status_frontier_reach(reach, fixture.owner, &fixture.case, "lookup");
            assert_eq!(
                ui_field(ui, frontier_ui_field.as_str()),
                frontier_count.to_string()
            );
            assert_eq!(
                ui_field(ui, "reach_ambiguous_frontier_calls"),
                ambiguous_count.to_string()
            );
        }
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing generated-boundary call context"
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 2,
            "code_item_lookup should surface generated-boundary proof rows"
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_dynamic_targetless_real_corpus_rows() {
    for case in DynamicToolCase::AXUM {
        let fixture = DynamicToolFixture::new(case).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("dynamic-targetless-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");
        let runtime_needs = payload
            .get("node_info")
            .and_then(|node| node.get("runtime_dispatch_needs"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.runtime_dispatch_needs array");
        // Same real-corpus dynamic targetless oracle as the lookup test above,
        // exercised through the edge-oriented exact tool payload.
        let site_id = assert_dynamic_context(
            call_context,
            fixture.owner,
            fixture.case.expected_path,
            fixture.case.expected_arg_count,
            fixture.case.label,
            "edges",
        );
        assert_dynamic_proof(
            proof_context,
            fixture.owner,
            site_id,
            fixture.case.build_domain(),
            fixture.case.label,
            "edges",
        );
        if let Some(expected_path) = fixture.case.expected_path {
            assert_self_field_binding_evidence(
                proof_context,
                fixture.owner,
                site_id,
                fixture.case.build_domain(),
                expected_path,
                "blocked",
                fixture.case.label,
                "edges",
            );
        }
        if fixture.case.expects_runtime_dispatch_blocker() {
            assert_runtime_dispatch_blocker(proof_context, site_id, fixture.case.label, "edges");
            assert_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "edges");
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing dynamic targetless call context"
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
    }
}

#[tokio::test]
async fn code_item_edges_returns_real_corpus_dynamic_candidate_rows() {
    for case in AmbiguousDynamicToolCase::AXUM_LAYER
        .into_iter()
        .chain(AmbiguousDynamicToolCase::MEMCHR_SELF_FIELD)
    {
        let fixture = AmbiguousDynamicToolFixture::new(case).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(fixture.case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(fixture.case.owner_type)),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("dynamic-candidate-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        // Same real-corpus dynamic candidate oracle as the lookup test above,
        // exercised through the edge-oriented exact tool payload.
        let site_id = assert_ambiguous_dynamic_candidates_with_relation(
            call_context,
            fixture.owner,
            Some(fixture.case.expected_path),
            fixture.case.expected_arg_count,
            &fixture.candidates,
            fixture.case.expected_relation.clone(),
            fixture.case.label,
            "edges",
        );
        assert_ambiguous_candidate_proof(
            proof_context,
            fixture.owner,
            site_id,
            fixture.case.build_domain(),
            &fixture.candidates,
            fixture.case.label,
            "edges",
        );
        if fixture.case.expects_self_field_binding_evidence() {
            assert_self_field_binding_evidence(
                proof_context,
                fixture.owner,
                site_id,
                fixture.case.build_domain(),
                fixture.case.expected_path,
                "ambiguous",
                fixture.case.label,
                "edges",
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing dynamic candidate context"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_size_hint_external_real_corpus_row() {
    for case in ReceiverToolCase::SIZE_HINT {
        let fixture = ReceiverToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("axum-size-hint-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");
        let summary_needs = payload
            .get("node_info")
            .and_then(|node| node.get("external_summary_needs"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.external_summary_needs array");
        let reach_effects = payload
            .get("node_info")
            .and_then(|node| node.get("call_reach_effects"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_reach_effects array");

        // Same real-corpus size_hint external-frontier oracle as the lookup test
        // above, exercised through the edge-oriented exact tool payload.
        let callee = fixture.case.callee();
        let site_id = assert_method_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
            fixture.case.generic_arg_count,
            fixture.case.label,
            "edges",
        );
        if let Some(summary) = fixture.case.admitted_external_summary() {
            assert_admitted_external_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "edges",
            );
            assert_no_external_summary_need(summary_needs, site_id, fixture.case.label, "edges");
            assert_admitted_external_summary_effect(
                reach_effects,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "edges",
            );
        } else {
            assert_method_proof(
                proof_context,
                fixture.owner,
                site_id,
                &fixture.case.status,
                fixture.case.label,
                "edges",
            );
            assert_external_summary_need(summary_needs, site_id, fixture.case.label, "edges");
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing size_hint external frontier call context"
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
        assert_eq!(
            ui_field(ui, "external_summary_needs"),
            summary_needs.len().to_string()
        );
    }
}

fn assert_external_summary_need(
    needs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site = site_id.to_string();
    let need = needs
        .iter()
        .find(|need| {
            need.get("call_site")
                .and_then(|call| call.get("site_id"))
                .and_then(serde_json::Value::as_str)
                == Some(site.as_str())
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose an external summary need for {label}: {needs:#?}")
        });
    let reasons = need
        .get("blocker_reasons")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} external summary need reasons missing: {need:#?}"));
    assert!(
        reasons
            .iter()
            .any(|reason| reason.as_str() == Some("external_dependency_summary_missing")),
        "{tool} external summary need should preserve missing-summary blocker for {label}: {need:#?}"
    );
    let paths = need
        .get("paths_to_owner")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} external summary need paths missing: {need:#?}"));
    assert!(
        paths.is_empty(),
        "{tool} direct frontier need should not include intermediate owner paths for {label}: {need:#?}"
    );
}

fn assert_no_external_summary_need(
    needs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site = site_id.to_string();
    assert!(
        needs.iter().all(|need| {
            need.get("call_site")
                .and_then(|call| call.get("site_id"))
                .and_then(serde_json::Value::as_str)
                != Some(site.as_str())
        }),
        "{tool} should not expose a remaining external summary need for admitted {label}: {needs:#?}"
    );
}

fn assert_runtime_dispatch_need(
    needs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site = site_id.to_string();
    let need = needs
        .iter()
        .find(|need| {
            need.get("call_site")
                .and_then(|call| call.get("site_id"))
                .and_then(serde_json::Value::as_str)
                == Some(site.as_str())
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose a runtime-dispatch need for {label}: {needs:#?}")
        });
    let blockers = need
        .get("blocker_reasons")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} runtime_dispatch_needs.blocker_reasons array"));
    assert!(
        blockers
            .iter()
            .any(|reason| reason.as_str() == Some("dynamic_dispatch_unbounded")),
        "{tool} runtime-dispatch need should preserve dynamic_dispatch_unbounded for {label}: {need:#?}"
    );
    let paths = need
        .get("paths_to_owner")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} runtime_dispatch_needs.paths_to_owner array"));
    assert!(
        paths.is_empty(),
        "{tool} direct runtime-dispatch need should not include intermediate owner paths for {label}: {need:#?}"
    );
}

fn assert_no_runtime_dispatch_need(
    needs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site = site_id.to_string();
    assert!(
        needs.iter().all(|need| {
            need.get("call_site")
                .and_then(|call| call.get("site_id"))
                .and_then(serde_json::Value::as_str)
                != Some(site.as_str())
        }),
        "{tool} should not expose a remaining runtime-dispatch need for admitted {label}: {needs:#?}"
    );
}

#[tokio::test]
async fn code_item_edges_returns_unsupported_receiver_targetless_real_corpus_rows() {
    for case in ReceiverToolCase::REQUEST_PARTS_TURBOFISH
        .into_iter()
        .chain(ReceiverToolCase::FUTURE_POLL)
    {
        let fixture = ReceiverToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result =
            CodeItemEdges::execute(params.clone(), fixture.ctx("axum-request-parts-edges"))
                .await
                .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");
        let runtime_needs = payload
            .get("node_info")
            .and_then(|node| node.get("runtime_dispatch_needs"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.runtime_dispatch_needs array");
        let future_poll_flows = payload
            .get("node_info")
            .and_then(|node| node.get("future_poll_field_producer_flows"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.future_poll_field_producer_flows array");

        // Same receiver source oracles as the lookup test above, exercised
        // through the edge-oriented payload.
        let callee = fixture.case.callee();
        if fixture.case.status == CallStatusKind::Resolved {
            let target = request_parts_extract_target(fixture.state.db.as_ref());
            let site_id = assert_resolved_method_context(
                call_context,
                fixture.owner,
                &callee,
                target,
                fixture.case.generic_arg_count,
                fixture.case.label,
                "edges",
            );
            assert_resolved_method_proof(
                proof_context,
                fixture.owner,
                site_id,
                target,
                fixture.case.label,
                "edges",
            );
        } else {
            let site_id = assert_method_context(
                call_context,
                fixture.owner,
                &callee,
                &fixture.case.status,
                fixture.case.generic_arg_count,
                fixture.case.label,
                "edges",
            );
            assert_method_proof(
                proof_context,
                fixture.owner,
                site_id,
                &fixture.case.status,
                fixture.case.label,
                "edges",
            );
            if fixture.case.expects_runtime_dispatch_blocker() {
                assert_runtime_dispatch_blocker(
                    proof_context,
                    site_id,
                    fixture.case.label,
                    "edges",
                );
                assert_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "edges");
                assert_handle_error_future_poll_producer_flow_payload(
                    future_poll_flows,
                    fixture.owner,
                    "code_item_edges",
                );
                fixture
                    .state
                    .db
                    .upsert_proof_fact_values(&[
                        ploke_test_utils::axum_dyn_future_poll_runtime_dispatch_summary(site_id),
                    ])
                    .unwrap_or_else(|err| {
                        panic!(
                            "{} runtime dispatch summary insert: {err}",
                            fixture.case.label
                        )
                    });

                let after = CodeItemEdges::execute(
                    params,
                    fixture.ctx("axum-future-poll-summary-after-edges"),
                )
                .await
                .unwrap_or_else(|err| {
                    panic!("{} summary code_item_edges: {err}", fixture.case.label)
                });
                let payload: serde_json::Value =
                    serde_json::from_str(&after.content).expect("deserialize NodeEdgeInfo");
                let call_context = payload
                    .get("node_info")
                    .and_then(|node| node.get("call_context"))
                    .and_then(serde_json::Value::as_array)
                    .expect("node_info.call_context array");
                let runtime_needs = payload
                    .get("node_info")
                    .and_then(|node| node.get("runtime_dispatch_needs"))
                    .and_then(serde_json::Value::as_array)
                    .expect("node_info.runtime_dispatch_needs array");
                assert_method_context(
                    call_context,
                    fixture.owner,
                    &callee,
                    &fixture.case.status,
                    fixture.case.generic_arg_count,
                    fixture.case.label,
                    "edges",
                );
                assert_no_runtime_dispatch_need(
                    runtime_needs,
                    site_id,
                    fixture.case.label,
                    "edges",
                );
                let ui = after.ui_payload.as_ref().expect("ui payload");
                assert_eq!(ui_field(ui, "runtime_dispatch_needs"), "0");
            }
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing unsupported receiver targetless call context"
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
        assert_eq!(
            ui_field(ui, "runtime_dispatch_needs"),
            runtime_needs.len().to_string()
        );
        assert_eq!(
            ui_field(ui, "future_poll_field_producer_flows"),
            future_poll_flows.len().to_string()
        );
    }
}

fn assert_status_frontier_reach(
    reach: &serde_json::Map<String, serde_json::Value>,
    owner: uuid::Uuid,
    case: &PathToolCase,
    tool: &str,
) -> (String, usize, usize) {
    let bucket = match &case.status {
        CallStatusKind::Unsupported => "unsupported_frontier_calls",
        CallStatusKind::Unresolved => "unresolved_frontier_calls",
        CallStatusKind::Ambiguous => "ambiguous_frontier_calls",
        other => panic!(
            "{tool} generated-boundary reach helper only accepts frontier statuses, got {other:?}"
        ),
    };
    let frontier = reach
        .get(bucket)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} call_reach {bucket} array"));
    let calls = frontier
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("{tool} {bucket} rows should deserialize: {err}"));
    let call = calls
        .iter()
        .find(|call| {
            call.owner_id == owner
                && call.status == case.status
                && call.targets.is_empty()
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path }
                        if path.iter().map(String::as_str).eq(case.path.iter().copied())
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "{tool} should expose {} in {bucket}: {calls:#?}",
                case.label,
            )
        });
    assert_eq!(call.owner_id, owner);

    let ambiguous = reach
        .get("ambiguous_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} call_reach ambiguous_frontier_calls array"));
    assert!(
        ambiguous.is_empty(),
        "{tool} should not report ambiguous frontier rows for {}: {ambiguous:#?}",
        case.label
    );
    (format!("reach_{bucket}"), calls.len(), ambiguous.len())
}

fn assert_resolved_direct_call_site_reach(
    reach: &serde_json::Map<String, serde_json::Value>,
    owner: uuid::Uuid,
    case: &PathToolCase,
    target: uuid::Uuid,
    relation: CallTargetKind,
    tool: &str,
) -> usize {
    let direct = reach
        .get("direct_call_sites")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} call_reach direct_call_sites array"));
    let calls = direct
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("{tool} direct_call_sites rows should deserialize: {err}"));
    assert!(
        calls.iter().any(|call| {
            call.owner_id == owner
                && call.status == CallStatusKind::Resolved
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path }
                        if path.iter().map(String::as_str).eq(case.path.iter().copied())
                )
                && call.targets.len() == 1
                && call.targets[0].target_id == target
                && call.targets[0].relation == relation
        }),
        "{tool} should expose {} as a resolved direct callsite: {calls:#?}",
        case.label
    );
    let unresolved = reach
        .get("unresolved_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} call_reach unresolved_frontier_calls array"));
    assert!(
        unresolved.iter().all(|call| {
            serde_json::from_value::<CallContextInfo>(call.clone()).map_or(true, |call| {
                !(call.owner_id == owner
                    && matches!(
                        &call.callee,
                        CallCalleeInfo::Path { path }
                            if path.iter().map(String::as_str).eq(case.path.iter().copied())
                    ))
            })
        }),
        "{tool} should not keep resolved {} in unresolved frontier: {unresolved:#?}",
        case.label
    );
    calls.len()
}

#[tokio::test]
async fn code_item_edges_returns_from_ref_dependency_root_path_rows() {
    for case in PathToolCase::FROM_REF_DEP_ROOT {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("axum-from-ref-path-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let _proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        // Same parent-function non-flattening oracle as the lookup test above,
        // exercised through the edge-oriented payload.
        let callee = fixture.case.callee();
        assert_path_context_absent(
            call_context,
            fixture.owner,
            &callee,
            fixture.case.label,
            "edges",
        );

        let _ui = result.ui_payload.as_ref().expect("ui payload");
    }
}

#[tokio::test]
async fn code_item_edges_preserves_shadowed_get_resolved_setup_boundary() {
    for case in PathToolCase::SHADOWED_GET {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("axum-shadowed-get-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        // Same real-corpus shadowed get oracle as the lookup test above,
        // exercised through the edge-oriented exact tool payload.
        let callee = fixture.case.callee();
        assert_resolved_path_context_count(
            call_context,
            fixture.owner,
            &callee,
            CallTargetKind::Function,
            2,
            fixture.case.label,
            "edges",
        );
        assert_macro_blocker_count(
            proof_context,
            fixture.owner,
            "macro_expansion_not_available",
            11,
            fixture.case.label,
            "edges",
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 2,
            "code_item_edges should surface both shadowed get setup rows"
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
    }
}

fn assert_macro_blocker_count(
    proofs: &[serde_json::Value],
    owner: Uuid,
    reason: &str,
    expected: usize,
    label: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| {
            serde_json::from_value::<ploke_core::rag_types::ProofContextInfo>(proof.clone()).ok()
        })
        .collect::<Vec<_>>();
    let owner_site_ids = rows
        .iter()
        .filter(|proof| {
            proof.kind == "call_site" && proof.caller_def_id.as_deref() == Some(owner.as_str())
        })
        .filter_map(|proof| proof.call_site_id.as_deref())
        .collect::<Vec<_>>();
    let matching = rows
        .iter()
        .filter(|proof| {
            proof.kind == "call_resolution"
                && proof.resolution_state.as_deref() == Some("blocked")
                && proof.blocker_reason.as_deref() == Some(reason)
                && proof
                    .call_site_id
                    .as_deref()
                    .is_some_and(|site| owner_site_ids.contains(&site))
        })
        .collect::<Vec<_>>();
    assert!(
        matching.len() >= expected,
        "{tool} should expose at least {expected} {reason} macro blocker rows for {label}: {rows:#?}"
    );
    for proof in matching {
        let site = proof
            .call_site_id
            .as_deref()
            .unwrap_or_else(|| panic!("{tool} {reason} proof row should carry call_site_id"));
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_site"
                    && row.caller_def_id.as_deref() == Some(owner.as_str())
                    && row.call_site_id.as_deref() == Some(site)
            }),
            "{tool} should expose the call_site row for {reason} blocker {site} in {label}: {rows:#?}"
        );
        assert!(
            rows.iter().all(|row| {
                row.kind != "call_edge" || row.call_site_id.as_deref() != Some(site)
            }),
            "{tool} must not fabricate call_edge proof rows for {reason} blocker {site} in {label}: {rows:#?}"
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_request_builder_alias_external_path_rows() {
    for case in PathToolCase::REQUEST_BUILDER_ALIAS
        .into_iter()
        .chain(PathToolCase::STD_MEM_REPLACE)
    {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("axum-request-builder-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");
        let summary_needs = payload
            .get("node_info")
            .and_then(|node| node.get("external_summary_needs"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.external_summary_needs array");
        let reach_effects = payload
            .get("node_info")
            .and_then(|node| node.get("call_reach_effects"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_reach_effects array");

        // Same real-corpus external-frontier oracles as the lookup test above,
        // exercised through the edge-oriented payload.
        let callee = fixture.case.callee();
        let site_id = assert_path_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
            fixture.case.label,
            "edges",
        );
        if let Some(summary) = fixture.case.admitted_external_summary() {
            assert_admitted_external_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "edges",
            );
            assert_no_external_summary_need(summary_needs, site_id, fixture.case.label, "edges");
            assert_admitted_external_summary_effect(
                reach_effects,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "edges",
            );
        } else {
            assert_path_blocker_proof(
                proof_context,
                fixture.owner,
                site_id,
                "bd:corpus-axum-call-graph",
                "external_dependency_summary_missing",
                fixture.case.label,
                "edges",
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing external frontier call context for {}",
            fixture.case.label
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
        assert_eq!(
            ui_field(ui, "external_summary_needs"),
            summary_needs.len().to_string()
        );
        assert_eq!(
            ui_field(ui, "reach_effects"),
            reach_effects.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_memchr_callable_trait_object_path_rows() {
    for case in PathToolCase::MEMCHR_CALLABLE_TRAIT_OBJECT {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("memchr-callable-path-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        // Same real-corpus callable trait-object path oracle as the lookup
        // test above, exercised through the edge-oriented exact tool payload.
        let callee = fixture.case.callee();
        let site_id = assert_path_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
            fixture.case.label,
            "edges",
        );
        assert_path_blocker_proof(
            proof_context,
            fixture.owner,
            site_id,
            fixture.case.build_domain(),
            "type_resolution_missing",
            fixture.case.label,
            "edges",
        );
        assert_runtime_dispatch_blocker(proof_context, site_id, fixture.case.label, "edges");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing callable trait-object path context for {}",
            fixture.case.label
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
    }
}

#[tokio::test]
async fn code_item_edges_returns_memchr_ifunc_generated_transmute_frontiers() {
    for case in IfuncToolCase::MEMCHR {
        let fixture = IfuncToolFixture::new(case).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("memchr-ifunc-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");
        let unsafe_calls = payload
            .get("node_info")
            .and_then(|node| node.get("unsafe_block_calls"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.unsafe_block_calls array");

        // Same generated `unsafe_ifunc!` source oracle as the lookup test
        // above, exercised through the edge-oriented exact tool payload.
        let sites = assert_ifunc_context(
            call_context,
            fixture.owner,
            fixture.case.expected_arg_count,
            fixture.case.label,
            "edges",
        );
        assert_ifunc_proof(
            proof_context,
            fixture.owner,
            sites,
            fixture.case.build_domain(),
            fixture.case.label,
            "edges",
        );
        assert_ifunc_unsafe_calls(
            unsafe_calls,
            fixture.owner,
            sites,
            fixture.case.expected_arg_count,
            fixture.case.label,
            "edges",
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 2,
            "code_item_edges should surface both generated ifunc frontier rows for {}",
            fixture.case.label
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
        assert_eq!(ui_field(ui, "unsafe_block_calls"), "2");
    }
}

#[tokio::test]
async fn code_item_edges_omits_admitted_memchr_runtime_dispatch_summary_needs() {
    for case in PathToolCase::MEMCHR_CALLABLE_TRAIT_OBJECT {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let initial = CodeItemEdges::execute(
            params.clone(),
            fixture.ctx("memchr-callable-summary-before-edges"),
        )
        .await
        .unwrap_or_else(|err| panic!("{} initial code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&initial.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let runtime_needs = payload
            .get("node_info")
            .and_then(|node| node.get("runtime_dispatch_needs"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.runtime_dispatch_needs array");

        // Same memchr boxed dyn FnMut source oracle as the lookup summary
        // test above, exercised through the edge-oriented exact tool payload.
        let site_id = assert_path_context(
            call_context,
            fixture.owner,
            &fixture.case.callee(),
            &fixture.case.status,
            fixture.case.label,
            "edges",
        );
        assert_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "edges");

        fixture
            .state
            .db
            .upsert_proof_fact_values(&[
                ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_summary(site_id),
            ])
            .unwrap_or_else(|err| {
                panic!(
                    "{} runtime dispatch summary insert: {err}",
                    fixture.case.label
                )
            });

        let result =
            CodeItemEdges::execute(params, fixture.ctx("memchr-callable-summary-after-edges"))
                .await
                .unwrap_or_else(|err| {
                    panic!("{} summary code_item_edges: {err}", fixture.case.label)
                });
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let runtime_needs = payload
            .get("node_info")
            .and_then(|node| node.get("runtime_dispatch_needs"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.runtime_dispatch_needs array");
        assert_path_context(
            call_context,
            fixture.owner,
            &fixture.case.callee(),
            &fixture.case.status,
            fixture.case.label,
            "edges",
        );
        assert_no_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "edges");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert_eq!(ui_field(ui, "runtime_dispatch_needs"), "0");
    }
}

#[tokio::test]
async fn code_item_edges_returns_generated_macro_boundary_path_rows() {
    for case in PathToolCase::INTO_SERVICE_FUTURE_NEW
        .into_iter()
        .chain(PathToolCase::ROUTING_POST)
        .chain(PathToolCase::ROUTING_GET_SERVICE)
    {
        let fixture = PathToolFixture::new(case.clone()).await;
        let boundary = fixture
            .case
            .admitted_macro_boundary_summary()
            .expect("generated-boundary fixture");
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result =
            CodeItemEdges::execute(params, fixture.ctx("axum-into-service-future-new-edges"))
                .await
                .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");
        let reach = payload
            .get("node_info")
            .and_then(|node| node.get("call_reach"))
            .and_then(serde_json::Value::as_object)
            .expect("node_info.call_reach object");

        // Same generated-boundary oracle as the lookup test above,
        // exercised through the edge-oriented payload.
        let callee = fixture.case.callee();
        let ui = result.ui_payload.as_ref().expect("ui payload");
        if fixture.case.status == CallStatusKind::Resolved {
            let (site_id, target) = assert_resolved_path_context_target(
                call_context,
                fixture.owner,
                &callee,
                fixture.case.expected_resolved_relation(),
                fixture.case.label,
                "edges",
            );
            assert_resolved_path_proof(
                proof_context,
                fixture.owner,
                site_id,
                target,
                fixture.case.label,
                "edges",
            );
            assert_admitted_macro_boundary_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
                boundary,
                fixture.case.label,
                "edges",
            );
            let direct_count = assert_resolved_direct_call_site_reach(
                reach,
                fixture.owner,
                &fixture.case,
                target,
                fixture.case.expected_resolved_relation(),
                "edges",
            );
            assert_eq!(
                ui_field(ui, "reach_direct_call_sites"),
                direct_count.to_string()
            );
        } else {
            let site_id = assert_path_context(
                call_context,
                fixture.owner,
                &callee,
                &fixture.case.status,
                fixture.case.label,
                "edges",
            );
            assert_path_resolution_proof(
                proof_context,
                fixture.owner,
                site_id,
                "bd:corpus-axum-call-graph",
                boundary.expected_state,
                boundary
                    .expected_blocker
                    .expect("blocked macro boundary should carry a blocker"),
                fixture.case.label,
                "edges",
            );
            assert_admitted_macro_boundary_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
                boundary,
                fixture.case.label,
                "edges",
            );
            let (frontier_ui_field, frontier_count, ambiguous_count) =
                assert_status_frontier_reach(reach, fixture.owner, &fixture.case, "edges");
            assert_eq!(
                ui_field(ui, frontier_ui_field.as_str()),
                frontier_count.to_string()
            );
            assert_eq!(
                ui_field(ui, "reach_ambiguous_frontier_calls"),
                ambiguous_count.to_string()
            );
        }
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing generated-boundary call context"
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
    }
}

#[tokio::test]
async fn code_item_edges_returns_route_oneshot_targetless_real_corpus_rows() {
    for case in ReceiverToolCase::ROUTE_ONESHOT {
        let fixture = ReceiverToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("axum-route-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = payload
            .get("node_info")
            .and_then(|node| node.get("proof_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");
        let summary_needs = payload
            .get("node_info")
            .and_then(|node| node.get("external_summary_needs"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.external_summary_needs array");
        let reach_effects = payload
            .get("node_info")
            .and_then(|node| node.get("call_reach_effects"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_reach_effects array");

        // Same real-corpus Route::oneshot external-summary oracle as the
        // lookup test above, exercised through the edge-oriented payload.
        let callee = fixture.case.callee();
        let site_id = assert_method_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
            fixture.case.generic_arg_count,
            fixture.case.label,
            "edges",
        );
        if let Some(summary) = fixture.case.admitted_external_summary() {
            assert_admitted_external_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "edges",
            );
            assert_no_external_summary_need(summary_needs, site_id, fixture.case.label, "edges");
            assert_admitted_external_summary_effect(
                reach_effects,
                fixture.owner,
                site_id,
                summary,
                fixture.case.label,
                "edges",
            );
        } else {
            assert_method_proof(
                proof_context,
                fixture.owner,
                site_id,
                &fixture.case.status,
                fixture.case.label,
                "edges",
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing Route::oneshot targetless call context"
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
        assert_eq!(
            ui_field(ui, "external_summary_needs"),
            summary_needs.len().to_string()
        );
    }
}
