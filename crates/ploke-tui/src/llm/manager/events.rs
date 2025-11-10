use crate::{
    EventPriority,
    llm::{
        error::LlmError,
        request::endpoint::EndpointsResponse,
        router_only::{Router, RouterVariants},
        types::meta::LLMMetadata,
    },
};

use super::*;

#[derive(Clone, Debug)]
pub(crate) enum LlmEvent {
    ChatCompletion(ChatEvt),
    Completion(ChatEvt),
    Tool(ToolEvent),
    Endpoint(endpoint::Event),
    Models(models::Event),
    Status(status::Event),
}

impl From<LlmEvent> for AppEvent {
    fn from(value: LlmEvent) -> Self {
        AppEvent::Llm(value)
    }
}

pub(crate) mod status {
    use serde_json::Value;
    use uuid::Uuid;

    use crate::{chat_history::MessageKind, tools::ToolName};

    #[derive(Clone, Debug, Copy)]
    pub(crate) enum Event {
        /// Status update
        Update {
            active_requests: usize, // Current workload
            queue_depth: usize,     // Pending requests
        },
    }
}

pub(crate) mod endpoint {
    use crate::llm::types::model_types::ModelVariant;

    use super::*;

    #[derive(Clone, Debug)]
    pub(crate) enum Event {
        Request {
            // removed "parent_id" since that is usually to refer to a conversation history
            // message, where this refers to a model and is not used in a chat history context
            model_key: ModelKey, // e.g., "gpt-4-turbo"
            // Larger response, make an Arc to hold it
            router: RouterVariants,
            variant: Option<ModelVariant>,
        },
        Response {
            model_key: ModelKey, // e.g., "gpt-4-turbo"
            // Larger response, make an Arc to hold it
            endpoints: Option<Arc<EndpointsResponse>>,
        },
        Error {
            request_id: Uuid,
            error: LlmError, // Structured error type
        },
    }
}

pub(crate) mod models {
    use ploke_core::ArcStr;

    use crate::llm::request;

    use super::*;
    #[derive(Clone, Debug)]
    pub(crate) enum Event {
        /// A request to the `/models` endpoint for a router, which should return a list of models
        /// that contain the model identifier in the form `{author}/{model}:{variant}` where
        /// `:{variant}` is optional.
        Request {
            router: RouterVariants,
        },
        /// Response with the full returned values for the models.
        Response {
            /// The information on all models, can be used to update the cached model info and/or
            /// persisted into the database.
            /// - Caches the owned deserialized values in-memory, then persist with 12-hour update
            /// cycles.
            // Larger response, make an Arc to hold it
            models: Option<Arc<request::models::Response>>,
            /// Optional search keyword that initiated this response, so consumers can drop stale
            /// payloads when a newer search is already in-flight.
            search_keyword: Option<ArcStr>,
        },
        Error {
            request_id: Uuid,
            error: LlmError, // Structured error type
        },
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ChatEvt {
    /// Request to generate content from an LLM
    Request {
        request_msg_id: Uuid,          // Unique tracking ID
        parent_id: Uuid,           // Message this responds to
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
    /// Includes:
    /// - conversation history from just-submitted user message to root
    ///     - `Role` of messages: All (User, Assistant, SysInfo, )
    /// - code context
    PromptConstructed {
        parent_id: Uuid,
        formatted_prompt: Vec<RequestMessage>,
    },
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


#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct UsageMetrics {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub latency_ms: u64,
}
