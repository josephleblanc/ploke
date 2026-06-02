use ploke_core::ArcStr;
use ploke_core::tool_contracts::tool_error_code_label;
pub use ploke_core::tool_contracts::{
    ToolErrorCode, ToolErrorWire, ToolLlmErrorPayload, ToolLlmErrorValue, ToolRetryContext,
    ToolRetryContextField, ToolRetryContextValue, ToolUiPayload,
};
use ploke_core::tool_types::ToolName;
use ploke_error::DomainError;
use ploke_error::Error as PlokeError;
use ploke_llm::LlmError;
use serde::Serialize;

/// Audience for formatting diagnostics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Audience {
    User,
    Llm,
    System,
}

use serde::Deserialize;

/// Structured tool error with audience-aware rendering.
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{code:?} error in tool {tool:?}: {message}")]
pub struct ToolError {
    pub tool: ToolName,
    pub code: ToolErrorCode,
    pub field: Option<&'static str>,
    pub expected: Option<String>,
    pub received: Option<String>,
    pub snippet: Option<String>,
    pub retry_hint: Option<String>,
    pub retry_context: Option<ToolRetryContext>,
    #[serde(skip)]
    pub audience: Audience,
    #[serde(skip)]
    pub message: String,
}

impl ToolError {
    pub fn new(tool: ToolName, code: ToolErrorCode, message: impl Into<String>) -> Self {
        Self {
            tool,
            code,
            field: None,
            expected: None,
            received: None,
            snippet: None,
            retry_hint: None,
            retry_context: None,
            audience: Audience::System,
            message: message.into(),
        }
    }

    pub fn field(mut self, field: &'static str) -> Self {
        self.field = Some(field);
        self
    }

    pub fn expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    pub fn received(mut self, received: impl Into<String>) -> Self {
        self.received = Some(received.into());
        self
    }

    pub fn snippet(mut self, snippet: impl Into<String>) -> Self {
        self.snippet = Some(snippet.into());
        self
    }

    pub fn retry_hint(mut self, hint: impl Into<String>) -> Self {
        self.retry_hint = Some(hint.into());
        self
    }

    pub fn retry_context(mut self, context: impl Into<ToolRetryContext>) -> Self {
        self.retry_context = Some(context.into());
        self
    }

    pub fn with_audience(mut self, audience: Audience) -> Self {
        self.audience = audience;
        self
    }

    pub fn format_for_audience(&self, audience: Audience) -> String {
        let mut parts: Vec<String> = Vec::new();

        let base = match audience {
            Audience::User => format!("{}: {}", self.tool.as_str(), self.message),
            Audience::Llm => format!(
                "Tool `{}` arguments need correction: {}",
                self.tool.as_str(),
                self.message
            ),
            Audience::System => format!(
                "tool={:?} code={:?}: {}",
                self.tool, self.code, self.message
            ),
        };
        parts.push(base);

        if let Some(field) = self.field {
            parts.push(format!("field: {}", field));
        }
        if let Some(expected) = &self.expected {
            parts.push(format!("expected: {}", expected));
        }
        if let Some(received) = &self.received {
            parts.push(format!("received: {}", received));
        }
        if let Some(snippet) = &self.snippet {
            parts.push(format!("snippet: {}", snippet));
        }

        parts.join(" — ")
    }

    pub fn to_llm_payload(&self) -> ToolLlmErrorPayload {
        ToolLlmErrorPayload::from_parts(
            self.tool,
            self.code,
            self.field.map(str::to_string),
            self.expected.clone(),
            self.received.clone(),
            self.message.clone(),
            self.snippet.clone(),
            self.retry_hint.clone(),
            self.retry_context.clone(),
        )
    }

    pub fn to_wire(&self) -> ToolErrorWire {
        ToolErrorWire {
            user: self.format_for_audience(Audience::User),
            llm: self.to_llm_payload(),
            system: self.format_for_audience(Audience::System),
        }
    }

    pub fn to_wire_string(&self) -> String {
        serde_json::to_string(&self.to_wire())
            .unwrap_or_else(|_| self.format_for_audience(Audience::User))
    }

    pub fn to_ui_payload(&self, call_id: ArcStr) -> ToolUiPayload {
        let mut payload =
            ToolUiPayload::new(self.tool, call_id, self.format_for_audience(Audience::User));
        payload.error = Some(self.to_wire());
        payload.error_code = Some(self.code);
        payload = payload.with_field("code", tool_error_code_label(self.code));
        if let Some(field) = self.field {
            payload = payload.with_field("field", field);
        }
        if let Some(expected) = &self.expected {
            payload = payload.with_field("expected", expected.as_str());
        }
        if let Some(received) = &self.received {
            payload = payload.with_field("received", received.as_str());
        }
        payload
    }
}

/// Adapter error enum used by the Tool trait.
#[derive(Debug, thiserror::Error)]
pub enum ToolInvocationError {
    #[error("transport")]
    Transport(#[from] LlmError),
    #[error("deserialize")]
    Deserialize {
        source: serde_json::Error,
        raw: Option<String>,
    },
    #[error("validation")]
    Validation(#[from] ToolError),
    #[error("exec")]
    Exec(#[from] ploke_error::Error),
    #[error("internal: {0}")]
    Internal(String),
}

impl ToolInvocationError {
    pub fn into_tool_error(self, tool: ToolName) -> ToolError {
        match self {
            ToolInvocationError::Validation(te) => te,
            ToolInvocationError::Transport(err) => {
                ToolError::new(tool, ToolErrorCode::Internal, err.to_string())
            }
            ToolInvocationError::Deserialize { source, raw } => {
                let mut te = ToolError::new(
                    tool,
                    ToolErrorCode::WrongType,
                    format!("failed to parse tool arguments: {}", source),
                );
                if let Some(raw) = raw {
                    te = te.snippet(truncate_for_error(&raw, 512));
                }
                te
            }
            ToolInvocationError::Exec(err) => {
                ToolError::from_ploke_error_with_tool(tool, err).with_audience(Audience::System)
            }
            ToolInvocationError::Internal(msg) => {
                ToolError::new(tool, ToolErrorCode::Internal, msg)
            }
        }
    }
}

impl From<ploke_error::Error> for ToolError {
    fn from(err: ploke_error::Error) -> Self {
        ToolError::from_ploke_error_with_tool(ToolName::RequestCodeContext, err)
    }
}

impl ToolError {
    pub fn from_ploke_error_with_tool(tool: ToolName, err: ploke_error::Error) -> Self {
        let code = match &err {
            PlokeError::Domain(DomainError::Io { .. }) => ToolErrorCode::Io,
            PlokeError::Domain(DomainError::Ui { .. }) => ToolErrorCode::InvalidFormat,
            _ => ToolErrorCode::Internal,
        };
        ToolError::new(tool, code, err.to_string())
    }
}

pub(crate) fn truncate_for_error(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let head = &s[..max.saturating_sub(200)];
        let tail = &s[s.len().saturating_sub(200)..];
        format!("{head}…<snip>…{tail}")
    }
}

pub fn allowed_tool_names() -> Vec<ArcStr> {
    ToolName::ALL
        .iter()
        .map(|tool| ArcStr::from(tool.as_str()))
        .collect()
}

pub fn tool_ui_error(message: impl Into<String>) -> ploke_error::Error {
    ploke_error::Error::Domain(ploke_error::DomainError::Ui {
        message: message.into(),
    })
}

pub fn tool_ui_payload_from_error(call_id: ArcStr, err: &ToolError) -> ToolUiPayload {
    err.to_ui_payload(call_id)
}

pub fn tool_io_error(message: impl Into<String>) -> ploke_error::Error {
    ploke_error::Error::Domain(ploke_error::DomainError::Io {
        message: message.into(),
    })
}
