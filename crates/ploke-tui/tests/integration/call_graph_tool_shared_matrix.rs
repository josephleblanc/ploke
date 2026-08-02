use std::sync::Arc;

use ploke_test_utils::{CallCorpusFixture, CallPipelineCoverage, call_shape_cases};
use ploke_tui::tools::{Tool, code_item_lookup::CodeItemLookup, get_code_edges::CodeItemEdges};

use crate::call_graph_tool_support::SharedCallShapeToolFixture;

#[tokio::test]
#[ignore = "full code_item_lookup/code_item_edges parity over real-corpus matrix is usage-summary heavy; run manually when auditing TUI parity"]
async fn code_item_tools_cover_shared_real_corpus_call_shape_matrix() {
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
    // Expected traversal: exact lookup of the target item exposes the resolved
    // incoming call edge; exact lookup of the owner item exposes the targetless
    // outgoing frontier row and proof blocker. The edge-oriented tool must
    // preserve the same call/proof payload.
    for corpus in [
        CallCorpusFixture::Axum,
        CallCorpusFixture::Chrono,
        CallCorpusFixture::Memchr,
        CallCorpusFixture::GenericArray,
    ] {
        let db = SharedCallShapeToolFixture::db_for_fixture(corpus);
        for case in call_shape_cases().iter().filter(|case| {
            case.fixture == corpus && case.coverage.contains(&CallPipelineCoverage::TuiTool)
        }) {
            if !case_filter_matches(case.name) {
                continue;
            }
            eprintln!("shared TUI call-shape case: {} setup", case.name);
            let fixture = SharedCallShapeToolFixture::new_with_db(case, Arc::clone(&db)).await;
            eprintln!("shared TUI call-shape case: {} lookup", case.name);
            assert_lookup_payload(&fixture).await;
            eprintln!("shared TUI call-shape case: {} edges", case.name);
            assert_edges_payload(&fixture).await;
            eprintln!("shared TUI call-shape case: {} done", case.name);
        }
    }
}

fn case_filter_matches(case_name: &str) -> bool {
    std::env::var("PLOKE_SHARED_CALL_SHAPE_CASE")
        .map(|filter| case_name.contains(&filter))
        .unwrap_or(true)
}

async fn assert_lookup_payload(fixture: &SharedCallShapeToolFixture) {
    let result = CodeItemLookup::execute(
        fixture.lookup_params(),
        fixture.ctx("shared-call-shape-lookup"),
    )
    .await
    .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.name));
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

    fixture.assert_call_context(call_context, "code_item_lookup");
    fixture.assert_proof_context(proof_context, "code_item_lookup");
    fixture.assert_ui_counts(
        result.ui_payload.as_ref().expect("ui payload"),
        proof_context.len(),
    );
}

async fn assert_edges_payload(fixture: &SharedCallShapeToolFixture) {
    let result = CodeItemEdges::execute(
        fixture.edges_params(),
        fixture.ctx("shared-call-shape-edges"),
    )
    .await
    .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.name));
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

    fixture.assert_call_context(call_context, "code_item_edges");
    fixture.assert_proof_context(proof_context, "code_item_edges");
    fixture.assert_ui_counts(
        result.ui_payload.as_ref().expect("ui payload"),
        proof_context.len(),
    );
}
