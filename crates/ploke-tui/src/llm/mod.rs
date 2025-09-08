pub mod openrouter;
pub mod registry;
pub mod session;

use itertools::Itertools;
pub use openrouter::*;

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

pub(crate) use self::providers::ProviderSlug;

use super::*;

#[derive(Serialize, Debug, Clone, Default, Deserialize)]
pub struct ProviderPreferences {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub allow: Option<Vec<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub deny: Option<Vec<ProviderSlug>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub order: Option<Vec<ProviderSlug>>,
    /// Gate to enforce provider-side parameter availability (per docs)
    pub require_parameters: bool,
}

impl ProviderPreferences {
    /// Convenience: build an "allow-only" preference list.
    pub fn allow<I>(slugs: I) -> Self
    where
        I: IntoIterator<Item = ProviderSlug>,
    {
        Self {
            allow: Some( slugs.into_iter().collect() ),
            deny: None,
            order: None,
            require_parameters: true,
        }
    }

    /// Add an ordering preference.
    pub fn with_order(mut self, ordered: Vec<ProviderSlug>) -> Self {
        self.order = Some(ordered);
        self
    }

    /// Add a deny list.
    pub fn with_deny<I: IntoIterator<Item = ProviderSlug>>(mut self, deny: I) -> Self {
        self.deny = Some( deny.into_iter().collect() );
        self
    }
}

// Lightweight tool to fetch current file metadata (tracking hash and basics)
#[deprecated = "use tools::GetFileMetadata::tool_def() instead"]
pub fn get_file_metadata_tool_def() -> crate::tools::ToolDefinition {
    use crate::tools::{FunctionMarker, ToolDefinition, ToolDescr, ToolFunctionDef, ToolName};
    ToolDefinition {
        r#type: FunctionMarker,
        function: ToolFunctionDef {
            name: ToolName::GetFileMetadata,
            description: ToolDescr::GetFileMetadata,
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path to the target file."
                    }
                },
                "required": ["file_path"],
                "additionalProperties": false
            }),
        },
    }
}

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
            AppEvent::Llm(
                request @ Event::Request {
                    parent_id,
                    new_msg_id,
                    ..
                },
            ) => {
                tracing::info!(
                    "Received LLM request for parent_id: {}\nnew_msg_id: {}",
                    parent_id,
                    new_msg_id
                );

                // pairing happens when PromptConstructed arrives for the same parent_id.
                // NOTE: May want to optimize by adding an array buffer for in-memory handling
                pending_requests.push(request);
            }
            AppEvent::Llm(context @ Event::PromptConstructed { parent_id, .. }) => {
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
                    if let Event::Request {
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
                                req.clone(),
                                Arc::clone(&state),
                                cmd_tx.clone(),
                                client.clone(),
                                event_bus.clone(),
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
            AppEvent::ModelsEndpointsRequest { model_id } => {
                let state = Arc::clone(&state);
                let event_bus = Arc::clone(&event_bus);
                let client = client.clone();
                tokio::spawn(async move {
                    // Resolve an OpenRouter API key
                    let api_key = {
                        let cfg = state.config.read().await;
                        cfg.model_registry
                            .providers
                            .iter()
                            .find(|p| {
                                matches!(
                                    p.provider_type,
                                    crate::user_config::ProviderType::OpenRouter
                                )
                            })
                            .map(|p| p.resolve_api_key())
                            .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                            .unwrap_or_default()
                    };
                    if api_key.is_empty() {
                        tracing::warn!(
                            "No OpenRouter API key available; cannot fetch endpoints for {}",
                            model_id
                        );
                        // Unblock the UI: return an empty provider list so the overlay stops loading.
                        event_bus.send(AppEvent::ModelsEndpointsResults {
                            model_id,
                            providers: Vec::new(),
                        });
                        return;
                    }

                    match openrouter_catalog::fetch_model_endpoints(
                        &client,
                        openrouter_url(),
                        &api_key,
                        &model_id,
                    )
                    .await
                    {
                        Ok(providers) => {
                            let provider_summaries = providers
                                .endpoints
                                .iter()
                                .map(ProviderSummary::from_endpoint)
                                .collect_vec();
                            event_bus.send(AppEvent::ModelsEndpointsResults {
                                model_id,
                                providers: provider_summaries,
                            });
                        }
                        Err(e) => {
                            tracing::warn!("Failed to fetch endpoints for {}: {}", model_id, e);
                            // Unblock the UI even on error with an empty provider list.
                            event_bus.send(AppEvent::ModelsEndpointsResults {
                                model_id,
                                // TODO: Change to none and update UI to handle None case
                                providers: Vec::new(),
                            });
                        }
                    }
                });
            }
            _ => {}
        }
    }
}

// The worker function that processes a single LLM request.
// TODO: Add proper error handling if the `CreateAssistantMessage` fails
#[instrument(skip_all,
    fields(
        model = %provider_config.model
    )
)]
pub async fn process_llm_request(
    request: Event,
    state: Arc<AppState>,
    cmd_tx: mpsc::Sender<StateCommand>,
    client: Client,
    event_bus: Arc<EventBus>,
    provider_config: crate::user_config::ModelConfig,
    context: Option<Event>,
) {
    let parent_id = match request {
        Event::Request {
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

#[instrument(skip_all,
    fields(
        provider_params = ?provider.llm_params
    )
)]
async fn prepare_and_run_llm_call(
    state: &Arc<AppState>,
    client: &Client,
    provider: &ModelConfig,
    context: Option<Event>,
    event_bus: Arc<EventBus>,
    parent_id: Uuid,
) -> Result<String, LlmError> {
    tracing::info!(model = %provider.model, has_context = %context.is_some(), "prepare_and_run_llm_call start");
    // Get the conversation history from AppState
    let history_guard = state.chat.0.read().await;
    let path = history_guard.get_current_path();

    let context_path = if path.len() > 1 {
        &path[..path.len() - 1]
    } else {
        &path[..]
    };

    let mut messages: Vec<RequestMessage> = Vec::new();

    // Get parameters from provider
    let params = match provider.llm_params.as_ref().cloned() {
        Some(p) => p,
        None => {
            tracing::warn!("LLMParameters falling back to defaults.");
            LLMParameters::default()
        }
    };

    // Defaults for tool handling
    let max_retries: u32 = params.tool_max_retries.unwrap_or(crate::TOOL_RETRIES);
    let token_limit: u32 = params.tool_token_limit.unwrap_or(crate::TOKEN_LIMIT);

    // Prepend system prompt if provided
    // NOTE: I don't think we are actually using this anywhere, and I'm not sure I like this here.
    if let Some(sys) = params.system_prompt.as_ref() {
        messages.push(RequestMessage::new_system(sys.clone()));
    }

    // Append the rest of the conversation
    let conversation_messages = if let Some(Event::PromptConstructed { prompt, .. }) = context {
        prompt
            .into_iter()
            .map(|(k, c)| RequestMessage {
                role: k.into(),
                content: c,
                tool_call_id: None,
            })
            .collect::<Vec<_>>()
    } else {
        context_path
            .iter()
            .filter(|msg| (msg.kind != MessageKind::SysInfo) && !msg.content.is_empty())
            .map(|msg| RequestMessage {
                role: msg.kind.into(),
                content: msg.content.clone(),
                tool_call_id: None,
            })
            .collect::<Vec<_>>()
    };

    messages.extend(conversation_messages);

    tracing::trace!(
        "Sending conversation history message with content: {:#?}",
        messages
    );
    // Release the lock before the network call
    drop(history_guard);

    // Decide tool usage based on registry capabilities and enforcement policy
    // NOTE: Need to experiment more with this. Not sure if it is working correctly to distinguish
    // models that support tools, and then to use the right kind of API call to OpenRouter to
    // ensure the call is being routed to only those providers that support tool use.
    let (supports_tools_opt, require_tools) = {
        let cfg = state.config.read().await;
        (
            cfg.model_registry.model_supports_tools(&provider.model),
            cfg.model_registry.require_tool_support,
        )
    };

    // Concise plan log: shows what we think about tool support and enforcement
    tracing::debug!(
        model = %provider.model,
        base_url = %provider.base_url,
        provider_type = ?provider.provider_type,
        provider_slug = ?provider.provider_slug,
        supports_tools_cache = ?supports_tools_opt,
        require_tool_support = require_tools,
        "llm_request_plan"
    );

    if require_tools && (supports_tools_opt != Some(true)) {
        // Persist a concise decision record explaining why the call is aborted
        if let Some(fut) = log_tool_use(LogToolUseCtx {
            provider,
            parent_id,
            supports_tools_opt,
            require_tools,
        }) {
            fut.await.ok();
        };

        return Err(LlmError::Api {
            status: 412,
            message: format!(
                "Active model '{}' is not marked as tool-capable in the capabilities cache. \
Run ':model refresh' to update the registry, select a tool-capable provider via the model browser, \
or disable enforcement with ':provider tools-only off'.",
                provider.model
            ),
        });
    }

    let tools: Vec<ToolDefinition> = if supports_tools_opt.unwrap_or(false) {
        vec![
            RequestCodeContextGat::tool_def(),
            GatCodeEdit::tool_def(),
            GetFileMetadata::tool_def(),
        ]
    } else {
        Vec::new()
    };

    // Summarize tools we will include for this request
    let tool_names: Vec<&str> = tools.iter().map(|t| t.function.name.as_str()).collect();
    tracing::debug!(
        "llm_request_tools:\n\tuse_tools: {}\n\ttool_names: {}",
        !tools.is_empty(),
        tool_names.join(","),
    );

    // Persist a diagnostic snapshot of the outgoing "request plan" for offline analysis.
    let log_fut = log_tool_use(LogToolUseCtx {
        provider,
        parent_id,
        supports_tools_opt,
        require_tools,
    });

    // Delegate the per-request loop to RequestSession (Milestone 2 extraction)
    let session = session::RequestSession {
        client,
        provider,
        event_bus,
        parent_id,
        messages,
        tools,
        params,
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

#[derive(Clone, Debug)]
pub enum Event {
    /// Request to generate content from an LLM
    Request {
        request_id: Uuid,          // Unique tracking ID
        parent_id: Uuid,           // Message this responds to
        prompt: String,            // Input to LLM
        parameters: LLMParameters, // Generation settings
        new_msg_id: Uuid, // callback: Option<Sender<Event>>, // Optional direct response channel
    },

    /// Successful LLM response
    Response {
        request_id: Uuid, // Matches Request ID
        parent_id: Uuid,
        content: String, // Generated content
        model: String,   // e.g., "gpt-4-turbo"
        metadata: LLMMetadata,
        usage: UsageMetrics, // Tokens/timing
    },

    /// Partial response (streaming)
    PartialResponse {
        request_id: Uuid,
        delta: String, // Text chunk
    },

    /// Error during processing
    Error {
        request_id: Uuid,
        error: LlmError, // Structured error type
    },

    /// Status update
    Status {
        active_requests: usize, // Current workload
        queue_depth: usize,     // Pending requests
    },

    /// Configuration change
    ModelChanged {
        new_model: String, // e.g., "claude-3-opus"
    },

    /// Tool/function call emitted by model (OpenAI tools or other)
    ToolCall {
        request_id: Uuid,
        parent_id: Uuid,
        name: ToolName,
        arguments: Value,
        // TODO: Change to Option<ArcStr> and propogate through tool returns
        call_id: Option<String>,
    },

    /// Prompt constructed to be sent to the LLM
    PromptConstructed {
        prompt: Vec<(MessageKind, String)>,
        parent_id: Uuid,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct UsageMetrics {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub latency_ms: u64,
}

impl Event {
    pub fn parent_id(&self) -> Uuid {
        match self {
            Event::Request { parent_id, .. } => *parent_id,
            Event::Response { parent_id, .. } => *parent_id,
            Event::PartialResponse { .. } => todo!(),
            Event::Error { .. } => todo!(),
            Event::Status { .. } => todo!(),
            Event::ModelChanged { .. } => todo!(),
            Event::ToolCall { parent_id, .. } => *parent_id,
            Event::PromptConstructed { parent_id, .. } => *parent_id,
        }
    }
}

/// Represents errors that can occur during LLM interactions.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum LlmError {
    /// Error related to network connectivity or the HTTP request itself.
    #[error("Network request failed: {0}")]
    Request(String),

    /// The API provider returned a non-success status code.
    #[error("API error (status {status}): {message}")]
    Api { status: u16, message: String },

    /// The request was rejected due to rate limiting.
    #[error("Rate limit exceeded. Please wait and try again.")]
    RateLimited,

    /// The request failed due to invalid credentials.
    #[error("Authentication failed. Please check your API key.")]
    Authentication,

    /// The request timed out.
    #[error("The request to the LLM provider timed out.")]
    Timeout,

    /// The response from the LLM was blocked due to content safety filters.
    #[error("Response blocked by content safety filter.")]
    ContentFilter,

    /// Failed to serialize the request payload.
    #[error("Failed to serialize request data: {0}")]
    Serialization(String),

    /// Failed to deserialize the API response.
    #[error("Failed to deserialize response data: {0}")]
    Deserialization(String),

    /// Failed to deserialize the API response.
    #[error("Failed to deserialize response data: {0}")]
    ToolCall(String),

    /// An unexpected or unknown error occurred.
    #[error("An unknown error occurred: {0}")]
    Unknown(String),
}

impl From<LlmError> for ploke_error::Error {
    fn from(error: LlmError) -> Self {
        match error {
            LlmError::Request(msg) => ploke_error::Error::Internal(
                ploke_error::InternalError::EmbedderError(std::sync::Arc::new(
                    std::io::Error::new(std::io::ErrorKind::ConnectionAborted, msg),
                )),
            ),
            LlmError::Api { status, message } => {
                ploke_error::Error::Internal(ploke_error::InternalError::EmbedderError(
                    std::sync::Arc::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("API error {}: {}", status, message),
                    )),
                ))
            }
            LlmError::RateLimited => ploke_error::Error::Warning(
                ploke_error::WarningError::PlokeDb("Rate limit exceeded".to_string()),
            ),
            LlmError::Authentication => {
                ploke_error::Error::Fatal(ploke_error::FatalError::PathResolution {
                    path: "Authentication failed - check API key".to_string(),
                    source: None,
                })
            }
            LlmError::Timeout => ploke_error::Error::Internal(
                ploke_error::InternalError::EmbedderError(std::sync::Arc::new(
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "Request timed out"),
                )),
            ),
            LlmError::ContentFilter => ploke_error::Error::Warning(
                ploke_error::WarningError::PlokeDb("Content blocked by safety filter".to_string()),
            ),
            LlmError::Serialization(msg) => ploke_error::Error::Internal(
                ploke_error::InternalError::CompilerError(format!("Serialization error: {}", msg)),
            ),
            LlmError::Deserialization(msg) => {
                ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(format!(
                    "Deserialization error: {}",
                    msg
                )))
            }
            LlmError::ToolCall(msg) => ploke_error::Error::Internal(
                ploke_error::InternalError::NotImplemented(format!("Tool Call error: {}", msg)),
            ),
            LlmError::Unknown(msg) => ploke_error::Error::Internal(
                ploke_error::InternalError::NotImplemented(format!("Unknown error: {}", msg)),
            ),
        }
    }
}

use crate::user_config::{default_model, openrouter_url};
/// Parameters for controlling LLM generation behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMParameters {
    /// LLM model identifier (e.g., "gpt-4-turbo", "claude-3-opus")
    #[serde(default = "default_model")]
    pub model: String,

    /// Sampling temperature (None = provider default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p nucleus sampling threshold (None = provider default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Maximum tokens to generate (None = provider default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Presence penalty (-2.0 to 2.0)
    #[serde(default)]
    pub presence_penalty: Option<f32>,

    /// Frequency penalty (-2.0 to 2.0)
    #[serde(default)]
    pub frequency_penalty: Option<f32>,

    /// Stop sequences to halt generation
    #[serde(default)]
    pub stop_sequences: Vec<String>,

    /// Enable parallel function calling
    #[serde(default = "default_true")]
    pub parallel_tool_calls: bool,

    /// JSON mode enforcement
    #[serde(default)]
    pub response_format: ResponseFormat,

    /// Safety/system controls
    #[serde(default)]
    pub safety_settings: SafetySettings,

    /// Optional system prompt to steer the model
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// Maximum number of tool-call retry cycles
    #[serde(default)]
    pub tool_max_retries: Option<u32>,

    /// Token limit for tool-provided context (sane default)
    #[serde(default)]
    pub tool_token_limit: Option<u32>,

    /// Optional character budget for conversation history before each request
    #[serde(default)]
    pub history_char_budget: Option<usize>,

    /// Per-tool call timeout in seconds
    #[serde(default)]
    pub tool_timeout_secs: Option<u64>,
}

/// Metadata about LLM execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMMetadata {
    /// Actual model used (may differ from request)
    pub model: String,

    /// Token usage metrics
    pub usage: TokenUsage,

    /// Generation completion reason
    pub finish_reason: FinishReason,

    /// Time spent in LLM processing
    pub processing_time: Duration,

    /// Content safety scores
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_scores: Option<SafetyScores>,

    /// Cost calculation in USD
    pub cost: f64,

    /// Performance metrics
    pub performance: PerformanceMetrics,
}

// --- Supporting Types ---

/// Response format specification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ResponseFormat {
    #[default]
    Text,
    JsonObject,
}

/// Safety control settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SafetySettings {
    pub hate: u8, // 0-8 (0=disable, 4=moderate, 7=strict)
    pub harassment: u8,
    pub self_harm: u8,
    #[serde(rename = "sexual")]
    pub sexual_content: u8,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Generation completion reasons
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinishReason {
    Stop,          // Natural stop sequence
    Length,        // Max tokens reached
    ContentFilter, // Blocked by safety system
    ToolCalls,     // Stopped for tool execution
    Timeout,       // Processing time exceeded
    Error(String), // Error description
}

/// Content safety assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyScores {
    pub hate: f32, // 0-1 probability score
    pub harassment: f32,
    pub self_harm: f32,
    pub sexual_content: f32,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub tokens_per_second: f32,
    pub time_to_first_token: Duration,
    pub queue_time: Duration,
}

// --- Default Implementations ---
impl Default for LLMParameters {
    fn default() -> Self {
        Self {
            model: default_model(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            stop_sequences: vec![],
            parallel_tool_calls: true,
            response_format: Default::default(),
            safety_settings: Default::default(),
            system_prompt: None,
            tool_max_retries: Some(TOOL_RETRIES),
            tool_token_limit: Some(TOKEN_LIMIT),
            history_char_budget: None,
            tool_timeout_secs: Some(LLM_TIMEOUT_SECS),
        }
    }
}

fn default_true() -> bool {
    true
}

fn skip_bool_always(_: &bool) -> bool {
    true
}

// Test-only typed response summary to validate live parsing against our internal DTOs.
#[cfg(feature = "test_harness")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseParseSummary {
    pub choices: usize,
    pub tool_calls_total: usize,
    pub has_message: bool,
    pub has_content: bool,
}

/// Attempt to parse a provider response body into our OpenAI-compatible types and summarize.
#[cfg(feature = "test_harness")]
pub fn test_parse_response_summary(body: &str) -> Result<ResponseParseSummary, String> {
    let parsed: OpenAiResponse =
        serde_json::from_str(body).map_err(|e| format!("typed parse failed: {}", e))?;
    let mut choices_cnt = 0usize;
    let mut tool_calls_total = 0usize;
    let mut has_message = false;
    let mut has_content = false;
    for ch in parsed.choices.iter() {
        choices_cnt += 1;

        // Check for message-based response
        if let Some(message) = &ch.message {
            has_message = true;
            if message
                .content
                .as_ref()
                .map(|s| !s.is_empty())
                .unwrap_or(false)
            {
                has_content = true;
            }
            if let Some(tcs) = &message.tool_calls {
                tool_calls_total += tcs.len();
            }
        }

        // Check for text-based response
        if let Some(text) = &ch.text {
            if !text.is_empty() {
                has_content = true;
            }
        }

        // Streaming delta responses would be handled here if needed
    }
    Ok(ResponseParseSummary {
        choices: choices_cnt,
        tool_calls_total,
        has_message,
        has_content,
    })
}

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
