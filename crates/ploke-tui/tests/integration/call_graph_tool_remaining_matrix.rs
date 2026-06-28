use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
    get_code_edges::{CodeItemEdges, EdgesParams},
};

use crate::call_graph_tool_support::{
    AxumRemainingTarget, AxumRemainingToolFixture, assert_expected_path_incoming_context,
    assert_target_proof, ui_field,
};

#[tokio::test]
async fn code_item_lookup_returns_remaining_real_corpus_supported_callers() {
    for case in AxumRemainingTarget::TOOL_REACHABLE_CASES {
        let fixture = AxumRemainingToolFixture::new(case).await;
        let module_path = fixture.module_path_arg();
        let params = LookupParams {
            item_name: Cow::Borrowed(fixture.item_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(fixture.node_kind),
            module_path: Cow::Owned(module_path),
            owner_trait: fixture.owner_trait.map(Cow::Borrowed),
            owner_type: fixture.owner_type.map(Cow::Borrowed),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("axum-remaining-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.label));
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
        // This batch covers additional tool-reachable supported rows from the
        // DB/RAG matrix: try_downcast helpers and the
        // FromRequest/FromRequestParts/FromRef trait method bindings. Each
        // exact lookup should expose the same incoming call-site identities as
        // the target-centered DB traversal.
        assert_expected_path_incoming_context(
            call_context,
            &fixture.callers,
            fixture.target,
            "code_item_lookup",
            fixture.label,
        );
        for caller in &fixture.callers {
            assert_target_proof(
                proof_context,
                caller.owner,
                fixture.target,
                "code_item_lookup",
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert_eq!(
            ui_field(ui, "call_context_incoming"),
            fixture.callers.len().to_string().as_str()
        );
    }
}

#[tokio::test]
async fn code_item_edges_returns_remaining_real_corpus_supported_callers() {
    for case in AxumRemainingTarget::TOOL_REACHABLE_CASES {
        let fixture = AxumRemainingToolFixture::new(case).await;
        let module_path = fixture.module_path_arg();
        let params = EdgesParams {
            item_name: Cow::Borrowed(fixture.item_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed(fixture.node_kind),
            module_path: Cow::Owned(module_path),
            owner_trait: fixture.owner_trait.map(Cow::Borrowed),
            owner_type: fixture.owner_type.map(Cow::Borrowed),
        };

        let result = CodeItemEdges::execute(params, fixture.ctx("axum-remaining-edges"))
            .await
            .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.label));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let node_info = payload.get("node_info").expect("node_info");
        let call_context = node_info
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");
        let proof_context = node_info
            .get("proof_context")
            .and_then(serde_json::Value::as_array)
            .expect("node_info.proof_context array");

        // Same real-corpus oracle batch as the lookup test above, exercised
        // through `code_item_edges` so tool callers can traverse from the exact
        // target item to its incoming callsites in the edge-oriented payload.
        assert_expected_path_incoming_context(
            call_context,
            &fixture.callers,
            fixture.target,
            "code_item_edges",
            fixture.label,
        );
        for caller in &fixture.callers {
            assert_target_proof(
                proof_context,
                caller.owner,
                fixture.target,
                "code_item_edges",
            );
        }

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert_eq!(
            ui_field(ui, "call_context_incoming"),
            fixture.callers.len().to_string().as_str()
        );
    }
}
