use crate::{
    llm2::{error::LlmError, request::endpoint::EndpointsResponse, router_only::{Router, RouterVariants}, types::meta::LLMMetadata}, EventPriority
};

use super::*;

#[derive(Clone, Debug)]
pub(crate) enum LlmEvent {
    ChatCompletion(LlmChatEvent),
    Completion(LlmChatEvent),
    Endpoint(endpoint::Event),
    Models(models::Event),
}

pub(crate) mod endpoint {
    use super::*;

    #[derive(Clone, Debug)]
    pub(crate) enum Event {
        Request {
            parent_id: Uuid,
            model: ModelKey, // e.g., "gpt-4-turbo"
            // Larger response, make an Arc to hold it
            endpoints: Arc<EndpointsResponse>,
        },
        Response {
            parent_id: Uuid,
            model: ModelKey, // e.g., "gpt-4-turbo"
            // Larger response, make an Arc to hold it
            endpoints: Arc<EndpointsResponse>,
        },
        Error {
            request_id: Uuid,
            error: LlmError, // Structured error type
        },
    }
}

pub(crate) mod models {
    use crate::llm2::request;

    use super::*;
    #[derive(Clone, Debug)]
    pub(crate) enum Event {
        /// A request to the `/models` endpoint for a router, which should return a list of models
        /// that contain the model identifier in the form `{author}/{model}:{variant}` where
        /// `:{variant}` is optional.
        Request {
            parent_id: Uuid,
            router: RouterVariants
        },
        /// Response with the full returned values for the models.
        Response {
            parent_id: Uuid,
            /// The information on all models, can be used to update the cache'd model info and/or
            /// persisted into the database.
            /// - Caches the owned deserialized values in-memory, then persist with 12-hour update
            /// cycles.
            // Larger response, make an Arc to hold it
            models: Arc<request::models::Response>,
        },
        Error {
            request_id: Uuid,
            error: LlmError, // Structured error type
        },
    }
}

#[derive(Clone, Debug)]
pub(crate) enum LlmChatEvent {
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

    EndpointRequest {
        parent_id: Uuid,
        // Is expecting a more loosely typed `ModelId` here, whatever generic kind has been
        // returned by the `/models` endpoint, e.g. OpenRouter might have
        // - nousresearch/deephermes-3-llama-3-8b-preview:free
        //
        // which is of the form {author}/{model}:{variant} as opposed to the more standard
        // {author}/{model}
        model_id: ModelId, // e.g., "openai/gpt-4-turbo"
        // Add Router as well to know where to send it and how to interpret the ModelId
        router: RouterVariants,
    },

    RequestModels {
        // no models needed to query all models
        parent_id: Uuid,
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
