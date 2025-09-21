use crate::llm::{LlmEvent, manager::events::ChatEvt};
use std::{
    ops::{ControlFlow, Deref},
    path::PathBuf,
};

use ploke_core::rag_types::{AssembledContext, ContextPart};
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
<-- SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.
"#;

pub static PROMPT_CODE: &str = r#"
Tool-aware code collaboration instructions

You can call tools to request more context and to stage code edits for user approval.

- Notes:
  - You do NOT provide byte offsets or hashes; we will resolve the canonical path to a node span and validate file hashes internally.
  - Provide complete item definitions (rewrite), including attributes and docs where appropriate.

Conversation structure
- After the Code section below, the User's query appears under a # USER header.
- If additional responses from collaborators appear (Assistant/Collaborator), treat them as context.
- When uncertain, ask for missing details or request additional context precisely.

# Code

"#;
static PROMPT_USER: &str = r#"
# USER

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
        chat::add_msg_immediate(
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
        let messages: Vec<RequestMessage> = guard.current_path_as_llm_request_messages();
        let budget = &state.budget;
        // TODO: Add this to the program config
        let top_k = TOP_K;
        let retrieval_strategy = RETRIEVAL_STRATEGY.deref();
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

        // TODO: Change this to LlmEvent and expand event_bus event types
        event_bus.send(AppEvent::Llm(augmented_prompt));
    } else {
        let msg = "No RAG configured";
        add_msg(msg).await;
    }
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

    let mut base: Vec<ReqMsg> = Vec::from([
        (ReqMsg::new_system(String::from(PROMPT_HEADER))),
        (ReqMsg::new_system(String::from(PROMPT_CODE))),
    ]);

    // Add assembled context parts as system messages
    let text = ctx.parts.into_iter()
        .map(reformat_context_to_system)
        .map(ReqMsg::new_system);
    base.extend(text);

    // Add conversation messages
    base.extend(messages);

    LlmEvent::ChatCompletion(ChatEvt::PromptConstructed {
        parent_id,
        formatted_prompt: base,
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
