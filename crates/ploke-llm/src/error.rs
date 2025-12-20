use std::sync::Arc;

use thiserror::Error;

use crate::response::{FinishReason, OpenAiResponse};

use super::*;

/// Represents errors that can occur during LLM interactions.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum LlmError {
    #[error("Invalid Conversion: {0}")]
    Conversion(String),
    /// Error related to network connectivity or the HTTP request itself.
    #[error("Network request failed: {message}")]
    Request {
        message: String,
        /// Optional URL for additional context.
        url: Option<String>,
        /// Hint for retry logic/diagnostics.
        is_timeout: bool,
    },

    /// The API provider returned a non-success status code.
    #[error("API error (status {status}): {message}")]
    Api {
        status: u16,
        message: String,
        /// Optional URL for correlation.
        url: Option<String>,
        /// Truncated body snippet for diagnostics.
        body_snippet: Option<String>,
    },

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
    #[error("Failed to deserialize response data: {message}")]
    Deserialization {
        message: String,
        /// Optional truncated snippet of the offending body.
        body_snippet: Option<String>,
    },

    /// Failed to deserialize the API response.
    #[error("Tool call failed: {0}")]
    ToolCall(String),

    /// An unexpected or unknown error occurred.
    #[error("An unknown error occurred: {0}")]
    Unknown(String),

    /// Failed to deserialize the API response.
    #[error("Embedding Error: {0}")]
    Embedding(String),

    /// Failed to deserialize the API response.
    #[error("ChatStep Error: {0}")]
    ChatStep(String),

    #[error("FinishReason Error: {msg}")]
    FinishError {
        msg: String,
        full_response: OpenAiResponse,
        finish_reason: FinishReason,
    },
}

impl LlmError {
    /// Returns a diagnostic string with contextual fields for UI/log surfaces.
    pub fn diagnostic(&self) -> String {
        match self {
            LlmError::Request {
                message,
                url,
                is_timeout,
            } => {
                let mut msg = format!("Network request failed: {message}");
                if let Some(u) = url {
                    msg.push_str(&format!("\nurl: {u}"));
                }
                if *is_timeout {
                    msg.push_str("\ncontext: timed out");
                }
                msg
            }
            LlmError::Api {
                status,
                message,
                url,
                body_snippet,
            } => {
                let mut msg = format!("API error (status {status}): {message}");
                if let Some(u) = url {
                    msg.push_str(&format!("\nurl: {u}"));
                }
                if let Some(snippet) = body_snippet {
                    msg.push_str("\nbody excerpt: ");
                    msg.push_str(snippet);
                }
                msg
            }
            LlmError::Deserialization {
                message,
                body_snippet,
            } => {
                let mut msg = format!("Failed to deserialize response data: {message}");
                if let Some(snippet) = body_snippet {
                    if !message.contains(snippet) {
                        msg.push_str("\nbody excerpt: ");
                        msg.push_str(snippet);
                    }
                }
                msg
            }
            LlmError::ToolCall(message) => {
                format!("Tool call failed: {message}")
            }
            other => other.to_string(),
        }
    }
}

impl From<LlmError> for ploke_error::Error {
    fn from(error: LlmError) -> Self {
        match error {
            LlmError::Request {
                message,
                is_timeout,
                ..
            } => ploke_error::Error::Internal(ploke_error::InternalError::EmbedderError(
                std::sync::Arc::new(if is_timeout {
                    std::io::Error::new(std::io::ErrorKind::TimedOut, message)
                } else {
                    std::io::Error::new(std::io::ErrorKind::ConnectionAborted, message)
                }),
            )),
            LlmError::Api {
                status, message, ..
            } => ploke_error::Error::Internal(ploke_error::InternalError::EmbedderError(
                std::sync::Arc::new(std::io::Error::other(format!(
                    "API error {}: {}",
                    status, message
                ))),
            )),
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
            LlmError::Serialization(message) => {
                ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(format!(
                    "Serialization error: {}",
                    message
                )))
            }
            LlmError::Deserialization { message, .. } => {
                ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(format!(
                    "Deserialization error: {}",
                    message
                )))
            }
            LlmError::ToolCall(message) => ploke_error::Error::Internal(
                ploke_error::InternalError::NotImplemented(format!("Tool Call error: {}", message)),
            ),
            LlmError::Conversion(message) => ploke_error::Error::Internal(
                ploke_error::InternalError::NotImplemented(message.to_string()),
            ),
            LlmError::Unknown(message) => ploke_error::Error::Internal(
                ploke_error::InternalError::NotImplemented(format!("Unknown error: {}", message)),
            ),
            err_ev @ LlmError::Embedding(_) => ploke_error::Error::Internal(
                ploke_error::InternalError::EmbedderError(std::sync::Arc::new(err_ev)),
            ),
            err_chat @ LlmError::ChatStep(_) => ploke_error::Error::Warning(
                ploke_error::WarningError::PlokeLlm(err_chat.to_string()),
            ),
            // TODO: Add more match arms for levels of error by `FinishReason`
            err_llm @ LlmError::FinishError { .. } => ploke_error::Error::Warning(
                ploke_error::WarningError::PlokeLlm(err_llm.to_string()),
            ),
        }
    }
}
