use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use super::super::core::AppState;

pub async fn bm25_rebuild(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    let add_msg = |msg: &str| super::chat::add_msg_immediate(
        state, event_bus, Uuid::new_v4(), msg.to_string(), crate::chat_history::MessageKind::SysInfo);

    if let Some(rag) = &state.rag {
        match rag.bm25_rebuild().await {
            Ok(()) => add_msg("BM25 rebuild requested").await,
            Err(e) => {
                let msg = format!("BM25 rebuild failed: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot rebuild BM25").await;
    }
}

pub async fn bm25_search(state: &Arc<AppState>, event_bus: &Arc<EventBus>, query: String, top_k: usize) {
    let add_msg = |msg: &str| super::chat::add_msg_immediate(
        state, event_bus, Uuid::new_v4(), msg.to_string(), crate::chat_history::MessageKind::SysInfo);

    if let Some(rag) = &state.rag {
        match rag.search_bm25(&query, top_k).await {
            Ok(results) => {
                let lines: Vec<String> = results
                    .into_iter()
                    .map(|(id, score)| format!("{}: {:.3}", id, score))
                    .collect();
                let content = if lines.is_empty() {
                    format!("BM25 results (top {}): <no hits>", top_k)
                } else {
                    format!("BM25 results (top {}):\n{}", top_k, lines.join("\n"))
                };
                add_msg(&content).await;
            }
            Err(e) => {
                let msg = format!("BM25 search failed: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot run BM25 search").await;
    }
}

pub async fn hybrid_search(state: &Arc<AppState>, event_bus: &Arc<EventBus>, query: String, top_k: usize) {
    let add_msg = |msg: &str| super::chat::add_msg_immediate(
        state, event_bus, Uuid::new_v4(), msg.to_string(), crate::chat_history::MessageKind::SysInfo);

    if let Some(rag) = &state.rag {
        match rag.hybrid_search(&query, top_k).await {
            Ok(results) => {
                let lines: Vec<String> = results
                    .into_iter()
                    .map(|(id, score)| format!("{}: {:.3}", id, score))
                    .collect();
                let content = if lines.is_empty() {
                    format!("Hybrid results (top {}): <no hits>", top_k)
                } else {
                    format!("Hybrid results (top {}):\n{}", top_k, lines.join("\n"))
                };
                add_msg(&content).await;
            }
            Err(e) => {
                let msg = format!("Hybrid search failed: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot run hybrid search").await;
    }
}

pub async fn bm25_status(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    let add_msg = |msg: &str| super::chat::add_msg_immediate(
        state, event_bus, Uuid::new_v4(), msg.to_string(), crate::chat_history::MessageKind::SysInfo);

    if let Some(rag) = &state.rag {
        match rag.bm25_status().await {
            Ok(status) => {
                let msg = format!("BM25 status: {:?}", status);
                add_msg(&msg).await;
            }
            Err(e) => {
                let msg = format!("BM25 status error: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot query BM25 status").await;
    }
}

pub async fn bm25_save(state: &Arc<AppState>, event_bus: &Arc<EventBus>, path: PathBuf) {
    let add_msg = |msg: &str| super::chat::add_msg_immediate(
        state, event_bus, Uuid::new_v4(), msg.to_string(), crate::chat_history::MessageKind::SysInfo);

    if let Some(rag) = &state.rag {
        match rag.bm25_save(path.clone()).await {
            Ok(()) => {
                let msg = format!("BM25 index saved to {}", path.display());
                add_msg(&msg).await;
            }
            Err(e) => {
                let msg = format!("BM25 save failed: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot save BM25 index").await;
    }
}

pub async fn bm25_load(state: &Arc<AppState>, event_bus: &Arc<EventBus>, path: PathBuf) {
    let add_msg = |msg: &str| super::chat::add_msg_immediate(
        state, event_bus, Uuid::new_v4(), msg.to_string(), crate::chat_history::MessageKind::SysInfo);

    if let Some(rag) = &state.rag {
        match rag.bm25_load(path.clone()).await {
            Ok(()) => {
                let msg = format!("BM25 index load requested from {}", path.display());
                add_msg(&msg).await;
            }
            Err(e) => {
                let msg = format!("BM25 load failed: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot load BM25 index").await;
    }
}

pub async fn sparse_search(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    req_id: Uuid,
    query: String,
    top_k: usize,
    strict: bool,
) {
    let add_msg = |msg: &str| super::chat::add_msg_immediate(
        state, event_bus, Uuid::new_v4(), msg.to_string(), crate::chat_history::MessageKind::SysInfo);

    if let Some(rag) = &state.rag {
        let result = if strict {
            rag.search_bm25_strict(&query, top_k).await
        } else {
            rag.search_bm25(&query, top_k).await
        };
        match result {
            Ok(results) => {
                let lines: Vec<String> = results
                    .into_iter()
                    .map(|(id, score)| format!("{}: {:.3}", id, score))
                    .collect();
                let header = format!(
                    "BM25 {}results (req_id: {}, top {}):",
                    if strict { "strict " } else { "" },
                    req_id,
                    top_k
                );
                let content = if lines.is_empty() {
                    format!("{} <no hits>", header)
                } else {
                    format!("{}\n{}", header, lines.join("\n"))
                };
                add_msg(&content).await;
            }
            Err(e) => {
                let msg = format!("BM25 search (req_id: {}) failed: {}", req_id, e);
                add_msg(&msg).await;
            }
        }
    } else {
        let msg = format!(
            "RAG service unavailable; cannot run BM25 search (req_id: {})",
            req_id
        );
        add_msg(&msg).await;
    }
}

pub async fn dense_search(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    req_id: Uuid,
    query: String,
    top_k: usize,
) {
    let add_msg = |msg: &str| super::chat::add_msg_immediate(
        state, event_bus, Uuid::new_v4(), msg.to_string(), crate::chat_history::MessageKind::SysInfo);

    if let Some(rag) = &state.rag {
        match rag.search(&query, top_k).await {
            Ok(results) => {
                let lines: Vec<String> = results
                    .into_iter()
                    .map(|(id, score)| format!("{}: {:.3}", id, score))
                    .collect();
                let header = format!("Dense results (req_id: {}, top {}):", req_id, top_k);
                let content = if lines.is_empty() {
                    format!("{} <no hits>", header)
                } else {
                    format!("{}\n{}", header, lines.join("\n"))
                };
                add_msg(&content).await;
            }
            Err(e) => {
                let msg = format!("Dense search (req_id: {}) failed: {}", req_id, e);
                add_msg(&msg).await;
            }
        }
    } else {
        let msg = format!(
            "RAG service unavailable; cannot run dense search (req_id: {})",
            req_id
        );
        add_msg(&msg).await;
    }
}

pub async fn assemble_context(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    req_id: Uuid,
    user_query: String,
    top_k: usize,
    budget: ploke_rag::TokenBudget,
    strategy: ploke_rag::RetrievalStrategy,
) {
    let add_msg = |msg: &str| super::chat::add_msg_immediate(
        state, event_bus, Uuid::new_v4(), msg.to_string(), crate::chat_history::MessageKind::SysInfo);

    if let Some(rag) = &state.rag {
        match rag.get_context(&user_query, top_k, budget, strategy).await {
            Ok(_ctx) => {
                let msg = format!(
                    "Assembled context successfully (req_id: {}, top_k: {})",
                    req_id, top_k
                );
                add_msg(&msg).await;
            }
            Err(e) => {
                let msg = format!("Assemble context (req_id: {}) failed: {}", req_id, e);
                add_msg(&msg).await;
            }
        }
    } else {
        let msg = format!(
            "RAG service unavailable; cannot assemble context (req_id: {})",
            req_id
        );
        add_msg(&msg).await;
    }
}
