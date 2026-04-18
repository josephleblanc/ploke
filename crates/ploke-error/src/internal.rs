use std::sync::Arc;

/// Errors indicating bugs, invalid states, or infrastructure failures inside the system.
///
/// These map to [`crate::Severity::Error`] and usually warrant investigation.
#[cfg_attr(feature = "diagnostic", derive(miette::Diagnostic))]
#[derive(Clone, Debug, thiserror::Error)]
pub enum InternalError {
    #[error("Internal compiler error: {0}")]
    CompilerError(String),

    #[error("Unexpected state: {0}")]
    InvalidState(&'static str),

    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    #[error("{0}")]
    LlmTransport(LlmTransportFailure),

    #[error("Embedding error: {0}")]
    EmbedderError(Arc<dyn std::error::Error + Send + Sync>),
}

impl InternalError {
    pub fn embedder_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> Self {
        Self::EmbedderError(Arc::new(e))
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmTransportFailure {
    pub url: Option<String>,
    pub elapsed_ms: Option<u128>,
    pub detail: String,
    pub phase: LlmTransportPhase,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmTransportPhase {
    Send(LlmSendFailure),
    Receive(LlmReceiveFailure),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmSendFailure {
    Timeout,
    Failed,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmReceiveFailure {
    pub status: Option<u16>,
    pub phase: LlmReceivePhase,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmReceivePhase {
    Headers,
    Body(LlmBodyFailure),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmBodyFailure {
    Timeout,
    ReadFailed,
    DecodeFailed,
}

impl std::fmt::Display for LlmTransportFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let summary = match &self.phase {
            LlmTransportPhase::Send(LlmSendFailure::Timeout) => {
                "Timed out while sending request to LLM provider."
            }
            LlmTransportPhase::Send(LlmSendFailure::Failed) => {
                "Failed while sending request to LLM provider."
            }
            LlmTransportPhase::Receive(LlmReceiveFailure {
                phase: LlmReceivePhase::Headers,
                ..
            }) => "Failed while receiving response headers from LLM provider.",
            LlmTransportPhase::Receive(LlmReceiveFailure {
                phase: LlmReceivePhase::Body(LlmBodyFailure::Timeout),
                ..
            }) => "Timed out while reading provider response body after receiving headers.",
            LlmTransportPhase::Receive(LlmReceiveFailure {
                phase: LlmReceivePhase::Body(LlmBodyFailure::ReadFailed),
                ..
            }) => "Failed while reading provider response body after receiving headers.",
            LlmTransportPhase::Receive(LlmReceiveFailure {
                phase: LlmReceivePhase::Body(LlmBodyFailure::DecodeFailed),
                ..
            }) => "Provider response body could not be decoded after receiving headers.",
        };

        write!(f, "{summary}")?;
        if let Some(status) = self.status() {
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

impl std::error::Error for LlmTransportFailure {}

impl LlmTransportFailure {
    pub fn status(&self) -> Option<u16> {
        match &self.phase {
            LlmTransportPhase::Send(_) => None,
            LlmTransportPhase::Receive(receive) => receive.status,
        }
    }
}
