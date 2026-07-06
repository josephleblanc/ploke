use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
    get_code_edges::{CodeItemEdges, EdgesParams},
};

use crate::call_graph_tool_support::{
    DynamicToolCase, DynamicToolFixture, PathToolCase, PathToolFixture, ReceiverToolCase,
    ReceiverToolFixture, assert_dynamic_context, assert_dynamic_proof, assert_method_context,
    assert_method_proof, assert_path_context_absent, ui_field,
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
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
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
        // Route::oneshot receiver rows, with zero callee targets until external
        // tower receiver dispatch and tuple-field receiver proof are modeled.
        let callee = fixture.case.callee();
        let site_id = assert_method_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
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
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
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
async fn code_item_lookup_returns_request_parts_targetless_real_corpus_row() {
    for case in ReceiverToolCase::REQUEST_PARTS {
        let fixture = ReceiverToolFixture::new(case.clone()).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
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
        //   axum-core/src/ext_traits/request_parts.rs:186 calls
        //   `parts.extract_with_state(state)`.
        // Expected traversal: exact owner lookup exposes the unresolved
        // local-binding receiver row with zero callee targets until receiver
        // type proof connects `parts` to RequestPartsExt::extract_with_state.
        let callee = fixture.case.callee();
        let site_id = assert_method_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
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
            "code_item_lookup should surface outgoing request-parts targetless call context"
        );
        assert!(
            ui_field(ui, "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 2,
            "code_item_lookup should surface request-parts targetless proof rows"
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
            owner_trait: None,
            owner_type: case.owner_type().map(Cow::Borrowed),
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
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
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
async fn code_item_edges_returns_request_parts_targetless_real_corpus_row() {
    for case in ReceiverToolCase::REQUEST_PARTS {
        let fixture = ReceiverToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
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

        // Same real-corpus request-parts targetless oracle as the lookup test
        // above, exercised through the edge-oriented exact tool payload.
        let callee = fixture.case.callee();
        let site_id = assert_method_context(
            call_context,
            fixture.owner,
            &callee,
            &fixture.case.status,
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
            "code_item_edges should surface outgoing request-parts targetless call context"
        );
        let proof_count = proof_context.len().to_string();
        assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
    }
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
            owner_trait: None,
            owner_type: case.owner_type().map(Cow::Borrowed),
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
async fn code_item_edges_returns_route_oneshot_targetless_real_corpus_rows() {
    for case in ReceiverToolCase::ROUTE_ONESHOT {
        let fixture = ReceiverToolFixture::new(case.clone()).await;
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
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
