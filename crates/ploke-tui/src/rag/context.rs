use std::{ops::ControlFlow, path::PathBuf};

use ploke_core::rag_types::AssembledContext;
use ploke_rag::{RetrievalStrategy, RrfConfig};
use tokio::sync::oneshot;

use crate::{
    app_state::handlers::{chat, embedding::wait_on_oneshot},
    chat_history::{Message, MessageKind},
    error::ErrorExt as _,
};

use super::*;

pub static PROMPT_HEADER: &str = r#"
<-- SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.
"#;

// TODO: Review the prompt, I think the LLM fucked it up.
pub static PROMPT_CODE: &str = r#"
Tool-aware code collaboration instructions

You can call tools to request more context and to stage code edits for user approval.
Use the tools ONLY as described below.

Code context format
- You will receive code snippets as system messages. Headers may include:
  - File: absolute path to the file
  - Span: byte or line range the snippet covers
  - File Hash: a UUID identifying the current on-disk content (TrackingHash)
- Snippet hashes and byte ranges are for validation on our side; you should not compute them.

How to request more context
- Call the tool request_code_context when you need additional code to proceed.
- Arguments:
  - token_budget: integer > 0 indicating approximately how many tokens of code to return
  - hint: optional string that narrows the search (file path, symbol name, module, brief description)
- Keep requests focused and iterative.

How to fetch file metadata (TrackingHash)
- If you need file metadata for reasoning, call get_file_metadata with:
  - file_path: absolute path to the file on disk
- The tool returns the file's current tracking hash (UUID), byte length, and modified time.

How to propose code edits (rewrite an entire item)
- Call apply_code_edit with:
  {
    "edits": [
      {
        "action": "code_edit",
        "file": "example_crate/src/main.rs",            // path relative to your project root
        "canon": "module_one::example_module::Thing",   // canonical path of the item (no leading 'crate')
        "node_type": "function",                         // one of: function|struct|enum|trait|type_alias|module|static|const|macro|import|impl|union
        "code": "fn thing() { /* new implementation */ }"
      }
    ]
  }
- Notes:
  - You do NOT provide byte offsets or hashes; we will resolve the canonical path to a node span and validate file hashes internally.
  - Provide complete item definitions (rewrite), including attributes and docs where appropriate.

Examples
- Request context for a file by path:
  request_code_context({ "token_budget": 2000, "hint": "file: crates/ploke-tui/src/app_state/handlers/rag.rs" })
- Request context for a symbol:
  request_code_context({ "token_budget": 1500, "hint": "function: handle_tool_call_requested in rag.rs" })
- Stage a single-item rewrite:
  apply_code_edit({
    "edits": [
      {
        "action": "code_edit",
        "file": "crates/ploke-tui/src/app_state/handlers/rag.rs",
        "canon": "app_state::handlers::rag::handle_tool_call_requested",
        "node_type": "function",
        "code": "pub async fn handle_tool_call_requested(/* ... */) { /* new body */ }"
      }
    ]
  })

Conversation structure
- After the Code section below, the User's query appears under a # USER header.
- If additional responses from collaborators appear (Assistant/Collaborator), treat them as context.
- When uncertain, ask for missing details or request additional context precisely.

# Code

"#;
static PROMPT_USER: &str = r#"
# USER

"#;

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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
        )
    };

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
