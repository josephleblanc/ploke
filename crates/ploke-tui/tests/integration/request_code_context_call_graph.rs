use std::borrow::Cow;

use ploke_core::rag_types::{CallExpansionKind, CallPathInfo, RequestCodeContextResult};
use ploke_db::bm25_index::bm25_service::Bm25Status;
use ploke_tui::{
    tools::{RequestCodeContextGat, Tool, request_code_context::RequestCodeContextParams},
    user_config::RetrievalStrategyUser,
};
use tokio::time::{Duration, sleep};
use uuid::Uuid;

use crate::call_graph_tool_support::{AxumRequestExtractPathToolFixture, ui_field};

#[tokio::test]
async fn request_code_context_returns_real_corpus_two_hop_call_path() {
    let fixture = AxumRequestExtractPathToolFixture::new().await;
    configure_request_context(&fixture).await;
    rebuild_bm25(&fixture).await;

    let result = RequestCodeContextGat::execute(
        RequestCodeContextParams {
            token_budget_per_result: Some(8192),
            token_budget_total: Some(131_072),
            search_term: Some(Cow::Borrowed("self.extract_with_state")),
        },
        fixture.ctx("axum-request-context-call-paths"),
    )
    .await
    .expect("request_code_context execution");
    let payload: RequestCodeContextResult =
        serde_json::from_str(&result.content).expect("deserialize request_code_context payload");
    assert!(
        payload.ok,
        "request_code_context should return usable axum context: {payload:#?}"
    );

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    // This exercises the broad retrieval tool boundary, proving a tool caller
    // can retrieve the real-corpus multi-hop call chain without switching to an
    // exact edge lookup.
    let start = payload
        .context
        .iter()
        .find(|part| part.id == fixture.start)
        .unwrap_or_else(|| {
            panic!("request_code_context should include RequestExt::extract: {payload:#?}")
        });
    let path = start
        .call_paths_from_owner
        .iter()
        .find(|path| path.end_id == fixture.target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!(
                "RequestExt::extract should carry the two-hop path to FromRequest::from_request: {start:#?}"
            )
        });
    assert_two_hop_path(path, fixture.start, fixture.intermediate, fixture.target);
    assert_path_node(
        path,
        fixture.start,
        "::extract",
        "axum-core/src/ext_traits/request.rs",
    );
    assert_path_node(
        path,
        fixture.intermediate,
        "::extract_with_state",
        "axum-core/src/ext_traits/request.rs",
    );
    assert_path_node(
        path,
        fixture.target,
        "::from_request",
        "axum-core/src/extract/mod.rs",
    );

    let terminal = payload
        .context
        .iter()
        .find(|part| part.id == fixture.target)
        .unwrap_or_else(|| {
            panic!(
                "request_code_context should include the two-hop terminal FromRequest::from_request: {payload:#?}"
            )
        });
    let expansion = terminal
        .call_expansion
        .expect("terminal target should carry call expansion provenance");
    assert_eq!(expansion.target_id, fixture.target);
    assert_eq!(expansion.relation, CallExpansionKind::OutgoingTarget);
    assert!(
        expansion.seed_id == fixture.start || expansion.seed_id == fixture.intermediate,
        "terminal expansion should come from the documented axum chain: {expansion:#?}"
    );
    assert!(
        (1..=2).contains(&expansion.distance),
        "broad retrieval may seed the intermediate node directly, but should not need more than the documented two-hop axum chain: {expansion:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("UI payload");
    assert!(
        ui_field(ui, "call_paths_from_owner")
            .parse::<usize>()
            .expect("outgoing path count")
            >= 1,
        "request_code_context should surface outgoing call-path carrier counts"
    );
    assert!(
        ui_field(ui, "call_expansion")
            .parse::<usize>()
            .expect("call expansion count")
            >= 1,
        "request_code_context should surface multi-hop call expansion counts"
    );
}

async fn configure_request_context(fixture: &AxumRequestExtractPathToolFixture) {
    let mut cfg = fixture.state.config.write().await;
    cfg.rag.strategy = RetrievalStrategyUser::Sparse { strict: true };
    cfg.rag.top_k = 64;
    cfg.rag.per_part_max_tokens = 8192;
    cfg.rag.call_context.max_owner_hits = 64;
    cfg.rag.call_context.max_caller_hits = 64;
    cfg.token_limit = 131_072;
}

async fn rebuild_bm25(fixture: &AxumRequestExtractPathToolFixture) {
    let rag = fixture
        .state
        .rag
        .as_ref()
        .expect("axum request fixture should provide RagService");
    rag.bm25_rebuild()
        .await
        .expect("rebuild BM25 for request_code_context");
    for _ in 0..50 {
        match rag.bm25_status().await.expect("BM25 status") {
            Bm25Status::Ready { docs } if docs > 0 => return,
            Bm25Status::Error(err) => panic!("BM25 rebuild failed: {err}"),
            _ => sleep(Duration::from_millis(50)).await,
        }
    }
    panic!("BM25 index must become ready before request_code_context");
}

fn assert_two_hop_path(path: &CallPathInfo, start: Uuid, intermediate: Uuid, target: Uuid) {
    assert_eq!(path.start_id, start);
    assert_eq!(path.end_id, target);
    assert_eq!(path.depth, 2);
    assert_eq!(path.edges.len(), 2);
    assert_eq!(path.edges[0].caller_id, start);
    assert_eq!(path.edges[0].callee_id, intermediate);
    assert_eq!(path.edges[1].caller_id, intermediate);
    assert_eq!(path.edges[1].callee_id, target);
}

fn assert_path_node(path: &CallPathInfo, id: Uuid, canon_suffix: &str, file_suffix: &str) {
    assert!(
        path.nodes.iter().any(|node| {
            node.id == id
                && node.canon_path.as_ref().ends_with(canon_suffix)
                && node.file_path.as_ref().ends_with(file_suffix)
        }),
        "call path should include node {id} ending with {canon_suffix:?} in {file_suffix:?}: {path:#?}"
    );
}
