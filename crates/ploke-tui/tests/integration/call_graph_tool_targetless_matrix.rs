use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
    get_code_edges::{CodeItemEdges, EdgesParams},
};

use crate::call_graph_tool_support::{
    DynamicToolCase, DynamicToolFixture, ReceiverToolCase, ReceiverToolFixture,
    assert_dynamic_context, assert_dynamic_proof, assert_method_context, assert_method_proof,
    ui_field,
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
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("axum-dynamic-lookup"))
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
        // Expected traversal: exact owner lookup exposes the structural
        // dynamic call_site and blocked call_resolution rows, with zero callee
        // targets until callable-field, closure, and trait-object proof exists.
        let site_id =
            assert_dynamic_context(call_context, fixture.owner, fixture.case.label, "lookup");
        assert_dynamic_proof(
            proof_context,
            fixture.owner,
            site_id,
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
        let fixture = ReceiverToolFixture::new(case).await;
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
            fixture.case.label,
            "lookup",
        );
        assert_method_proof(
            proof_context,
            fixture.owner,
            site_id,
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
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("axum-dynamic-edges"))
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
        let site_id =
            assert_dynamic_context(call_context, fixture.owner, fixture.case.label, "edges");
        assert_dynamic_proof(
            proof_context,
            fixture.owner,
            site_id,
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
async fn code_item_edges_returns_route_oneshot_targetless_real_corpus_rows() {
    for case in ReceiverToolCase::ROUTE_ONESHOT {
        let fixture = ReceiverToolFixture::new(case).await;
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
            fixture.case.label,
            "edges",
        );
        assert_method_proof(
            proof_context,
            fixture.owner,
            site_id,
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
