use ploke_test_utils::{CallPipelineCoverage, call_shape_cases};
use ploke_tui::tools::{Tool, code_item_lookup::CodeItemLookup, get_code_edges::CodeItemEdges};

use crate::call_graph_tool_support::SharedCallShapeToolFixture;

#[tokio::test]
async fn code_item_lookup_covers_shared_real_corpus_call_shape_matrix() {
    for case in call_shape_cases()
        .iter()
        .filter(|case| case.coverage.contains(&CallPipelineCoverage::TuiTool))
    {
        let fixture = SharedCallShapeToolFixture::new(case).await;
        let result = CodeItemLookup::execute(
            fixture.lookup_params(),
            fixture.ctx("shared-call-shape-lookup"),
        )
        .await
        .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", case.name));
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
        // Shared case source locations include:
        //   axum-macros/src/typed_path.rs:23
        //   chrono/src/offset/mod.rs:143
        //   axum/src/handler/service.rs:174
        //   axum/src/serve/listener.rs:236
        //
        // Expected traversal: exact lookup of the target item exposes the
        // resolved incoming call edge; exact lookup of the owner item exposes
        // the targetless outgoing frontier row and proof blocker.
        fixture.assert_call_context(call_context, "code_item_lookup");
        fixture.assert_proof_context(proof_context, "code_item_lookup");
        fixture.assert_ui_counts(
            result.ui_payload.as_ref().expect("ui payload"),
            proof_context.len(),
        );
    }
}

#[tokio::test]
async fn code_item_edges_covers_shared_real_corpus_call_shape_matrix() {
    for case in call_shape_cases()
        .iter()
        .filter(|case| case.coverage.contains(&CallPipelineCoverage::TuiTool))
    {
        let fixture = SharedCallShapeToolFixture::new(case).await;
        let result = CodeItemEdges::execute(
            fixture.edges_params(),
            fixture.ctx("shared-call-shape-edges"),
        )
        .await
        .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", case.name));
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

        // Same shared real-corpus call-shape matrix as the lookup test,
        // exercised through the edge-oriented tool payload.
        fixture.assert_call_context(call_context, "code_item_edges");
        fixture.assert_proof_context(proof_context, "code_item_edges");
        fixture.assert_ui_counts(
            result.ui_payload.as_ref().expect("ui payload"),
            proof_context.len(),
        );
    }
}
