use ploke_core::{
    ArcStr,
    rag_types::{ContextPartKind, ContextStats},
};
use ploke_core::tool_types::ToolName;
use ploke_llm::{
    LLMMetadata, LlmError, RequestMessage,
    manager::events::{ToolEvent, UsageMetrics, embedding_models, endpoint, models, status},
};
use serde_json::Value;
use uuid::Uuid;

use crate::chat_history::MessageKind;

#[derive(Clone, Debug)]
pub struct ContextPlan {
    pub plan_id: Uuid,
    pub parent_id: Uuid,
    pub estimated_total_tokens: usize,
    pub included_messages: Vec<ContextPlanMessage>,
    pub included_rag_parts: Vec<ContextPlanRagPart>,
    pub rag_stats: Option<ContextStats>,
}

#[derive(Clone, Debug)]
pub struct ContextPlanMessage {
    pub message_id: Option<Uuid>,
    pub kind: MessageKind,
    pub estimated_tokens: usize,
}

#[derive(Clone, Debug)]
pub struct ContextPlanRagPart {
    pub part_id: Uuid,
    pub file_path: String,
    pub kind: ContextPartKind,
    pub estimated_tokens: usize,
    pub score: f32,
}

#[derive(Clone, Debug)]
pub enum LlmEvent {
    ChatCompletion(ChatEvt),
    // NOTE: the event for `Completion` is unused, so commenting it out for now.
    // See `ploke/docs/active/open-questions/llm-api-related.md` for details/updates
    // Completion(ChatEvt),
    Tool(ToolEvent),
    Endpoint(endpoint::Event),
    Models(models::Event),
    EmbeddingModels(embedding_models::Event),
    Status(status::Event),
}

// NOTE:ploke-llm 2025-12-14
//Keeping this for now in case the
#[derive(Clone, Debug)]
pub enum ChatEvt {
    /// Request to generate content from an LLM
    Request {
        request_msg_id: Uuid, // Unique tracking ID
        parent_id: Uuid,      // Message this responds to
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
        context_plan: ContextPlan,
    },
}

// impl From<LlmChatEvt> for ChatEvt {
//     fn from(value: LlmChatEvt) -> Self {
//         match value {
//             LlmChatEvt::Request {
//                 request_msg_id,
//                 parent_id,
//             } => ChatEvt::Request {
//                 request_msg_id,
//                 parent_id,
//             },
//             LlmChatEvt::Response {
//                 request_id,
//                 parent_id,
//                 content,
//                 model,
//                 metadata,
//                 usage,
//             } => ChatEvt::Response {
//                 request_id,
//                 parent_id,
//                 content,
//                 model,
//                 metadata,
//                 usage,
//             },
//             LlmChatEvt::PartialResponse {
//                 request_id,
//                 delta,
//             } => ChatEvt::PartialResponse {
//                 request_id,
//                 delta,
//             },
//             LlmChatEvt::Error {
//                 request_id,
//                 error,
//             } => ChatEvt::Error {
//                 request_id,
//                 error,
//             },
//             LlmChatEvt::Status {
//                 active_requests,
//                 queue_depth,
//             } => ChatEvt::Status {
//                 active_requests,
//                 queue_depth,
//             },
//             LlmChatEvt::ModelChanged { new_model } => ChatEvt::ModelChanged { new_model },
//             LlmChatEvt::ToolCall {
//                 request_id,
//                 parent_id,
//                 name,
//                 arguments,
//                 call_id,
//             } => ChatEvt::ToolCall {
//                 request_id,
//                 parent_id,
//                 name,
//                 arguments,
//                 call_id,
//             },
//             LlmChatEvt::PromptConstructed {
//                 parent_id,
//                 formatted_prompt,
//             } => ChatEvt::PromptConstructed {
//                 parent_id,
//                 formatted_prompt,
//             },
//         }
//     }
// }
