pub mod registry;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{default, sync::Arc, time::Duration};
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    oneshot,
};
use tracing::instrument;
use uuid::Uuid;

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
        }
    }
}

#[derive(Serialize, Debug)]
pub struct RequestMessage<'a> {
    pub role: &'a str,
    pub content: String,
}

impl RequestMessage<'_> {
    pub fn new_system(content: String) -> Self {
        Self {
            role: "system",
            content,
        }
    }
}

#[derive(Deserialize, Debug)]
struct OpenAiResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize, Debug)]
struct ResponseMessage {
    content: String,
}

pub async fn llm_manager(
    mut event_rx: broadcast::Receiver<AppEvent>,
    state: Arc<AppState>,
    cmd_tx: mpsc::Sender<StateCommand>,
    // providers: crate::user_config::ProviderRegistry,
) {
    let client = Client::new();
    let mut pending_requests = Vec::new();
    let mut ready_contexts = std::collections::HashMap::new();

    while let Ok(event) = event_rx.recv().await {
        match event {
            AppEvent::Llm(
                request @ llm::Event::Request {
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
            AppEvent::Llm(context @ llm::Event::PromptConstructed { parent_id, .. }) => {
                tracing::info!("Received context for parent_id: {}", parent_id);
                ready_contexts.insert(parent_id, context);

                let guard = state.config.read().await;
                let maybe_provider = guard.provider_registry.get_active_provider();
                if maybe_provider.is_none() {
                    tracing::warn!("Could not find active provider in registry, returning early");
                    return;
                }
                let provider_config = maybe_provider
                    .expect("Error in unwrapping a value guarenteed to be Some")
                    .clone();
                drop(guard);
                // Process any pending requests that now have context
                pending_requests.retain(|req| {
                    if let llm::Event::Request {
                        new_msg_id: req_parent,
                        // parent_id: req_parent,
                        ..
                    } = req
                    {
                        tracing::info!(
                            "pending_requests found match for req_parent: {}",
                            req_parent
                        );
                        if let Some(context) = ready_contexts.remove(req_parent) {
                            tracing::info!(
                                "ready_contexts found match for req_parent: {}",
                                req_parent
                            );
                            tokio::spawn(process_llm_request(
                                req.clone(),
                                Arc::clone(&state),
                                cmd_tx.clone(),
                                client.clone(),
                                provider_config.to_owned(),
                                Some(context),
                            ));
                            tracing::info!("removing id from pending_requests");
                            false // Remove from pending
                        } else {
                            tracing::info!("keep id from pending_requests
                                found pending_request but not ready_context
                                checking if ready_contexts removed req_parent during conditional: {}", 
                                ready_contexts.contains_key(req_parent));
                            true // Keep waiting
                        }
                    } else {
                        tracing::info!("keep id from pending_requests\nno matched pending_requests");
                        true
                    }
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
    request: llm::Event,
    state: Arc<AppState>,
    cmd_tx: mpsc::Sender<StateCommand>,
    client: Client,
    provider_config: crate::user_config::ProviderConfig,
    context: Option<llm::Event>,
) {
    tracing::info!("Inside process_llm_request");
    let parent_id = match request {
        llm::Event::Request {
            parent_id,
            new_msg_id,
            ..
        } => new_msg_id,
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
    let update_cmd = prepare_and_run_llm_call(&state, &client, &provider_config, context)
        .await
        .map(|content| StateCommand::UpdateMessage {
            id: assistant_message_id,
            update: MessageUpdate {
                content: Some(content),
                status: Some(MessageStatus::Completed),
                ..Default::default()
            },
        })
        .unwrap_or_else(|e| {
            let err_string = e.to_string();
            StateCommand::UpdateMessage {
                id: assistant_message_id,
                update: MessageUpdate {
                    content: Some(format!("Error: {}", err_string)),
                    status: Some(MessageStatus::Error {
                        description: err_string,
                    }),
                    ..Default::default()
                },
            }
        });

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
    context: Option<llm::Event>,
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

    // Prepend system prompt if provided
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
            })
            .collect::<Vec<_>>()
    } else {
        context_path
            .iter()
            .filter(|msg| (msg.kind != MessageKind::SysInfo) && !msg.content.is_empty())
            .map(|msg| RequestMessage {
                role: msg.kind.into(),
                content: msg.content.clone(),
            })
            .collect::<Vec<_>>()
    };

    messages.extend(conversation_messages);

    log::info!(
        "Sending conversation history message with content: {:#?}",
        messages
    );
    log::info!("Sending request using model config: {:#?}", provider);
    // Release the lock before the network call
    drop(history_guard);

    tracing::info!(
        "Inside prepare_and_run_llm_call num2 {:#?}",
        provider.llm_params
    );
    let params = provider.llm_params.as_ref().cloned().unwrap_or_default();
    let request_payload = OpenAiRequest {
        model: provider.model.as_str(),
        messages,
        temperature: params.temperature,
        max_tokens: params.max_tokens,
        top_p: params.top_p,
        stream: false,
    };

    tracing::info!(
        "Inside prepare_and_run_llm_call num3 {:#?}",
        request_payload
    );

    let response_test = client
        .post(format!("{}/chat/completions", provider.base_url))
        .bearer_auth(provider.resolve_api_key())
        .json(&request_payload);
    
    tracing::info!(
        "request_payload is {:#?}",
        request_payload
    );

    let response = client
        .post(format!("{}/chat/completions", provider.base_url))
        .bearer_auth(provider.resolve_api_key())
        .json(&request_payload)
        .send()
        .await
        .map_err(|e| LlmError::Request(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Could not retrieve error body".to_string());

        let user_friendly_msg = if status == 401 {
            format!(
                "Authentication failed. Please check your API key configuration.\n\nDetails {}",
                error_text
            )
        } else if status == 429 {
            format!(
                "Rate limit exceeded. Please wait and try again.\n\nDetails: {}",
                error_text
            )
        } else if status >= 500 {
            format!(
                "Server error. The API provider may be experiencing issues.\n\nDetails: {}",
                error_text
            )
        } else {
            format!("API error (status {}): {}", status, error_text)
        };

        return Err(LlmError::Api {
            status,
            message: user_friendly_msg,
        });
    }

    let body = response
        .text()
        .await
        .map_err(|e| LlmError::Request(e.to_string()))?;
    tracing::debug!("raw body: {}", body);

    // OpenRouter sometimes puts errors inside a 200 body
    if let Ok(err) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(err_obj) = err.get("error") {
            let msg = err_obj
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown OpenRouter error");
            let code = err_obj.get("code").and_then(|v| v.as_u64()).unwrap_or(0);
            return Err(LlmError::Api {
                status: code as u16,
                message: msg.to_string(),
            });
        }
    }

    // Now safe to deserialize into OpenAiResponse
    let response_body: OpenAiResponse = serde_json::from_str(&body)
        .map_err(|e| LlmError::Deserialization(format!("{} — body was: {}", e, body)))?;
    tracing::trace!("raw body response to request: {:#?}", body);

    let content = response_body
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .unwrap_or_else(|| "No content received from API.".to_string());

    Ok(content)
}

fn kind_to_str<'a>(msg: &'a &'a Message) -> &'a str {
    match msg.kind {
        MessageKind::User => "user",
        MessageKind::Assistant => "assistant",
        MessageKind::System => "system",
        MessageKind::Tool => todo!(),
        MessageKind::SysInfo => "sysinfo",
    }
}

// Backpressure-aware command sender
struct CommandSender {
    inner: mpsc::Sender<StateCommand>,
    event_bus: Arc<EventBus>,
}

impl CommandSender {
    async fn send(&self, cmd: StateCommand) {
        match self.inner.try_send(cmd) {
            Ok(_) => {}
            Err(TrySendError::Full(cmd)) => {
                self.event_bus
                    .send(AppEvent::System(SystemEvent::CommandDropped(
                        cmd.discriminant(),
                    )));
                // Optional retry logic
                tokio::time::sleep(Duration::from_millis(10)).await;
                if let Err(e) = self.inner.send(cmd).await {
                    log::error!("Permanent send failure: {}", e);
                    // possibly more logging here.
                }
            }
            Err(TrySendError::Closed(_)) => {
                // TODO: What should go here?
                // check docs on ratatui? tokio?
                // Shutting down
            }
        }
    }
}

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
            Event::PartialResponse { request_id, delta } => todo!(),
            Event::Error { request_id, error } => todo!(),
            Event::Status {
                active_requests,
                queue_depth,
            } => todo!(),
            Event::ModelChanged { new_model } => todo!(),
            Event::PromptConstructed { prompt, parent_id } => *parent_id,
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
        }
    }
}

fn default_true() -> bool {
    true
}
