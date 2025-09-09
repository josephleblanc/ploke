mod commands;
mod events;
mod session;

use crate::HashMap;
use crate::SystemEvent;
use crate::app_state::handlers::chat::add_msg_immediate;
use crate::error::ResultExt as _;
use crate::llm2::error::LlmError;
use crate::llm2::router_only::HasEndpoint;
use crate::llm2::router_only::HasModels;
use crate::tools;
use events::ChatEvt;
pub(crate) use events::LlmEvent;
use events::endpoint;
use events::models;
use itertools::Itertools;
use ploke_core::ArcStr;

use ploke_rag::TokenCounter as _;
use ploke_rag::context::ApproxCharTokenizer;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{env, fs, path::PathBuf, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::instrument;
use uuid::Uuid;

use crate::app_state::{AppState, StateCommand};
use crate::rag::utils::ToolCallParams;
use crate::tools::code_edit::GatCodeEdit;
use crate::tools::request_code_context::RequestCodeContextGat;
use crate::tools::{
    FunctionCall, FunctionMarker, GetFileMetadata, RequestCodeContext, Tool as _, ToolCall,
    ToolDefinition, ToolFunctionDef, ToolName,
};
use crate::utils::consts::DEBUG_TOOLS;
use crate::{AppEvent, EventBus};
use crate::{
    chat_history::{Message, MessageKind, MessageStatus, MessageUpdate},
    user_config::ModelConfig,
};

// API and Config

use super::response::TokenUsage;
use super::router_only::openrouter::OpenRouter;
use super::router_only::openrouter::OpenRouterModelId;
use super::*;

#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct RequestMessage {
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

#[derive(Deserialize, Debug)]
pub struct OpenAiResponse {
    #[serde(default)]
    id: String,
    #[serde(default)]
    choices: Vec<Choices>,
    #[serde(default)]
    created: i64,
    #[serde(default)]
    model: String,
    #[serde(default)]
    object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logprobs: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialOrd, PartialEq)]
pub(super) enum ResponseObject {
    #[serde(rename = "chat.completion")]
    ChatCompletion,
    #[serde(rename = "chat.completion.chunk")]
    ChatCompletionChunk,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialOrd, PartialEq)]
pub(super) struct ResponseUsage {
    /** Including images and tools if any */
    prompt_tokens: i64,
    /** The tokens generated */
    completion_tokens: i64,
    /** Sum of the above two fields */
    total_tokens: i64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Choices {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<ResponseMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
    // For non-streaming choices that might have text instead of message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    // For streaming choices
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<StreamingDelta>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct StreamingDelta {
    // May be null or string
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    // May or may not be present
    role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    // May or may not be present
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ErrorResponse {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    // Contains additional error information such as provider details, the raw error message, etc.
    // Original is Record<string, unknown>
    metadata: Option<HashMap<String, serde_json::Value>>,
}

// Use OpenAI-style normalized tool call shape per OpenRouter docs

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(super) struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ResponseMessage {
    // When tool_calls are present, role may be null/absent
    role: Option<String>,
    // When tool_calls are present, content may be null/absent
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logprobs: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refusal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<String>,
}

// Removed legacy OpenAiToolCall and OpenAiToolFunctionCall in favor of tools::ToolCall<'a>

pub async fn llm_manager(
    mut event_rx: broadcast::Receiver<AppEvent>,
    state: Arc<AppState>,
    cmd_tx: mpsc::Sender<StateCommand>,
    event_bus: Arc<EventBus>,
    // providers: crate::user_config::ModelRegistry,
) {
    let client = Client::new();
    let mut pending_requests = Vec::new();
    let mut ready_contexts = std::collections::HashMap::new();

    // Enters loop every time there is a new event.
    //  - Currently only receives:
    //      - AppEvent::Llm(Event::Request)
    //      - AppEvent::Llm(Event::PromptConstructed)
    while let Ok(event) = event_rx.recv().await {
        match event {
            AppEvent::Llm2(LlmEvent::ChatCompletion(
                request @ ChatEvt::Request {
                    parent_id,
                    new_msg_id,
                    ..
                },
            )) => {
                tracing::info!(
                    "Received LLM request for parent_id: {}\nnew_msg_id: {}",
                    parent_id,
                    new_msg_id
                );

                // pairing happens when PromptConstructed arrives for the same parent_id.
                // NOTE: May want to optimize by adding an array buffer for in-memory handling
                pending_requests.push(request);
            }
            AppEvent::Llm2(LlmEvent::ChatCompletion(
                context @ ChatEvt::PromptConstructed { parent_id, .. },
            )) => {
                tracing::info!("Received context for parent_id: {}", parent_id);
                ready_contexts.insert(parent_id, context);

                let guard = state.config.read().await;
                let maybe_provider = guard.model_registry.get_active_model_config();
                if maybe_provider.is_none() {
                    // TODO: Trigger update of registered models here
                    tracing::warn!(
                        "Could not find active provider in registry, continuing event loop"
                    );
                    continue;
                }
                let provider_config = maybe_provider
                    .expect("Error in unwrapping a value guarenteed to be Some")
                    .clone();
                drop(guard);
                // Process any pending requests that now have context
                pending_requests.retain(|req| {
                    if let ChatEvt::Request {
                        parent_id: req_parent,
                        ..
                    } = req.clone()
                    {
                        tracing::info!(
                            "pending_requests found match for req_parent: {}",
                            req_parent
                        );
                        if let Some(context) = ready_contexts.remove(&req_parent) {
                            tracing::info!(
                                "ready_contexts found match for req_parent: {}",
                                req_parent
                            );
                            // Note that using `retain` here removes only those messages which have
                            // been sent to be processed. 
                            //  - possible improvement might be to keep req if it is not correctly
                            //      processed by `process_llm_request`?
                            //  - may want to spawn with a timeout or something to prevent leaks(?)
                            tokio::spawn(process_llm_request(
                                *req,
                                Arc::clone(&state),
                                cmd_tx.clone(),
                                client.clone(),
                                event_bus.clone(),
                                #[cfg(not(feature = "llm_refactor"))]
                                provider_config.to_owned(),
                                Some(context),
                            ));
                            tracing::info!("removing id from pending_requests");
                            false // Remove from pending
                        } else {
                            tracing::info!(
                                "keep id from pending_requests
                                found pending_request but not ready_context
                                checking if ready_contexts contains req_parent during conditional: {}",
                                ready_contexts.contains_key(&req_parent)
                            );
                            true // Keep waiting
                        }
                    } else {
                        tracing::info!("keep id from pending_requests\nno matched pending_requests");
                        true
                    }
                });
            }
            AppEvent::System(SystemEvent::ToolCallRequested {
                tool_call,
                request_id,
                parent_id,
                // name,
                // arguments,
                // call_id,
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
            AppEvent::Llm2(LlmEvent::Endpoint(endpoint::Event::Request {
                parent_id,
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
                    parent_id,
                );
            }
            AppEvent::Llm2(LlmEvent::Models(models::Event::Request { parent_id, router })) => {
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
                        .map(|m| Arc::new(m))
                        .inspect_err(|e| {
                            let msg = format!("Failed to fetch models from API: {}", e);
                            tracing::warn!(msg);
                            // Unblock the UI even on error with an empty provider list.
                            event_bus.send(AppEvent::Llm2(LlmEvent::Models(Event::Response {
                                parent_id,
                                models: None,
                            })));
                        });
                    event_bus.send(AppEvent::Llm2(LlmEvent::Models(Event::Response {
                        parent_id,
                        models: result.ok(),
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
    parent_id: Uuid,
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
            .map(|ep| Arc::new(ep))
            .inspect_err(|e| {
                let msg = format!("Failed to fetch endpoints for {}: {:?}", typed_model, e);
                tracing::warn!(msg);
                // TODO: send a response with an error
            })
            .ok();
        event_bus.send(AppEvent::Llm2(LlmEvent::Endpoint(Event::Response {
            parent_id,
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
    #[cfg(not(feature = "llm_refactor"))] provider_config: crate::user_config::ModelConfig,
    context: Option<ChatEvt>,
) {
    let parent_id = match request {
        ChatEvt::Request {
            parent_id,
            new_msg_id: _,
            ..
        } => parent_id,
        _ => {
            tracing::debug!("Not a Request, do nothing");
            return;
        } // Not a request, do nothing
    };

    // This part remains the same: create a placeholder message first.
    let (responder_tx, responder_rx) = oneshot::channel();
    let create_cmd = StateCommand::CreateAssistantMessage {
        parent_id,
        responder: responder_tx,
    };

    // WARN: Trying out `.expect()` here
    // because I *think* this represents and invalid state. There should never be a case in which
    // the command cannot be created.
    //  - May want to add a `shutdown` command or something, as it is possible that the shutdown
    //  command in the main app could run while we are waiting for a returned value, blah blah
    // if cmd_tx.send(create_cmd).await.is_err() {
    //     log::error!("Failed to send CreateAssistantMessage command: channel closed.");
    //     return;
    // }
    cmd_tx.send(create_cmd).await.expect(
        "Invalid state: sending over closed channel from process_llm_request via StateCommand",
    );

    // TODO: There must be a cleaner way to handle this than using the one-shot for a callback. Try
    // to find a better way.
    // WARN: Trying out `.expect()` here
    let assistant_message_id = responder_rx.await.expect(
        "Invalid state: Failed to create assistant message, state_manager dropped responder.",
    );
    // let assistant_message_id = match responder_rx.await {
    //     Ok(id) => id,
    //     Err(_) => {
    //         log::error!("Failed to create assistant message: state_manager dropped responder.");
    //         return;
    //     }
    // };

    // Prepare and execute the API call, then create the final update command.
    let result = prepare_and_run_llm_call(
        &state,
        &client,
        #[cfg(not(feature = "llm_refactor"))]
        &provider_config,
        context,
        event_bus.clone(),
        parent_id,
    )
    .await;

    // Build concise per-request outcome summary before consuming `result`
    let summary = match &result {
        Ok(_) => "Request summary: success".to_string(),
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
                assistant_message_id,
                preview
            );

            StateCommand::UpdateMessage {
                id: assistant_message_id,
                update: MessageUpdate {
                    content: Some(content),
                    status: Some(MessageStatus::Completed),
                    ..Default::default()
                },
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
            StateCommand::UpdateMessage {
                id: assistant_message_id,
                update: MessageUpdate {
                    content: Some(format!("Request failed: {}", err_string)),
                    status: Some(MessageStatus::Completed),
                    ..Default::default()
                },
            }
        }
    };

    // Send the final update command to the state manager.
    if cmd_tx.send(update_cmd).await.is_err() {
        log::error!("Failed to send final UpdateMessage: channel closed.");
    }

    let _ = cmd_tx
        .send(StateCommand::AddMessageImmediate {
            msg: summary,
            kind: MessageKind::SysInfo,
            new_msg_id: Uuid::new_v4(),
        })
        .await;
}

#[instrument(skip_all)]
async fn prepare_and_run_llm_call(
    state: &Arc<AppState>,
    client: &Client,
    #[cfg(not(feature = "llm_refactor"))] _cfg: &ModelConfig,
    context: Option<ChatEvt>,
    event_bus: Arc<EventBus>,
    parent_id: Uuid,
) -> Result<String, LlmError> {
    tracing::info!(has_context = %context.is_some(), "prepare_and_run_llm_call start");
    // Get the conversation history from AppState
    let history_guard = state.chat.0.read().await;
    let path = history_guard.get_current_path();

    let context_path = if path.len() > 1 {
        &path[..path.len() - 1]
    } else {
        &path[..]
    };

    let mut messages: Vec<RequestMessage> = Vec::new();
    drop(history_guard);

    // Get parameters

    // Defaults for tool handling

    // Prepend system prompt if provided

    // Append the rest of the conversation as `RequestMessage`, and map onto Roles

    // Decide tool usage based on registry capabilities and enforcement policy

    // Concise plan log: shows what we think about tool support and enforcement


    let tools: Vec<ToolDefinition> = vec![
            RequestCodeContextGat::tool_def(),
            GatCodeEdit::tool_def(),
            GetFileMetadata::tool_def(),
    ];

    // Persist a diagnostic snapshot of the outgoing "request plan" for offline analysis.
    let log_fut = log_tool_use(LogToolUseCtx {
        provider,
        parent_id,
        supports_tools_opt,
        require_tools,
    });

    // Delegate the per-request loop to RequestSession
    let session = session::RequestSession {
        client,
        event_bus,
        parent_id,
        messages,
        tools,
        fallback_on_404: false,
        attempts: 3,
    };

    let result = session.run().await;

    // Persist model output or error for later inspection
    if let Some(fut) = log_fut {
        if fut.await.is_err() {
            tracing::error!("Failed to write tool use logs.");
        }
    }

    result
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

struct LogToolUseCtx<'a> {
    provider: &'a ModelConfig,
    parent_id: Uuid,
    supports_tools_opt: Option<bool>,
    require_tools: bool,
}

fn log_tool_use(ctx: LogToolUseCtx) -> Option<impl Future<Output = tokio::io::Result<()>>> {
    match diag_dir() {
        Some(dir) => {
            let LogToolUseCtx {
                provider,
                parent_id,
                supports_tools_opt,
                require_tools,
            } = ctx;
            let fname = format!("{}-{}-decision.json", now_ts(), parent_id);
            let record = json!({
                    "phase": "preflight",
                    "reason": "tools_required_but_model_not_marked_tool_capable",
                    "provider": {
                    "model": provider.model,
                    "base_url": provider.base_url,
                    "provider_type": format!("{:?}", provider.provider_type),
                    "provider_slug": provider.provider_slug
                },
                "capabilities": {
                    "supports_tools_cache": supports_tools_opt,
                    "require_tools_policy": require_tools
                }
            });
            Some(tokio::fs::write(
                dir.join(fname),
                serde_json::to_string_pretty(&record).unwrap_or_default(),
            ))
        }
        None => None,
    }
}

fn log_tool_reason(ctx: LogToolUseCtx) -> Option<impl Future<Output = tokio::io::Result<()>>> {
    let LogToolUseCtx {
        provider,
        parent_id,
        supports_tools_opt,
        require_tools,
    } = ctx;
    if let Some(dir) = diag_dir() {
        let fname = format!("{}-{}-decision.json", now_ts(), parent_id);
        let record = json!({
            "phase": "preflight",
            "reason": "tools_required_but_model_not_marked_tool_capable",
            "provider": {
                "model": provider.model,
                "base_url": provider.base_url,
                "provider_type": format!("{:?}", provider.provider_type),
                "provider_slug": provider.provider_slug
            },
            "capabilities": {
                "supports_tools_cache": supports_tools_opt,
                "require_tools_policy": require_tools
            }
        });
        Some(tokio::fs::write(
            dir.join(fname),
            serde_json::to_string_pretty(&record).unwrap_or_default(),
        ))
    } else {
        None
    }
}

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

#[derive(Clone, Debug)]
pub enum ToolEvent {
    Requested {
        request_id: Uuid,
        parent_id: Uuid,
        name: String,
        arguments: Value,
        call_id: ArcStr,
    },
    Completed {
        request_id: Uuid,
        parent_id: Uuid,
        call_id: ArcStr,
        content: String,
    },
    Failed {
        request_id: Uuid,
        parent_id: Uuid,
        call_id: ArcStr,
        error: String,
    },
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
}
