use crate::{
    chat_history::{ContextStatus, TurnsToLive},
    llm::{
        manager::Role, ChatEvt, LlmEvent
    },
};
use std::{
    ops::{ControlFlow, Deref},
    path::PathBuf,
};

use once_cell::sync::Lazy;
use ploke_core::{
    ArcStr,
    rag_types::{AssembledContext, ContextPart},
};
use ploke_rag::{RetrievalStrategy, RrfConfig};
use tokio::sync::oneshot;

use crate::{
    RETRIEVAL_STRATEGY, TOP_K,
    app_state::handlers::{chat, embedding::wait_on_oneshot},
    chat_history::{Message, MessageKind},
    error::ErrorExt as _,
    llm::manager::RequestMessage,
};

use super::*;

pub static PROMPT_HEADER: &str = r#"
<-- BEGIN SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.
<-- END SYSTEM PROMPT -->
"#;

/// Reads the just-submitted user message and:
/// - uses rag strategy to find similar items from the code graph
/// - adds conversation history from tail of last submitted user message
/// - forwards the complete and formatted messages to the system managing the API call
pub async fn process_with_rag(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    scan_rx: oneshot::Receiver<Option<Vec<PathBuf>>>,
    new_msg_id: Uuid,
    completion_rx: oneshot::Receiver<()>,
) {
    let add_msg = |msg: &str| {
        chat::add_msg_immediate_nofocus(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
        )
    };
    if let ControlFlow::Break(_) = wait_on_oneshot(new_msg_id, completion_rx).await {
        let msg = "ScanForChange did not complete successfully";
        add_msg(msg).await;
        return;
    }

    // Obtain last user message and the current conversation path as LLM request messages
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

    if let Some(rag) = &state.rag {
        let messages: Vec<RequestMessage> = guard.current_path_as_llm_request_messages();
        let budget = &state.budget;
        let top_k = TOP_K;
        let retrieval_strategy = RETRIEVAL_STRATEGY.deref();
        match rag
            .get_context(&user_msg, top_k, budget, retrieval_strategy)
            .await
        {
            Ok(rag_ctx) => {
                let augmented_prompt = construct_context_from_rag(rag_ctx, messages, msg_id);
                event_bus.send(AppEvent::Llm(augmented_prompt));
                return;
            }
            Err(e) => {
                e.emit_error();
                tracing::error!("RAG get_context failed; falling back to conversation-only prompt");
            }
        }
    } else {
        add_msg("No RAG configured; using conversation-only prompt").await;
    }

    // Conversation-only fallback: prepend a short system notice then send PromptConstructed
    let convo_only: Vec<RequestMessage> = guard.current_path_as_llm_request_messages();
    let (crate_loaded, first_tip): (bool, bool) = {
        let mut sys = state.system.write().await;
        let loaded = sys.crate_focus.is_some();
        let first = !sys.no_workspace_tip_shown;
        if !loaded {
            sys.no_workspace_tip_shown = true;
        }
        (loaded, first)
    };
    drop(guard);

    // If no crate is loaded, surface a user-facing tip in chat
    if !crate_loaded && first_tip {
        add_msg("No workspace is selected. Tip: use 'index start <path>' to index a project or 'load crate <name>' to load a saved database. Proceeding without code context.").await;
    }
    let mut formatted: Vec<RequestMessage> = Vec::with_capacity(convo_only.len() + 1);
    formatted.push(RequestMessage::new_system(
        "No workspace context loaded; proceeding without code context. Index or load a workspace to enable RAG.".to_string(),
    ));
    formatted.extend(convo_only.into_iter());

    event_bus.send(AppEvent::Llm(LlmEvent::ChatCompletion(
        ChatEvt::PromptConstructed {
            parent_id: msg_id,
            formatted_prompt: formatted,
        },
    )));
}

/// Reformats the different kinds (in this order) os messages from:
///
/// - System Prompt
///     - system prompt (consts) -> System
/// - Retrieved code context
///     - retrieved code context -> System
/// - Message History (newest to oldest)
///     - User -> User
///     - Assistant -> Assistant
///     - SysInfo -> filtered
///     - System -> filtered
///
/// Returns an event that is sent in the caller to the system managing the API call
fn construct_context_from_rag(
    ctx: AssembledContext,
    messages: Vec<RequestMessage>,
    parent_id: Uuid,
) -> LlmEvent {
    use RequestMessage as ReqMsg;

    tracing::info!(
        "constructing context (RAG) with {} parts and {} messages",
        ctx.parts.len(),
        messages.len()
    );

    // Add assembled context parts as system messages
    let mut text = ctx
        .parts
        .into_iter()
        .map(reformat_context_to_system)
        .map(ReqMsg::new_system)
        .collect::<Vec<RequestMessage>>();

    // Add conversation messages
    text.extend(messages);

    LlmEvent::ChatCompletion(ChatEvt::PromptConstructed {
        parent_id,
        formatted_prompt: text,
    })
}

fn reformat_context_to_system(ctx_part: ContextPart) -> String {
    format!(
        "file_path: {}\ncanon_path: {}\ncode_snippet: {}",
        ctx_part.file_path.as_ref(),
        ctx_part.canon_path.as_ref(),
        ctx_part.text
    )
}
