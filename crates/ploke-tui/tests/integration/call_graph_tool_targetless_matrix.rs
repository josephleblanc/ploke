use std::borrow::Cow;

use ploke_core::rag_types::{CallCalleeInfo, CallContextInfo};
use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
    get_code_edges::{CodeItemEdges, EdgesParams},
};

use crate::call_graph_tool_support::{
    DynamicToolCase, DynamicToolFixture, PathToolCase, PathToolFixture, ReceiverToolCase,
    ReceiverToolFixture, assert_admitted_external_summary_proof, assert_dynamic_context,
    assert_dynamic_proof, assert_method_context, assert_method_proof, assert_parts_blocker,
    assert_path_blocker_proof, assert_path_context, assert_path_context_absent,
    assert_path_context_count, assert_path_resolution_proof, assert_runtime_dispatch_blocker,
    ui_field,
};

#[tokio::test]
async fn code_item_lookup_returns_dynamic_targetless_real_corpus_rows() {
    for case in DynamicToolCase::AXUM
        .into_iter()
        .chain(DynamicToolCase::MEMCHR)
    {
        let fixture = DynamicToolFixture::new(case).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
            parent_name: None,
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

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chains:
        //   axum/src/boxed.rs:85 calls `(self.into_route)(self.handler, state)`.
        //   axum/src/boxed.rs:120 calls `(self.into_route)(self.router, state)`.
        //   axum/src/boxed.rs:159 calls `(self.layer)(self.inner.into_route(state))`.
        //   axum/src/serve/listener.rs:236 calls `(self.tap_fn)(&mut io)`.
        //   memchr/src/memmem/searcher.rs:222 calls
        //   `(self.call)(self, prestate, haystack, needle)`.
        //   memchr/src/memmem/searcher.rs:718 calls `(self.call)(self, haystack)`.
        // Expected traversal: exact owner lookup exposes the structural
        // dynamic call_site and blocked call_resolution rows, with zero callee
        // targets until callable-field, closure, function-pointer, and
        // trait-object proof exists.
        let site_id = assert_dynamic_context(
            call_context,
            fixture.owner,
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

        // Matrix:
        //   docs/active/agents/call-graph/
        //   2026-06-28_real-corpus-call-site-oracle-matrices.md
        //
        // Source chains:
        //   axum/src/routing/route.rs:51 calls
        //   `self.0.clone().oneshot(req)`.
        //   axum/src/routing/route.rs:57 calls `self.0.oneshot(req)`.
        // Expected traversal: exact owner lookup exposes both structural
        // Route::oneshot receiver rows as external frontiers, with zero callee
        // targets until external tower receiver dispatch is modeled.
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
        assert_method_proof(
            proof_context,
            fixture.owner,
            site_id,
            &fixture.case.status,
            fixture.case.label,
            "lookup",
        );

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
        assert_method_proof(
            proof_context,
            fixture.owner,
            site_id,
            &fixture.case.status,
            fixture.case.label,
            "lookup",
        );

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
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("axum-request-parts-lookup"))
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
        //   axum-core/src/ext_traits/request_parts.rs:159 binds
        //   `parts` from `Request::new(()).into_parts()`.
        //   axum-core/src/ext_traits/request_parts.rs:164 calls
        //   `parts.extract_with_state::<State<String>, String>(&state)`.
        //   axum/src/error_handling/mod.rs:238 defines `HandleErrorFuture`.
        //   axum/src/error_handling/mod.rs:251 calls
        //   `self.project().future.poll(cx)` through a boxed dyn Future.
        // Expected traversal: exact owner lookup exposes the unsupported
        // receiver rows, preserves method-generic arguments where present,
        // and leaves them targetless until method-chain, async poll/resume, or
        // runtime trait-object dispatch proof is available.
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
        assert_method_proof(
            proof_context,
            fixture.owner,
            site_id,
            &fixture.case.status,
            fixture.case.label,
            "lookup",
        );
        if fixture.case.expects_runtime_dispatch_blocker() {
            assert_runtime_dispatch_blocker(proof_context, site_id, fixture.case.label, "lookup");
        }
        if fixture.case.label.contains("request_parts.rs:164") {
            assert_parts_blocker(proof_context, site_id, fixture.case.label, "lookup");
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
async fn code_item_lookup_preserves_shadowed_get_targetless_boundary() {
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
        // `get(...)` path rows, keeps them targetless, and does not fabricate
        // edges from the later shadowed macro-argument calls to routing `get`.
        let callee = fixture.case.callee();
        let site_ids = assert_path_context_count(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
            2,
            fixture.case.label,
            "lookup",
        );
        for site_id in site_ids {
            assert_path_blocker_proof(
                proof_context,
                fixture.owner,
                site_id,
                fixture.case.build_domain(),
                "type_resolution_missing",
                fixture.case.label,
                "lookup",
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 2,
            "code_item_lookup should surface both shadowed get boundary rows"
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 4,
            "code_item_lookup should surface both shadowed get proof rows"
        );
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
        if fixture.case.expects_admitted_external_summary() {
            assert_admitted_external_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
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
async fn code_item_lookup_returns_generated_constructor_frontier_path_rows() {
    for case in PathToolCase::INTO_SERVICE_FUTURE_NEW {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
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
        // Source chain:
        //   axum/src/handler/service.rs:155 binds
        //   `type Future = super::future::IntoServiceFuture<H::Future>`.
        //   axum/src/handler/service.rs:174 calls
        //   `super::future::IntoServiceFuture::new(future)`.
        //   axum/src/handler/future.rs:11-18 and axum/src/macros.rs:19-20
        //   generate the concrete inherent constructor.
        // Expected traversal: exact tool lookup can target
        // `impl Service<Request<B>> for HandlerService::call` by both trait
        // input and self type, but the generated constructor remains an
        // unresolved targetless frontier until macro-expanded inherent items
        // are modeled.
        let callee = fixture.case.callee();
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
            "unresolved",
            "type_resolution_missing",
            fixture.case.label,
            "lookup",
        );
        let (unresolved_count, ambiguous_count) =
            assert_unresolved_frontier_reach(reach, fixture.owner, &fixture.case, "lookup");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing generated constructor frontier call context"
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 2,
            "code_item_lookup should surface generated constructor frontier proof rows"
        );
        assert_eq!(
            ui_field(ui, "reach_unresolved_frontier_calls"),
            unresolved_count.to_string()
        );
        assert_eq!(
            ui_field(ui, "reach_ambiguous_frontier_calls"),
            ambiguous_count.to_string()
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_dynamic_targetless_real_corpus_rows() {
    for case in DynamicToolCase::AXUM
        .into_iter()
        .chain(DynamicToolCase::MEMCHR)
    {
        let fixture = DynamicToolFixture::new(case).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
            parent_name: None,
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

        // Same real-corpus dynamic targetless oracle as the lookup test above,
        // exercised through the edge-oriented exact tool payload.
        let site_id = assert_dynamic_context(
            call_context,
            fixture.owner,
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
        assert_method_proof(
            proof_context,
            fixture.owner,
            site_id,
            &fixture.case.status,
            fixture.case.label,
            "edges",
        );

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
    }
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
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("axum-request-parts-edges"))
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

        // Same unsupported-receiver source oracles as the lookup test above,
        // exercised through the edge-oriented payload.
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
        assert_method_proof(
            proof_context,
            fixture.owner,
            site_id,
            &fixture.case.status,
            fixture.case.label,
            "edges",
        );
        if fixture.case.expects_runtime_dispatch_blocker() {
            assert_runtime_dispatch_blocker(proof_context, site_id, fixture.case.label, "edges");
        }
        if fixture.case.label.contains("request_parts.rs:164") {
            assert_parts_blocker(proof_context, site_id, fixture.case.label, "edges");
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
    }
}

fn assert_unresolved_frontier_reach(
    reach: &serde_json::Map<String, serde_json::Value>,
    owner: uuid::Uuid,
    case: &PathToolCase,
    tool: &str,
) -> (usize, usize) {
    let unresolved = reach
        .get("unresolved_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} call_reach unresolved_frontier_calls array"));
    let calls = unresolved
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("{tool} unresolved frontier rows should deserialize: {err}"));
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
                "{tool} should expose {} in unresolved frontier rows: {calls:#?}",
                case.label
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
    (calls.len(), ambiguous.len())
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
async fn code_item_edges_preserves_shadowed_get_targetless_boundary() {
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
        let site_ids = assert_path_context_count(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
            2,
            fixture.case.label,
            "edges",
        );
        for site_id in site_ids {
            assert_path_blocker_proof(
                proof_context,
                fixture.owner,
                site_id,
                fixture.case.build_domain(),
                "type_resolution_missing",
                fixture.case.label,
                "edges",
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 2,
            "code_item_edges should surface both shadowed get boundary rows"
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
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
        if fixture.case.expects_admitted_external_summary() {
            assert_admitted_external_summary_proof(
                proof_context,
                fixture.owner,
                site_id,
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
async fn code_item_edges_returns_generated_constructor_frontier_path_rows() {
    for case in PathToolCase::INTO_SERVICE_FUTURE_NEW {
        let fixture = PathToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.item),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(case.node_kind()),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: case.owner_trait().map(Cow::Borrowed),
            owner_type: case.owner_type().map(Cow::Borrowed),
            parent_name: None,
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

        // Same generated-constructor frontier oracle as the lookup test above,
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
        assert_path_resolution_proof(
            proof_context,
            fixture.owner,
            site_id,
            "bd:corpus-axum-call-graph",
            "unresolved",
            "type_resolution_missing",
            fixture.case.label,
            "edges",
        );
        let (unresolved_count, ambiguous_count) =
            assert_unresolved_frontier_reach(reach, fixture.owner, &fixture.case, "edges");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_edges should surface outgoing generated constructor frontier call context"
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
        assert_eq!(
            ui_field(ui, "reach_unresolved_frontier_calls"),
            unresolved_count.to_string()
        );
        assert_eq!(
            ui_field(ui, "reach_ambiguous_frontier_calls"),
            ambiguous_count.to_string()
        );
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

        // Same real-corpus Route::oneshot targetless oracle as the lookup test
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
        assert_method_proof(
            proof_context,
            fixture.owner,
            site_id,
            &fixture.case.status,
            fixture.case.label,
            "edges",
        );

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
    }
}
