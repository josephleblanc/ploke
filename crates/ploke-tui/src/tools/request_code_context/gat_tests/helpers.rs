use super::*;

use crate::app::commands::harness::TestRuntime;
use crate::tools::ToolResult;
use crate::user_config::RetrievalStrategyUser;
use ploke_core::ArcStr;
use ploke_core::rag_types::RequestCodeContextResult;
use ploke_db::Database;
use ploke_db::bm25_index::bm25_service::Bm25Status;
use ploke_embed::indexer::EmbeddingProcessor;
use ploke_rag::RagConfig;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use uuid::Uuid;

pub(in super::super) async fn execute_fixture_request(
    db: &Arc<Database>,
    search_term: &str,
    top_k: usize,
    call_id: &'static str,
) -> color_eyre::Result<RequestCodeContextResult> {
    let tool_result = execute_fixture_tool_request(db, search_term, top_k, call_id).await?;
    Ok(serde_json::from_str(&tool_result.content)?)
}

pub(in super::super) async fn execute_fixture_tool_request(
    db: &Arc<Database>,
    search_term: &str,
    top_k: usize,
    call_id: &'static str,
) -> color_eyre::Result<ToolResult> {
    let mut rag_config = RagConfig::default();
    rag_config.call_context.max_owner_hits = 64;
    rag_config.call_context.max_caller_hits = 64;
    let rt = TestRuntime::new_with_embedding_processor_and_rag_config(
        db,
        EmbeddingProcessor::new_mock(),
        rag_config,
    );
    rt.setup_loaded_standalone_crate(ploke_test_utils::workspace_root())
        .await;
    let state = rt.state_arc();
    {
        let mut cfg = state.config.write().await;
        cfg.rag.strategy = RetrievalStrategyUser::Sparse { strict: true };
        cfg.rag.top_k = top_k;
        cfg.rag.per_part_max_tokens = 4096;
        cfg.rag.call_context.max_owner_hits = 64;
        cfg.rag.call_context.max_caller_hits = 64;
        cfg.token_limit = 65_536;
    }
    let rag = state
        .rag
        .as_ref()
        .expect("test runtime should provide RagService")
        .clone();
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture projection should enable call-context expansion"
    );
    rag.bm25_rebuild().await?;
    let mut ready = false;
    for _ in 0..50 {
        match rag.bm25_status().await? {
            Bm25Status::Ready { docs } if docs > 0 => {
                ready = true;
                break;
            }
            Bm25Status::Error(err) => panic!("BM25 rebuild failed: {err}"),
            _ => sleep(Duration::from_millis(50)).await,
        }
    }
    assert!(
        ready,
        "BM25 index must become ready before request_code_context"
    );

    let ctx = super::super::super::Ctx {
        state,
        event_bus: Arc::new(crate::EventBus::new(crate::EventBusCaps::default())),
        request_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        call_id: ArcStr::from(call_id),
    };
    let tool_result = RequestCodeContextGat::execute(
        RequestCodeContextParams {
            token_budget_per_result: Some(4096),
            token_budget_total: Some(65_536),
            search_term: Some(Cow::Borrowed(search_term)),
        },
        ctx,
    )
    .await?;

    Ok(tool_result)
}

pub(in super::super) fn assert_result_ok(
    result: &RequestCodeContextResult,
    search_term: &str,
    top_k: usize,
    fixture: &str,
) {
    assert!(
        result.ok,
        "request_code_context returned error for {fixture}: {result:#?}"
    );
    assert_eq!(result.search_term, search_term);
    assert_eq!(result.top_k, top_k);
    assert!(
        result
            .note
            .as_deref()
            .is_none_or(|note| !note.contains("Call-context expansion is unavailable")),
        "call-context degradation should not be surfaced for {fixture}: {result:#?}"
    );
}

pub(in super::super) fn ui_field(payload: &crate::tools::ToolUiPayload, name: &str) -> String {
    payload
        .fields
        .iter()
        .find(|field| field.name.as_ref() == name)
        .unwrap_or_else(|| panic!("missing {name} field in payload: {payload:#?}"))
        .value
        .as_ref()
        .to_string()
}
