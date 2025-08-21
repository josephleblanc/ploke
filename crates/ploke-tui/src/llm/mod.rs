pub mod registry;
pub mod openrouter_catalog;
mod session;
mod tool_call;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::instrument;
use uuid::Uuid;

use crate::app_state::handlers::rag as rag_handlers;
use crate::app_state::{AppState, StateCommand};
use crate::{AppEvent, EventBus};
use crate::{
    chat_history::{Message, MessageKind, MessageStatus, MessageUpdate},
    system::SystemEvent,
    user_config::ProviderConfig,
};

// API and Config

use super::*;

#[derive(Serialize, Debug)]
pub struct OpenAiRequest<'a> {
    model: &'a str,
    messages: Vec<RequestMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>, // e.g., "auto"
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<ProviderPreferences>,
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct ProviderPreferences {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub allow: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub deny: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub order: Vec<String>,
}

impl<'a> OpenAiRequest<'a> {
    pub fn new(
        model: &'a str,
        messages: Vec<RequestMessage<'a>>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        top_p: Option<f32>,
    ) -> Self {
        Self {
            model,
            messages,
            temperature,
            max_tokens,
            top_p,
            stream: false,
            tools: None,
            tool_choice: None,
            provider: None,
        }
    }
}

// Lightweight tool to fetch current file metadata (tracking hash and basics)
fn get_file_metadata_tool_def() -> ToolDefinition {
    ToolDefinition {
        r#type: "function",
        function: ToolFunctionDef {
            name: "get_file_metadata",
            description: "Fetch current file metadata to obtain the expected_file_hash (tracking hash UUID) for safe edits.",
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

#[derive(Serialize, Debug, Clone)]
pub struct RequestMessage<'a> {
    pub role: &'a str,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl RequestMessage<'_> {
    pub fn new_system(content: String) -> Self {
        Self {
            role: "system",
            content,
            tool_call_id: None,
        }
    }

    pub fn new_tool(content: String, tool_call_id: String) -> Self {
        Self {
            role: "tool",
            content,
            tool_call_id: Some(tool_call_id),
        }
    }
}

// OpenAI tool/function definition (for request payload)
#[derive(Serialize, Debug, Clone)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub r#type: &'static str, // "function"
    pub function: ToolFunctionDef,
}

#[derive(Serialize, Debug, Clone)]
pub struct ToolFunctionDef {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value, // JSON Schema
}

// Helper to define our example tool
fn request_code_context_tool_def() -> ToolDefinition {
    ToolDefinition {
        r#type: "function",
        function: ToolFunctionDef {
            name: "request_code_context",
            description: "Request additional code context from the repository up to a token budget.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "token_budget": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Maximum tokens of code context to return."
                    },
                    "hint": {
                        "type": "string",
                        "description": "Optional hint to guide which code to retrieve."
                    }
                },
                "required": ["token_budget"],
                "additionalProperties": false
            }),
        },
    }
}

// New tool definition for applying code edits atomically via ploke-io
fn apply_code_edit_tool_def() -> ToolDefinition {
    ToolDefinition {
        r#type: "function",
        function: ToolFunctionDef {
            name: "apply_code_edit",
            description: "Apply one or more code edits atomically (tempfile + fsync + rename) using ploke-io. Each edit splices bytes [start_byte, end_byte) with replacement.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "confidence": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "description": "Optional model confidence (0.0–1.0) for approval gating."
                    },
                    "namespace": {
                        "type": "string",
                        "description": "Optional namespace UUID for tracking."
                    },
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" },
                                "expected_file_hash": { "type": "string" },
                                "start_byte": { "type": "integer", "minimum": 0 },
                                "end_byte": { "type": "integer", "minimum": 0 },
                                "replacement": { "type": "string" }
                            },
                            "required": ["file_path", "expected_file_hash", "start_byte", "end_byte", "replacement"],
                            "additionalProperties": false
                        },
                        "minItems": 1
                    }
                },
                "required": ["edits"],
                "additionalProperties": false
            }),
        },
    }
}

#[derive(Deserialize, Debug)]
pub(super) struct OpenAiResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<TokenUsage>,
}

#[derive(Deserialize, Debug)]
pub(super) struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize, Debug)]
pub(super) struct ResponseMessage {
    // When tool_calls are present, content may be null/absent
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<GenericToolCall>>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub(super) enum GenericToolCall {
    OpenAi(OpenAiToolCall),
    Other(Value), // Placeholder for other vendor formats
}

#[derive(Deserialize, Debug)]
pub(super) struct OpenAiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String, // should be "function"
    pub function: OpenAiToolFunctionCall,
}

#[derive(Deserialize, Debug)]
pub(super) struct OpenAiToolFunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

pub async fn llm_manager(
    mut event_rx: broadcast::Receiver<AppEvent>,
    state: Arc<AppState>,
    cmd_tx: mpsc::Sender<StateCommand>,
    event_bus: Arc<EventBus>,
    // providers: crate::user_config::ProviderRegistry,
) {
    let client = Client::new();
    let mut pending_requests = Vec::new();
    let mut ready_contexts = std::collections::HashMap::new();

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
                    "Received LLM request for parent_id: {}
                new_msg_id: {}
                ",
                    parent_id,
                    new_msg_id
                );
                pending_requests.push(request);
            }
            AppEvent::Llm(context @ Event::PromptConstructed { parent_id, .. }) => {
                tracing::info!("Received context for parent_id: {}", parent_id);
                ready_contexts.insert(parent_id, context);

                let guard = state.config.read().await;
                let maybe_provider = guard.provider_registry.get_active_provider();
                if maybe_provider.is_none() {
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
            AppEvent::Llm(Event::ToolCall {
                request_id,
                parent_id,
                name,
                arguments,
                vendor,
                call_id,
            }) => {
                let call_id = call_id.unwrap_or_else(|| "unknown".to_string());
                tracing::info!(
                    "ToolCall event routed: vendor={:?}, request_id={}, parent_id={}, call_id={}, name={}, arguments={}",
                    vendor,
                    request_id,
                    parent_id,
                    call_id,
                    name,
                    arguments
                );
                tracing::warn!(
                    "DEPRECATED: Routing tool calls via SystemEvent::ToolCallRequested; this path will be replaced by dedicated tool events in a future EventBus refactor."
                );
                event_bus.send(AppEvent::System(SystemEvent::ToolCallRequested {
                    request_id,
                    parent_id,
                    vendor,
                    name,
                    arguments,
                    call_id,
                }));
            }
            AppEvent::LlmTool(ToolEvent::Requested {
                request_id,
                parent_id,
                vendor,
                name,
                arguments,
                call_id,
            }) => {
                tracing::info!(
                    request_id = %request_id,
                    parent_id = %parent_id,
                    call_id = %call_id,
                    vendor = ?vendor,
                    tool = %name,
                    "Dispatching ToolEvent::Requested in LLM manager"
                );
                let state = Arc::clone(&state);
                let event_bus = Arc::clone(&event_bus);
                tokio::spawn(async move {
                    rag_handlers::handle_tool_call_requested(
                        &state, &event_bus, request_id, parent_id, vendor, name, arguments, call_id,
                    )
                    .await;
                });
            }
            AppEvent::System(SystemEvent::ToolCallRequested {
                request_id,
                parent_id,
                vendor,
                name,
                arguments,
                call_id,
            }) => {
                tracing::info!(
                    "Dispatching ToolCallRequested in system handler: name={}",
                    name
                );
                tracing::warn!(
                    "DEPRECATED PATH: SystemEvent::ToolCallRequested handling is deprecated and will be refactored into dedicated event types; retained for compatibility."
                );
                let state = Arc::clone(&state);
                let event_bus = Arc::clone(&event_bus);
                tokio::spawn(async move {
                    rag_handlers::handle_tool_call_requested(
                        &state, &event_bus, request_id, parent_id, vendor, name, arguments, call_id,
                    )
                    .await;
                });
            }
            _ => {}
        }
    }
}

// The worker function that processes a single LLM request.
// TODO: Add proper error handling if the `CreateAssistantMessage` fails
#[instrument(skip(state, client, provider_config))]
pub async fn process_llm_request(
    request: Event,
    state: Arc<AppState>,
    cmd_tx: mpsc::Sender<StateCommand>,
    client: Client,
    event_bus: Arc<EventBus>,
    provider_config: crate::user_config::ProviderConfig,
    context: Option<Event>,
) {
    tracing::info!("Inside process_llm_request");
    let parent_id = match request {
        Event::Request {
            parent_id,
            new_msg_id: _,
            ..
        } => parent_id,
        _ => {
            tracing::info!("Not a Request, do nothing");
            return;
        } // Not a request, do nothing
    };
    tracing::info!("Inside process_llm_request");

    // This part remains the same: create a placeholder message first.
    let (responder_tx, responder_rx) = oneshot::channel();
    let create_cmd = StateCommand::CreateAssistantMessage {
        parent_id,
        responder: responder_tx,
    };

    if cmd_tx.send(create_cmd).await.is_err() {
        log::error!("Failed to send CreateAssistantMessage command: channel closed.");
        return;
    }

    let assistant_message_id = match responder_rx.await {
        Ok(id) => id,
        Err(_) => {
            log::error!("Failed to create assistant message: state_manager dropped responder.");
            return;
        }
    };

    // Prepare and execute the API call, then create the final update command.
    let update_cmd = match prepare_and_run_llm_call(
        &state,
        &client,
        &provider_config,
        context,
        &event_bus,
        parent_id,
    )
    .await
    {
        Ok(content) => StateCommand::UpdateMessage {
            id: assistant_message_id,
            update: MessageUpdate {
                content: Some(content),
                status: Some(MessageStatus::Completed),
                ..Default::default()
            },
        },
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
}

#[instrument(skip(provider, state, client))]
async fn prepare_and_run_llm_call(
    state: &Arc<AppState>,
    client: &Client,
    provider: &ProviderConfig,
    context: Option<Event>,
    event_bus: &Arc<EventBus>,
    parent_id: Uuid,
) -> Result<String, LlmError> {
    // Get the conversation history from AppState
    let history_guard = state.chat.0.read().await;
    let path = history_guard.get_current_path();

    let context_path = if path.len() > 1 {
        &path[..path.len() - 1]
    } else {
        &path[..]
    };
    tracing::info!("Inside prepare_and_run_llm_call");

    let mut messages: Vec<RequestMessage> = Vec::new();

    // Get parameters from provider
    tracing::info!(
        "Inside prepare_and_run_llm_call num2 {:#?}",
        provider.llm_params
    );
    let params = provider.llm_params.as_ref().cloned().unwrap_or_default();

    // Defaults for tool handling
    let max_retries: u32 = params.tool_max_retries.unwrap_or(2);
    let token_limit: u32 = params.tool_token_limit.unwrap_or(2048);

    // Prepend system prompt if provided
    if let Some(sys) = params.system_prompt.as_ref() {
        messages.push(RequestMessage::new_system(sys.clone()));
    }

    // Append the rest of the conversation
    let conversation_messages = if let Some(Event::PromptConstructed { prompt, .. }) = context {
        prompt
            .into_iter()
            .map(|(k, c)| RequestMessage {
                role: match k {
                    MessageKind::User => "user",
                    MessageKind::Assistant => "assistant",
                    MessageKind::System => "system",
                    MessageKind::Tool => "tool",
                    MessageKind::SysInfo => "system",
                },
                content: c,
                tool_call_id: None,
            })
            .collect::<Vec<_>>()
    } else {
        context_path
            .iter()
            .filter(|msg| (msg.kind != MessageKind::SysInfo) && !msg.content.is_empty())
            .map(|msg| RequestMessage {
                role: match msg.kind {
                    MessageKind::User => "user",
                    MessageKind::Assistant => "assistant",
                    MessageKind::System => "system",
                    MessageKind::Tool => "tool",
                    MessageKind::SysInfo => "system",
                },
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
    log::info!("Sending request using model config: {:#?}", provider);
    // Release the lock before the network call
    drop(history_guard);

    // Decide tool usage based on registry capabilities and enforcement policy
    let (supports_tools_opt, require_tools) = {
        let cfg = state.config.read().await;
        (
            cfg.provider_registry.model_supports_tools(&provider.model),
            cfg.provider_registry.require_tool_support,
        )
    };

    // Concise plan log: shows what we think about tool support and enforcement
    tracing::info!(
        model = %provider.model,
        base_url = %provider.base_url,
        provider_type = ?provider.provider_type,
        provider_slug = ?provider.provider_slug,
        supports_tools_cache = ?supports_tools_opt,
        require_tool_support = require_tools,
        "llm_request_plan"
    );

    if require_tools && supports_tools_opt != Some(true) {
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

    let tools = if supports_tools_opt.unwrap_or(true) {
        vec![
            request_code_context_tool_def(),
            get_file_metadata_tool_def(),
            apply_code_edit_tool_def(),
        ]
    } else {
        Vec::new()
    };

    // Summarize tools we will include for this request
    let tool_names: Vec<&str> = tools.iter().map(|t| t.function.name).collect();
    tracing::info!(
        use_tools = %(!tools.is_empty()),
        tools = %tool_names.join(","),
        "llm_request_tools"
    );

    // Delegate the per-request loop to RequestSession (Milestone 2 extraction)
    let session = session::RequestSession::new(
        client,
        provider,
        Arc::clone(event_bus),
        parent_id,
        messages,
        tools,
        params.clone(),
    );

    session.run().await
}

pub(super) fn cap_messages_by_chars<'a>(
    messages: &'a [RequestMessage<'a>],
    budget: usize,
) -> Vec<RequestMessage<'a>> {
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

// Example tool-call argument struct
#[derive(Serialize, Deserialize, Debug, Clone)]
struct RequestCodeContextArgs {
    pub token_budget: u32,
    #[serde(default)]
    pub hint: Option<String>,
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
        call_id: String,
        vendor: ToolVendor,
    },
    Completed {
        request_id: Uuid,
        parent_id: Uuid,
        call_id: String,
        content: String,
    },
    Failed {
        request_id: Uuid,
        parent_id: Uuid,
        call_id: String,
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
        name: String,
        arguments: Value,
        call_id: Option<String>,
        vendor: ToolVendor,
    },

    /// Prompt constructed to be sent to the LLM
    PromptConstructed {
        prompt: Vec<(MessageKind, String)>,
        parent_id: Uuid,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToolVendor {
    OpenAI,
    Other,
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
            LlmError::Unknown(msg) => ploke_error::Error::Internal(
                ploke_error::InternalError::NotImplemented(format!("Unknown error: {}", msg)),
            ),
        }
    }
}

use crate::user_config::default_model;
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
            tool_max_retries: Some(2),
            tool_token_limit: Some(2048),
            history_char_budget: Some(12000),
            tool_timeout_secs: Some(30),
        }
    }
}

fn default_true() -> bool {
    true
}
