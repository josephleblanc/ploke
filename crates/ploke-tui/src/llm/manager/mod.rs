use crate::llm::router_only::{ApiRoute, Router};
mod commands;
pub(crate) mod events;
mod session;

use crate::tools::create_file::CreateFile;
use crate::tools::ns_patch::NsPatch;
use crate::SystemEvent;
use crate::app_state::handlers::chat::add_msg_immediate;
use crate::error::ResultExt as _;
use crate::llm::error::LlmError;
use crate::llm::router_only::ApiRoute as _;
use crate::llm::router_only::ChatCompRequest;
use crate::llm::router_only::HasEndpoint;
use crate::llm::router_only::HasModels;
use crate::tools;
use events::ChatEvt;
pub(crate) use events::LlmEvent;
use events::endpoint;
use events::models;
use fxhash::FxHashMap as HashMap;
use itertools::Itertools;
use ploke_core::ArcStr;

use ploke_rag::TokenCounter as _;
use ploke_rag::context::ApproxCharTokenizer;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::ops::ControlFlow;
use std::{env, fs, path::PathBuf, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinSet;
use tracing::instrument;
use uuid::Uuid;

use crate::app_state::{AppState, StateCommand};
use crate::chat_history::{Message, MessageKind, MessageStatus, MessageUpdate};
use crate::rag::utils::ToolCallParams;
use crate::tools::code_edit::GatCodeEdit;
use crate::tools::request_code_context::RequestCodeContextGat;
use crate::tools::{
    FunctionCall, FunctionMarker, RequestCodeContext, Tool as _, ToolCall,
    ToolDefinition, ToolFunctionDef, ToolName,
};
use crate::utils::consts::{DEBUG_TOOLS, TOOL_CALL_CHAIN_LIMIT};
use crate::{AppEvent, EventBus};

// API and Config

use super::response::TokenUsage;
use super::router_only::openrouter::OpenRouter;
use super::router_only::openrouter::OpenRouterModelId;
use super::*;

#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, PartialOrd, Eq)]
pub(crate) struct RequestMessage {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<ArcStr>,
}

// TODO: Add Role::Tool
// - be careful when adding `Tool`
// - note differences in the way the original json handles the `type Message` when there is a
// `role: 'tool'`, such that it requires a `tool_call_id`. We will need to propogate this
// requirement somehow. Needs HUMAN decision, ask.
// - see original json below:
// ```json
// type Message =
//   | {
//       role: 'user' | 'assistant' | 'system';
//       // ContentParts are only for the "user" role:
//       content: string | ContentPart[];
//       // If "name" is included, it will be prepended like this
//       // for non-OpenAI models: `{name}: {content}`
//       name?: string;
//     }
//   | {
//       role: 'tool';
//       content: string;
//       tool_call_id: string;
//       name?: string;
//     };
// ```
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

impl RequestMessage {
    pub fn new_system(content: String) -> Self {
        Self {
            role: Role::System,
            content,
            tool_call_id: None,
        }
    }

    pub fn new_tool(content: String, tool_call_id: ArcStr) -> Self {
        Self {
            role: Role::Tool,
            content,
            tool_call_id: Some(tool_call_id),
        }
    }

    pub fn new_user(content: String) -> Self {
        Self {
            role: Role::User,
            content,
            tool_call_id: None,
        }
    }

    pub fn new_assistant(content: String) -> Self {
        Self {
            role: Role::Assistant,
            content,
            tool_call_id: None,
        }
    }

    /// Validates that the message structure is correct according to OpenAI/OpenRouter spec
    pub fn validate(&self) -> Result<(), String> {
        match self.role {
            Role::Tool => {
                if self.tool_call_id.is_none() {
                    return Err("Tool messages must have a tool_call_id".to_string());
                }
            }
            Role::User | Role::Assistant | Role::System => {
                // These roles should not have tool_call_id set, but we allow it for flexibility
            }
        }
        Ok(())
    }
}

impl From<MessageKind> for Role {
    fn from(value: MessageKind) -> Self {
        match value {
            MessageKind::User => Role::User,
            MessageKind::Assistant => Role::Assistant,
            // TODO: Should change below to Role::System, might break something, check tests
            // before/after
            MessageKind::System => Role::System,
            MessageKind::Tool => Role::Tool,
            _ => panic!("Invalid state: Cannot have a Role other than User, Assistant, and System"),
        }
    }
}

/// Event id helper struct, uses the user's previous message (parent_id) and the new id that will
/// be updated with the LLM's response (new_msg_id) to differentiate items in the queue.
// note: This is super overkill, we don't really need two 128-bit keys for such a small set of
// itmes.
// TODO: We don't currently send the msg id of the newly generated Llm message placeholder to the
// context initalization, find a good way to coordinate this.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct EvtKey {
    parent_id: Uuid,
}

pub async fn llm_manager(
    mut event_rx: broadcast::Receiver<AppEvent>,
    state: Arc<AppState>,
    cmd_tx: mpsc::Sender<StateCommand>,
    event_bus: Arc<EventBus>,
    // providers: crate::user_config::ModelRegistry,
) {
    let client = Client::new();
    let mut pending_requests: HashMap<EvtKey, ChatEvt> = HashMap::default();
    let mut ready_contexts: HashMap<EvtKey, ChatEvt> = HashMap::default();

    // Enters loop every time there is a new event.
    //  - Currently only receives:
    //      - AppEvent::Llm(Event::Request)
    //      - AppEvent::Llm(Event::PromptConstructed)
    while let Ok(event) = event_rx.recv().await {
        tracing::info!(?event);
        match event {
            AppEvent::Llm(LlmEvent::ChatCompletion(
                request @ ChatEvt::Request {
                    parent_id,
                    request_msg_id,
                    ..
                },
            )) => {
                // pairing happens when PromptConstructed arrives for the same request_id.
                let event_key = EvtKey { parent_id };
                pending_requests.insert(event_key, request);
            }
            AppEvent::Llm(LlmEvent::ChatCompletion(
                context @ ChatEvt::PromptConstructed { parent_id, .. },
            )) => {
                let event_key = EvtKey { parent_id };
                if !pending_requests.contains_key(&event_key) {
                    // no match, keep waiting
                    ready_contexts.insert(event_key, context);
                } else {
                    // match found, process request
                    let req = pending_requests
                        .remove(&event_key)
                        .expect("Event key-val must exist");
                    tokio::spawn(process_llm_request(
                        req,
                        Arc::clone(&state),
                        cmd_tx.clone(),
                        client.clone(),
                        event_bus.clone(),
                        context,
                    ));
                }
            }
            AppEvent::System(SystemEvent::ToolCallRequested {
                tool_call,
                request_id,
                parent_id,
            }) => {
                tracing::debug!(target: DEBUG_TOOLS,
                    request_id = %request_id,
                    parent_id = %parent_id,
                    call_id = ?tool_call.call_id,
                    tool = ?tool_call.function.name,
                    "Dispatching ToolEvent::Requested in LLM manager"
                );
                let state = Arc::clone(&state);
                let event_bus = Arc::clone(&event_bus);

                let ctx = crate::tools::Ctx {
                    state,
                    event_bus,
                    request_id,
                    parent_id,
                    call_id: tool_call.call_id.clone(),
                };
                tokio::task::spawn(tools::process_tool(tool_call, ctx));
            }
            AppEvent::Llm(LlmEvent::Endpoint(endpoint::Event::Request {
                model_key,
                variant,
                router,
            })) => {
                handle_endpoint_request(
                    state.clone(),
                    event_bus.clone(),
                    client.clone(),
                    model_key,
                    variant,
                );
            }
            AppEvent::Llm(LlmEvent::Models(models::Event::Request { router })) => {
                use std::str::FromStr;
                // TODO: Add `router` field to ModelEndpointRequest, then process the model_id into
                // the typed version for that specific router before making the request.
                // + make model_id typed as ModelKey
                let state = Arc::clone(&state);
                let event_bus = Arc::clone(&event_bus);
                let client = client.clone();

                tokio::task::spawn(async move {
                    use models::Event;
                    let result = OpenRouter::fetch_models(&client)
                        .await
                        .map(Arc::new)
                        .inspect_err(|e| {
                            let msg = format!("Failed to fetch models from API: {}", e);
                            tracing::warn!(msg);
                            // Unblock the UI even on error with an empty provider list.
                            event_bus.send(AppEvent::Llm(LlmEvent::Models(Event::Response {
                                models: None,
                                search_keyword: None,
                            })));
                        });
                    event_bus.send(AppEvent::Llm(LlmEvent::Models(Event::Response {
                        models: result.ok(),
                        search_keyword: None,
                    })));
                });
            }
            _ => {}
        }
    }
}

fn handle_endpoint_request(
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    client: Client,
    model_key: ModelKey,
    variant: Option<types::model_types::ModelVariant>,
) {
    use std::str::FromStr;
    // TODO: Add `router` field to ModelEndpointRequest, then process the model_id into
    // the typed version for that specific router before making the request.
    // + make model_id typed as ModelKey
    tokio::task::spawn(async move {
        use endpoint::Event;
        let model_id = ModelId::from_parts(model_key.clone(), variant);
        let typed_model = OpenRouterModelId::from(model_id);
        let result = OpenRouter::fetch_model_endpoints(&client, typed_model.clone())
            .await
            .map(Arc::new)
            .inspect_err(|e| {
                let msg = format!("Failed to fetch endpoints for {}: {:?}", typed_model, e);
                tracing::warn!(msg);
                // TODO: send a response with an error
            })
            .ok();
        event_bus.send(AppEvent::Llm(LlmEvent::Endpoint(Event::Response {
            model_key,
            endpoints: result,
        })));
    });
}

#[instrument(skip_all)]
/// The worker function that processes a single LLM request.
pub async fn process_llm_request(
    request: ChatEvt,
    state: Arc<AppState>,
    cmd_tx: mpsc::Sender<StateCommand>,
    client: Client,
    event_bus: Arc<EventBus>,
    context: ChatEvt,
) {
    // - parent_id here is the message the user sent that prompted the call to the API for the LLM's
    // response.
    // - request_message_id is the id created when the user's message is added to the conversation
    // history, and helps to co-ordinate the message to first create here and update below when the
    // placeholder message is updated with the LLM's response.
    let (parent_id, request_message_id) = match request {
        ChatEvt::Request {
            parent_id: p,
            request_msg_id: r,
        } => (p, r),
        _ => {
            tracing::debug!("Not a Request, do nothing");
            return;
        } // Not a request, do nothing
    };

    let messages = match context {
        ChatEvt::PromptConstructed {
            parent_id,
            formatted_prompt,
        } => formatted_prompt,
        _ => {
            tracing::debug!("No prompt constructed, do nothing");
            return;
        }
    };

    // llm: runtime routing uses registry prefs + active model; no legacy ModelConfig required.

    // This part remains the same: create a placeholder message first at the provided message id
    let (responder_tx, responder_rx) = oneshot::channel();
    let create_cmd = StateCommand::CreateAssistantMessage {
        parent_id,
        responder: responder_tx,
        new_assistant_msg_id: request_message_id,
    };
    cmd_tx.send(create_cmd).await.expect(
        "Invalid state: sending over closed channel from process_llm_request via StateCommand",
    );

    let assistant_message_id = match responder_rx.await {
        Ok(id) if id == request_message_id => id,
        Ok(_id) => {
            log::error!("Failed to create assistant message: mismatch in Uuid of created message.");
            return;
        }
        Err(_) => {
            log::error!("Failed to create assistant message: state_manager dropped responder.");
            return;
        }
    };

    // Prepare and execute the API call, then create the final update command.
    let result = prepare_and_run_llm_call(
        &state,
        &client,
        messages,
        event_bus.clone(),
        request_message_id,
        cmd_tx.clone()
    )
    .await;

    // Build concise per-request outcome summary before consuming `result`
    // TODO: Add a typed return value that contains a summary and an option for the content
    let summary = match &result {
        Ok(_msg) => "Request summary: [success]".to_string(),
        Err(LlmError::Api { status: 404, .. }) => {
            "Request summary: error 404 (endpoint/tool support?)".to_string()
        }
        Err(LlmError::Api { status: 429, .. }) => "Request summary: rate limited (429)".to_string(),
        Err(e) => format!("Request summary: error ({})", e),
    };

    let update_cmd = match result {
        Ok(content) => {
            // Small preview for logs so tests / debug can observe the returned text.
            let preview = content.chars().take(200).collect::<String>();
            tracing::info!(
                "LLM produced response for parent_id={} -> assistant_message_id={}. preview={}",
                parent_id,
                request_message_id,
                preview
            );

            StateCommand::AddMessageImmediate {
                msg: content,
                kind: MessageKind::Assistant,
                new_msg_id: Uuid::new_v4(),
                }
            }
        Err(e) => {
            let err_string = e.to_string();
            // Inform the user in-chat so the "Pending..." isn't left hanging without context.
            let _ = cmd_tx
                .send(StateCommand::AddMessageImmediate {
                    msg: format!("LLM request failed: {}", err_string),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                })
                .await;

            // Avoid invalid status transition by finalizing the assistant message with a failure note.
            StateCommand::AddMessageImmediate {
                msg: format!("Request failed: {}", err_string),
                kind: MessageKind::SysInfo,
                new_msg_id: Uuid::new_v4(),
                }
        }
    };

    // Send the final update command to the state manager.
    if cmd_tx.send(update_cmd).await.is_err() {
        log::error!("Failed to send final UpdateMessage: channel closed.");
    }

    // let _ = cmd_tx
    //     .send(StateCommand::AddMessageImmediate {
    //         msg: summary,
    //         kind: MessageKind::SysInfo,
    //         new_msg_id: Uuid::new_v4(),
    //     })
    //     .await;
}

#[instrument(skip_all)]
async fn prepare_and_run_llm_call(
    state: &Arc<AppState>,
    client: &Client,
    messages: Vec<RequestMessage>,
    event_bus: Arc<EventBus>,
    parent_id: Uuid,
    cmd_tx: mpsc::Sender<StateCommand>,
) -> Result<String, LlmError> {
    // 5) Tool selection. For now, expose a fixed set of tools.
    //    Later, query registry caps and enforcement policy for tool_choice.
    let tool_defs: Vec<ToolDefinition> = vec![
        RequestCodeContextGat::tool_def(),
        GatCodeEdit::tool_def(),
        CreateFile::tool_def(),
        NsPatch::tool_def(),
    ];

    // 4) Parameters (placeholder: use defaults until llm registry/prefs are wired)
    //    When registry is available, merge model/user defaults into LLMParameters.
    let _llm_params = crate::llm::LLMParameters::default();

    // 4.1) Build a router-generic ChatCompRequest using the builder pattern (OpenRouter default).
    //      Construct a concrete request object that RequestSession will dispatch.
    use crate::llm::request::endpoint::ToolChoice;
    use crate::llm::router_only;
    use crate::llm::router_only::openrouter;

    // Gate tools by crate_focus: disable when no workspace is loaded
    let crate_loaded = {
        let sys = state.system.read().await;
        sys.crate_focus.is_some()
    };
    let (tools, tool_choice) = if crate_loaded {
        (Some(tool_defs.clone()), Some(ToolChoice::Auto))
    } else {
        (None, None)
    };

    // Use the runtime-selected active model (includes optional variant)
    let model_id = {
        let cfg = state.config.read().await;
        cfg.active_model.clone()
    };

    // WARN: Using default fields here, should try to load from registry first and use default if
    // the selected model is default or if the registry is not yet set up.
    let req = OpenRouter::default_chat_completion()
        .with_core_bundle(crate::llm::request::ChatCompReqCore::default())
        .with_model(model_id)
        .with_messages(messages)
        .with_param_bundle(_llm_params)
        // TODO: This is where Registry will plug in, maybe?
        // .with_params_union(_llm_params)
        .with_tools(tools)
        .with_tool_choice(tool_choice);

    // 6) Diagnostics: skip provider-bound diag logs until registry replaces user_config.
    // let log_fut: Option<_> = None;

    // Persist a diagnostic snapshot of the outgoing "request plan" for offline analysis (disabled for now).

    // Delegate the per-request loop to RequestSession
    let session = session::RequestSession::<OpenRouter> {
        client,
        event_bus,
        parent_id,
        req,
        fallback_on_404: false,
        attempts: TOOL_CALL_CHAIN_LIMIT,
        state_cmd_tx: cmd_tx.clone(),
    };

    session.run().await

    // Persist model output or error for later inspection
    // if let Some(fut) = log_fut {
    //     todo!();
    // if fut.await.is_err() {
    //     tracing::error!("Failed to write tool use logs.");
    // }
    // }
}

pub(super) fn cap_messages_by_chars(
    messages: &[RequestMessage],
    budget: usize,
) -> Vec<RequestMessage> {
    // Walk from the tail so we keep the most recent context, then reverse to restore order
    let mut used = 0usize;
    let mut kept: Vec<&RequestMessage> = Vec::new();
    for m in messages.iter().rev() {
        let len = m.content.len();
        if used.saturating_add(len) > budget && !kept.is_empty() {
            break;
        }
        used = used.saturating_add(len);
        kept.push(m);
    }
    kept.reverse();
    // Clone into a fresh vec; RequestMessage owns String content so zero-copy is not possible here
    kept.into_iter().cloned().collect()
}

pub(super) fn cap_messages_by_tokens(
    messages: &[RequestMessage],
    token_budget: usize,
) -> Vec<RequestMessage> {
    // Use shared TokenCounter to approximate tokens deterministically
    let tokenizer = ApproxCharTokenizer;
    let mut used = 0usize;
    let mut kept: Vec<&RequestMessage> = Vec::new();
    for m in messages.iter().rev() {
        let tokens = tokenizer.count(&m.content);
        if used.saturating_add(tokens) > token_budget && !kept.is_empty() {
            break;
        }
        used = used.saturating_add(tokens);
        kept.push(m);
    }
    kept.reverse();
    kept.into_iter().cloned().collect()
}

// Diagnostics helpers (env-driven, independent of tracing)
fn diag_dir() -> Option<PathBuf> {
    // Prefer explicit env override; otherwise default to a stable test-output folder.
    let path = env::var_os("PLOKE_E2E_DIAG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/test-output/openrouter_e2e"));
    let _ = fs::create_dir_all(&path);
    Some(path)
}
fn now_ts() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

// Legacy diagnostics helpers referencing ModelConfig removed; llm routes via registry prefs.

// Example tool-call handler (stub)

// --- Supporting Types ---

/// Specifies which chat history a command should operate on.
/// Useful for applications with multiple contexts (e.g., main chat, scratchpad).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatHistoryTarget {
    /// The primary, active chat history.
    #[default]
    Main,
    /// A secondary history for notes or drafts.
    Scratchpad,
}

// --- Supporting Types ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_tool_serialization() {
        // Test that Role::Tool serializes correctly
        let role = Role::Tool;
        let serialized = serde_json::to_string(&role).unwrap();
        assert_eq!(serialized, "\"tool\"");

        // Test deserialization
        let deserialized: Role = serde_json::from_str("\"tool\"").unwrap();
        assert_eq!(deserialized, Role::Tool);
    }

    #[test]
    fn test_tool_message_constructors() {
        // Test new_tool constructor
        let call_123 = ArcStr::from("call_123");
        let tool_msg = RequestMessage::new_tool("result content".to_string(), call_123.clone());
        assert_eq!(tool_msg.role, Role::Tool);
        assert_eq!(tool_msg.content, "result content");
        assert_eq!(tool_msg.tool_call_id, Some(call_123));

        // Test validation passes for valid tool message
        assert!(tool_msg.validate().is_ok());

        // Test other constructors don't have tool_call_id
        let user_msg = RequestMessage::new_user("hello".to_string());
        assert_eq!(user_msg.role, Role::User);
        assert_eq!(user_msg.tool_call_id, None);
        assert!(user_msg.validate().is_ok());
    }

    #[test]
    fn test_tool_message_validation() {
        // Valid tool message
        let call_id = ArcStr::from("call_id");
        let valid_tool = RequestMessage::new_tool("content".to_string(), call_id.clone());
        assert!(valid_tool.validate().is_ok());

        // Invalid tool message (missing tool_call_id)
        let invalid_tool = RequestMessage {
            role: Role::Tool,
            content: "content".to_string(),
            tool_call_id: None,
        };
        assert!(invalid_tool.validate().is_err());
        assert!(
            invalid_tool
                .validate()
                .unwrap_err()
                .contains("tool_call_id")
        );
    }

    #[test]
    fn test_tool_message_serialization() {
        let call_id = ArcStr::from("call_abc");
        let tool_msg = RequestMessage::new_tool("test result".to_string(), call_id);
        let serialized = serde_json::to_string(&tool_msg).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["role"], "tool");
        assert_eq!(parsed["content"], "test result");
        assert_eq!(parsed["tool_call_id"], "call_abc");
    }

    #[test]
    fn test_cap_messages_by_chars_keeps_latest_even_if_over_budget() {
        let m1 = RequestMessage::new_user("a".into()); // 1
        let m2 = RequestMessage::new_user("bb".into()); // 2
        let m3 = RequestMessage::new_user("ccc".into()); // 3
        let m4 = RequestMessage::new_user("dddd".into()); // 4 (tail)
        let all = vec![m1, m2, m3, m4.clone()];

        // Budget smaller than last message; policy keeps at least the most recent
        let kept = cap_messages_by_chars(&all, 3);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].content, m4.content);

        // Budget that fits last two (3 + 4 > 6), but logic walks tail-first and stops when over
        let kept2 = cap_messages_by_chars(&all, 7);
        // Tail first (4), then preceding (3) fits → 4 + 3 = 7
        assert_eq!(kept2.len(), 2);
        assert_eq!(kept2[0].content, "ccc");
        assert_eq!(kept2[1].content, "dddd");
    }

    #[test]
    fn test_cap_messages_by_tokens_behaves_reasonably() {
        // We cannot assert exact token counts without knowing tokenizer internals,
        // but we can validate ordering and non-empty behavior.
        let m1 = RequestMessage::new_user("short".into());
        let m2 = RequestMessage::new_user("a bit longer".into());
        let m3 = RequestMessage::new_user("the longest content in this small set".into());
        let all = vec![m1.clone(), m2.clone(), m3.clone()];

        // With a tiny budget, we still keep at least the latest
        let kept = cap_messages_by_tokens(&all, 1);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].content, m3.content);

        // With a generous budget, we keep all in original order
        let kept_all = cap_messages_by_tokens(&all, 10_000);
        assert_eq!(kept_all.len(), 3);
        assert_eq!(kept_all[0].content, m1.content);
        assert_eq!(kept_all[1].content, m2.content);
        assert_eq!(kept_all[2].content, m3.content);
    }
}
