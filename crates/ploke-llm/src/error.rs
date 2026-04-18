use thiserror::Error;

use crate::response::{FinishReason, OpenAiResponse};
use ploke_error::{
    LlmBodyFailure as StableLlmBodyFailure, LlmReceiveFailure as StableLlmReceiveFailure,
    LlmReceivePhase as StableLlmReceivePhase, LlmSendFailure as StableLlmSendFailure,
    LlmTransportFailure as StableLlmTransportFailure, LlmTransportPhase as StableLlmTransportPhase,
};

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpFailure {
    pub url: Option<String>,
    pub elapsed_ms: Option<u128>,
    pub detail: String,
    pub phase: HttpPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HttpPhase {
    Send(HttpSendFailure),
    Receive(HttpReceiveFailure),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HttpSendFailure {
    Timeout,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpReceiveFailure {
    pub status: Option<u16>,
    pub phase: HttpReceivePhase,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HttpReceivePhase {
    Headers,
    Body(HttpBodyFailure),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HttpBodyFailure {
    Timeout,
    ReadFailed,
    DecodeFailed,
}

impl HttpFailure {
    pub fn send(
        url: Option<String>,
        elapsed_ms: Option<u128>,
        detail: impl Into<String>,
        phase: HttpSendFailure,
    ) -> Self {
        Self {
            url,
            elapsed_ms,
            detail: detail.into(),
            phase: HttpPhase::Send(phase),
        }
    }

    pub fn receive(
        url: Option<String>,
        elapsed_ms: Option<u128>,
        status: Option<u16>,
        detail: impl Into<String>,
        phase: HttpReceivePhase,
    ) -> Self {
        Self {
            url,
            elapsed_ms,
            detail: detail.into(),
            phase: HttpPhase::Receive(HttpReceiveFailure { status, phase }),
        }
    }

    pub fn diagnostic(&self) -> String {
        let mut msg = self.to_string();
        msg.push_str("\ntransport detail: ");
        msg.push_str(&self.detail);
        msg
    }

    pub fn to_ploke_transport_failure(&self) -> StableLlmTransportFailure {
        StableLlmTransportFailure {
            url: self.url.clone(),
            elapsed_ms: self.elapsed_ms,
            detail: self.detail.clone(),
            phase: match &self.phase {
                HttpPhase::Send(HttpSendFailure::Timeout) => {
                    StableLlmTransportPhase::Send(StableLlmSendFailure::Timeout)
                }
                HttpPhase::Send(HttpSendFailure::Failed) => {
                    StableLlmTransportPhase::Send(StableLlmSendFailure::Failed)
                }
                HttpPhase::Receive(receive) => {
                    StableLlmTransportPhase::Receive(StableLlmReceiveFailure {
                        status: receive.status,
                        phase: match &receive.phase {
                            HttpReceivePhase::Headers => StableLlmReceivePhase::Headers,
                            HttpReceivePhase::Body(HttpBodyFailure::Timeout) => {
                                StableLlmReceivePhase::Body(StableLlmBodyFailure::Timeout)
                            }
                            HttpReceivePhase::Body(HttpBodyFailure::ReadFailed) => {
                                StableLlmReceivePhase::Body(StableLlmBodyFailure::ReadFailed)
                            }
                            HttpReceivePhase::Body(HttpBodyFailure::DecodeFailed) => {
                                StableLlmReceivePhase::Body(StableLlmBodyFailure::DecodeFailed)
                            }
                        },
                    })
                }
            },
        }
    }
}

impl std::fmt::Display for HttpFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let summary = match &self.phase {
            HttpPhase::Send(HttpSendFailure::Timeout) => {
                "Timed out while sending request to LLM provider."
            }
            HttpPhase::Send(HttpSendFailure::Failed) => {
                "Failed while sending request to LLM provider."
            }
            HttpPhase::Receive(HttpReceiveFailure {
                phase: HttpReceivePhase::Headers,
                ..
            }) => "Failed while receiving response headers from LLM provider.",
            HttpPhase::Receive(HttpReceiveFailure {
                phase: HttpReceivePhase::Body(HttpBodyFailure::Timeout),
                ..
            }) => "Timed out while reading provider response body after receiving headers.",
            HttpPhase::Receive(HttpReceiveFailure {
                phase: HttpReceivePhase::Body(HttpBodyFailure::ReadFailed),
                ..
            }) => "Failed while reading provider response body after receiving headers.",
            HttpPhase::Receive(HttpReceiveFailure {
                phase: HttpReceivePhase::Body(HttpBodyFailure::DecodeFailed),
                ..
            }) => "Provider response body could not be decoded after receiving headers.",
        };

        write!(f, "{summary}")?;
        if let HttpPhase::Receive(HttpReceiveFailure {
            status: Some(status),
            ..
        }) = &self.phase
        {
            write!(f, " status={status}")?;
        }
        if let Some(elapsed_ms) = self.elapsed_ms {
            write!(f, " elapsed_ms={elapsed_ms}")?;
        }
        if let Some(url) = &self.url {
            write!(f, " url={url}")?;
        }
        Ok(())
    }
}

impl std::error::Error for HttpFailure {}

/// Represents errors that can occur during LLM interactions.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum LlmError {
    #[error("Invalid Conversion: {0}")]
    Conversion(String),
    #[error(transparent)]
    Http(#[from] HttpFailure),

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
            LlmError::Http(http) => http.diagnostic(),
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
            LlmError::Http(http) => ploke_error::Error::Internal(
                ploke_error::InternalError::LlmTransport(http.to_ploke_transport_failure()),
            ),
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
                ploke_error::InternalError::LlmTransport(ploke_error::LlmTransportFailure {
                    url: None,
                    elapsed_ms: None,
                    detail: "Request timed out".to_string(),
                    phase: ploke_error::LlmTransportPhase::Send(
                        ploke_error::LlmSendFailure::Timeout,
                    ),
                }),
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
