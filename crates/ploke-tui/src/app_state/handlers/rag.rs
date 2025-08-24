#![allow(clippy::needless_lifetimes)]
//! RAG handlers and tool integrations for the TUI layer.
//!
//! This module provides asynchronous handlers that:
//! - Dispatch LLM tool calls and return results via the EventBus as System events
//!   (ToolCallCompleted / ToolCallFailed) and SysInfo chat messages.
//! - Stage source-code edits as proposals (including human-readable previews) and
//!   apply/deny them on user command.
//! - Run sparse (BM25), dense, and hybrid search via an optional RagService stored
//!   on AppState, and surface results as chat messages.
//! - Assemble a prompt context from retrieved snippets and the current conversation,
//!   emitting a constructed prompt event for the LLM subsystem.
//!
//! Observable characteristics (from this file):
//! - Handlers accept Arc<AppState> and Arc<EventBus) and communicate by sending AppEvent
//!   instances via the EventBus (realtime/background). No implicit global state is modified,
//!   aside from fields within AppState accessed through provided references.
//! - When required services or inputs are missing, functions emit SysInfo chat messages and/or
//!   ToolCallFailed events instead of panicking.
//! - IO and database operations are delegated to other subsystems (IoManager, Database, RagService);
//!   this module validates inputs, constructs requests, and forwards results as events/messages.
//!
//! Context in the larger project (as visible here):
//! - Events and system integration types come from the crate root (AppEvent, SystemEvent, llm::Event).
//! - The module is invoked by the state manager/dispatcher and other handlers; it does not own
//!   background loops itself. It relies on the EventBus for communicating with the UI and LLM manager.

use ploke_core::{
    PROJECT_NAMESPACE_UUID, TrackingHash, WriteSnippetData, rag_types::AssembledContext,
};
use ploke_rag::{RetrievalStrategy, RrfConfig};
use serde::Deserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
use similar::TextDiff;
use std::collections::BTreeSet;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::{
    AppEvent, AppState, EventBus,
    chat_history::{Message, MessageKind},
    error::ErrorExt,
    llm::{self, ToolEvent},
    system::SystemEvent,
};

use super::chat;
use super::embedding::wait_on_oneshot;

static PROMPT_HEADER: &str = r#"
<-- SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.
"#;

// TODO: Review the prompt, I think the LLM fucked it up.
static PROMPT_CODE: &str = r#"
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


#[tracing::instrument(skip(tool_call_params))]
pub async fn handle_tool_call_requested<'a>(tool_call_params: ToolCallParams<'a>) {
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        vendor,
        name,
        arguments,
        call_id,
    } = tool_call_params.clone();
    tracing::info!(
        "handle_tool_call_requested: vendor={:?}, name={}",
        vendor,
        name
    );
    tracing::warn!(
        "DEPRECATED PATH: SystemEvent::ToolCallRequested execution path is deprecated; will be refactored into dedicated tool events. Kept for compatibility."
    );
    let tool_call_failed = |error| {
        AppEvent::System(SystemEvent::ToolCallFailed {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            error,
        })
    };

    let tool_call_params = ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        vendor,
        name: name.clone(),
        arguments: arguments.clone(),
        call_id: call_id.clone(),
    };
    match name.as_str() {
        "apply_code_edit" => apply_code_edit_tool(tool_call_params).await,
        // New: get_file_metadata tool for fetching current file hash and basic metadata
        "get_file_metadata" => get_file_metadata_tool(tool_call_params).await,
        "request_code_context" => {}
        _ => {
            tracing::warn!("Unsupported tool call: {}", name);
            let err = format!("Unsupported tool: {}", name);
            let _ = event_bus.realtime_tx.send(tool_call_failed(err.clone()));
            return;
        }
    }

    if name != "request_code_context" {
        tracing::warn!("Unsupported tool call: {}", name);
        let err = format!("Unsupported tool: {}", name);
        let _ = event_bus.realtime_tx.send(tool_call_failed(err.clone()));
        return;
    }

    // Parse arguments
    let token_budget = arguments
        .get("token_budget")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    if token_budget.is_none() || token_budget == Some(0) {
        let msg = "Invalid or missing token_budget".to_string();
        let _ = event_bus.realtime_tx.send(tool_call_failed(msg.clone()));
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
        let msg = "No query available (no hint provided and no recent user message)".to_string();
        let _ = event_bus.realtime_tx.send(tool_call_failed(msg.clone()));
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
                            call_id: call_id.clone(),
                            content,
                        }));
            }
            Err(e) => {
                let msg = format!("RAG hybrid_search failed: {}", e);
                tracing::warn!("{}", msg);
                let _ = event_bus.realtime_tx.send(tool_call_failed(msg));
            }
        }
    } else {
        let msg = "RAG service unavailable".to_string();
        tracing::warn!("{}", msg);
        let _ = event_bus.realtime_tx.send(tool_call_failed(msg.clone()));
    }
}








