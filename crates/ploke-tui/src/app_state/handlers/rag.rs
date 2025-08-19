use ploke_rag::AssembledContext;
use ploke_rag::RagService;
use ploke_rag::RetrievalStrategy;
use ploke_rag::RrfConfig;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::AppEvent;
use crate::EventBus;
use crate::chat_history::Message;
use crate::chat_history::MessageKind;
use crate::error::ErrorExt;
use crate::llm;
use crate::system::SystemEvent;

use crate::AppState;
use crate::RagEvent;

use super::embedding::wait_on_oneshot;

static PROMPT_HEADER: &str = r#"
<-- SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.
"#;

static PROMPT_CODE: &str = r#"
Next, you will be provided with some of the user's code, that has been retrieved
to provide helpful context for you to answer their questions. This context will
be provided within code tags like these:

<code="path/to/file.rs" #132:486>Code goes here</code>

Where the "path/to/file.rs" is the absolute path to the file and the #132:486
are the line numbers, inclusive.

What follows is the provided code snippets for you to use as reference, and will
be shown in a header (like # Header) and with subheaders (like ## subheader).
Follow the code section will be the User's query, delineated by a header.

After the user query, there may be a response from another collaborator marked
with a header (like # Assistant or # Collaborator). These headers may alternate
and contain subheaders with the whole text of their messages so far, summaries
of the conversation, or other contextual information about the code base.

# Code

"#;
static PROMPT_USER: &str = r#"
# USER

"#;

#[tracing::instrument(skip(state, event_bus, arguments), fields(%request_id, %parent_id, call_id = %call_id, tool = %name))]
pub async fn handle_tool_call_requested(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    request_id: Uuid,
    parent_id: Uuid,
    vendor: llm::ToolVendor,
    name: String,
    arguments: serde_json::Value,
    call_id: String,
) {
    tracing::info!(
        "handle_tool_call_requested: vendor={:?}, name={}",
        vendor,
        name
    );
    tracing::warn!(
        "DEPRECATED PATH: SystemEvent::ToolCallRequested execution path is deprecated; will be refactored into dedicated tool events. Kept for compatibility."
    );
    if name != "request_code_context" {
        tracing::warn!("Unsupported tool call: {}", name);
        let _ = event_bus
            .realtime_tx
            .send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id,
                parent_id,
                call_id,
                error: format!("Unsupported tool: {}", name),
            }));
        return;
    }

    // Parse arguments
    let token_budget = arguments
        .get("token_budget")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    if token_budget.is_none() || token_budget == Some(0) {
        let _ = event_bus
            .realtime_tx
            .send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id,
                parent_id,
                call_id,
                error: "Invalid or missing token_budget".to_string(),
            }));
        return;
    }
    let token_budget = token_budget.unwrap();
    let hint = arguments
        .get("hint")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Determine query: prefer hint, otherwise last user message
    let query = if let Some(h) = hint.filter(|s| !s.trim().is_empty()) {
        h
    } else {
        let guard = state.chat.read().await;
        match guard.last_user_msg() {
            Ok(Some((_id, content))) => content,
            _ => String::new(),
        }
    };

    if query.trim().is_empty() {
        let _ = event_bus
            .realtime_tx
            .send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id,
                parent_id,
                call_id,
                error: "No query available (no hint provided and no recent user message)"
                    .to_string(),
            }));
        return;
    }

    let top_k = calc_top_k_for_budget(token_budget);

    if let Some(rag) = &state.rag {
        match rag.hybrid_search(&query, top_k).await {
            Ok(results) => {
                let results_json: Vec<serde_json::Value> = results
                    .into_iter()
                    .map(|(id, score)| serde_json::json!({"id": id.to_string(), "score": score}))
                    .collect();

                let content = serde_json::json!({
                    "ok": true,
                    "query": query,
                    "top_k": top_k,
                    "results": results_json
                })
                .to_string();

                let _ =
                    event_bus
                        .realtime_tx
                        .send(AppEvent::System(SystemEvent::ToolCallCompleted {
                            request_id,
                            parent_id,
                            call_id,
                            content,
                        }));
            }
            Err(e) => {
                let msg = format!("RAG hybrid_search failed: {}", e);
                tracing::warn!("{}", msg);
                let _ = event_bus
                    .realtime_tx
                    .send(AppEvent::System(SystemEvent::ToolCallFailed {
                        request_id,
                        parent_id,
                        call_id,
                        error: msg,
                    }));
            }
        }
    } else {
        let msg = "RAG service unavailable".to_string();
        tracing::warn!("{}", msg);
        let _ = event_bus
            .realtime_tx
            .send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id,
                parent_id,
                call_id,
                error: msg,
            }));
    }
}

fn calc_top_k_for_budget(token_budget: u32) -> usize {
    let top_k = (token_budget / 200) as usize;
    if top_k < 5 {
        5
    } else if top_k > 20 {
        20
    } else {
        top_k
    }
}

pub async fn process_with_rag(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    scan_rx: oneshot::Receiver<Option<Vec<PathBuf>>>,
    new_msg_id: Uuid,
    completion_rx: oneshot::Receiver<()>,
) {
    if let ControlFlow::Break(_) = wait_on_oneshot(new_msg_id, completion_rx).await {
        return;
    }
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };
    if let Some(rag) = &state.rag {
        let guard = state.chat.read().await;

        let (msg_id, user_msg) = {
            match guard.last_user_msg().inspect_err(|e| e.emit_error()) {
                Ok(maybe_msg) => match maybe_msg {
                    Some(msg) => msg,
                    None => {
                        tracing::warn!("Attempting to submit empty user message");
                        return;
                    }
                },
                Err(e) => {
                    e.emit_error();
                    return;
                }
            }
        };
        let messages: Vec<Message> = guard.clone_current_path_conv();
        let budget = &state.budget;
        // TODO: Add this to the program config
        let top_k = 15;
        let retrieval_strategy = RetrievalStrategy::Hybrid {
            rrf: RrfConfig::default(),
            mmr: None,
        };
        let rag_ctx = match rag
            .get_context(&user_msg, top_k, budget, retrieval_strategy)
            .await
        {
            Ok(res) => res,
            Err(e) => {
                e.emit_error();
                tracing::error!("Failed to return results from hybrid RAG");
                return;
            }
        };
        let augmented_prompt = construct_context_from_rag(rag_ctx, messages, msg_id);

        event_bus.send(AppEvent::Llm(augmented_prompt));
    }
}

fn construct_context_from_rag(
    ctx: AssembledContext,
    messages: Vec<Message>,
    parent_id: Uuid,
) -> llm::Event {
    tracing::info!(
        "constructing context (RAG) with {} parts and {} messages",
        ctx.parts.len(),
        messages.len()
    );

    let mut base: Vec<(MessageKind, String)> = Vec::from([
        (MessageKind::System, String::from(PROMPT_HEADER)),
        (MessageKind::System, String::from(PROMPT_CODE)),
    ]);

    // Add assembled context parts as system messages
    let text = ctx.parts.into_iter().map(|p| (MessageKind::System, p.text));
    base.extend(text);

    // Add conversation messages
    let msgs = messages
        .into_iter()
        .filter(|m| m.kind == MessageKind::User || m.kind == MessageKind::Assistant)
        .inspect(|m| tracing::debug!("m.content.is_empty() = {}", m.content.is_empty()))
        .map(|msg| (msg.kind, msg.content));
    base.extend(msgs);

    llm::Event::PromptConstructed {
        parent_id,
        prompt: base,
    }
}

pub async fn bm25_rebuild(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

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

pub async fn bm25_search(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    query: String,
    top_k: usize,
) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

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

pub async fn hybrid_search(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    query: String,
    top_k: usize,
) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

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
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

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
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

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
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

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
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

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
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

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
    budget: &ploke_rag::TokenBudget,
    strategy: ploke_rag::RetrievalStrategy,
) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

    if let Some(rag) = &state.rag {
        match rag.get_context(&user_query, top_k, &budget, strategy).await {
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
