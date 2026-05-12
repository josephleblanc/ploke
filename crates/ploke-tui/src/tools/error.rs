use ploke_core::ArcStr;
use ploke_core::tool_types::ToolName;
use ploke_error::DomainError;
use ploke_error::Error as PlokeError;
use ploke_llm::LlmError;
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::BTreeMap, ops::Index};

/// Bounded retry context attached to tool errors.
///
/// This is intentionally first-order: callers can attach named scalar facts or
/// string lists, but the shared wire shape does not preserve arbitrary nested
/// JSON as replay authority.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolRetryContext {
    pub fields: Vec<ToolRetryContextField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolRetryContextField {
    pub name: String,
    pub value: ToolRetryContextValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ToolRetryContextValue {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    StringList(Vec<String>),
}

impl ToolRetryContext {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    pub fn field(
        mut self,
        name: impl Into<String>,
        value: impl Into<ToolRetryContextValue>,
    ) -> Self {
        self.fields.push(ToolRetryContextField {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    pub fn get(&self, name: &str) -> Option<&ToolRetryContextValue> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.value)
    }
}

impl Default for ToolRetryContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRetryContextValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn as_string_list(&self) -> Option<&[String]> {
        match self {
            Self::StringList(value) => Some(value.as_slice()),
            _ => None,
        }
    }
}

impl From<&str> for ToolRetryContextValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for ToolRetryContextValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&String> for ToolRetryContextValue {
    fn from(value: &String) -> Self {
        Self::String(value.clone())
    }
}

impl From<bool> for ToolRetryContextValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<usize> for ToolRetryContextValue {
    fn from(value: usize) -> Self {
        Self::Number(value.to_string())
    }
}

impl From<u64> for ToolRetryContextValue {
    fn from(value: u64) -> Self {
        Self::Number(value.to_string())
    }
}

impl From<i64> for ToolRetryContextValue {
    fn from(value: i64) -> Self {
        Self::Number(value.to_string())
    }
}

impl From<Vec<String>> for ToolRetryContextValue {
    fn from(value: Vec<String>) -> Self {
        Self::StringList(value)
    }
}

impl From<Vec<&str>> for ToolRetryContextValue {
    fn from(value: Vec<&str>) -> Self {
        Self::StringList(value.into_iter().map(str::to_string).collect())
    }
}

impl From<Vec<char>> for ToolRetryContextValue {
    fn from(value: Vec<char>) -> Self {
        Self::StringList(value.into_iter().map(|ch| ch.to_string()).collect())
    }
}

impl From<Option<&str>> for ToolRetryContextValue {
    fn from(value: Option<&str>) -> Self {
        match value {
            Some(value) => Self::String(value.to_string()),
            None => Self::Null,
        }
    }
}

/// Audience for formatting diagnostics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Audience {
    User,
    Llm,
    System,
}

/// Canonical error codes for tool validation and execution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorCode {
    #[serde(alias = "FieldTooLarge")]
    FieldTooLarge,
    #[serde(alias = "WrongType")]
    WrongType,
    #[serde(alias = "MissingField")]
    MissingField,
    #[serde(alias = "MalformedDiff")]
    MalformedDiff,
    #[serde(alias = "InvalidFormat")]
    InvalidFormat,
    #[serde(alias = "Io")]
    Io,
    #[serde(alias = "Timeout")]
    Timeout,
    #[serde(alias = "Internal")]
    Internal,
}

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

    /// Human-readable message for a given audience.
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

    /// LLM-friendly structured payload embedded in tool result JSON.
    pub fn to_llm_payload(&self) -> ToolLlmErrorPayload {
        ToolLlmErrorPayload {
            ok: false,
            tool: self.tool,
            code: self.code,
            field: self.field.map(str::to_string),
            expected: self.expected.clone(),
            received: self.received.clone(),
            message: self.message.clone(),
            snippet: self.snippet.clone(),
            retry_hint: self.retry_hint.clone(),
            retry_context: self.retry_context.clone(),
            index: ToolLlmErrorPayloadIndex::default(),
        }
        .with_index()
    }

    /// Wire payload containing both user-facing and LLM payloads.
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
        // Without tool context, fall back to the first known tool; callers should prefer
        // `ToolError::from_ploke_error_with_tool` for accurate attribution.
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

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ToolLlmErrorPayload {
    pub ok: bool,
    pub tool: ToolName,
    pub code: ToolErrorCode,
    pub field: Option<String>,
    pub expected: Option<String>,
    pub received: Option<String>,
    pub message: String,
    pub snippet: Option<String>,
    pub retry_hint: Option<String>,
    pub retry_context: Option<ToolRetryContext>,
    #[serde(skip)]
    index: ToolLlmErrorPayloadIndex,
}

impl<'de> Deserialize<'de> for ToolLlmErrorPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Payload {
            ok: bool,
            tool: ToolName,
            code: ToolErrorCode,
            field: Option<String>,
            expected: Option<String>,
            received: Option<String>,
            message: String,
            snippet: Option<String>,
            retry_hint: Option<String>,
            retry_context: Option<ToolRetryContext>,
        }

        let payload = Payload::deserialize(deserializer)?;
        Ok(ToolLlmErrorPayload {
            ok: payload.ok,
            tool: payload.tool,
            code: payload.code,
            field: payload.field,
            expected: payload.expected,
            received: payload.received,
            message: payload.message,
            snippet: payload.snippet,
            retry_hint: payload.retry_hint,
            retry_context: payload.retry_context,
            index: ToolLlmErrorPayloadIndex::default(),
        }
        .with_index())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct ToolLlmErrorPayloadIndex {
    fields: BTreeMap<String, ToolLlmErrorValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolLlmErrorValue {
    Null,
    Bool(bool),
    String(String),
    U64(u64),
    Object(BTreeMap<String, ToolLlmErrorValue>),
}

impl ToolLlmErrorPayload {
    fn with_index(mut self) -> Self {
        self.rebuild_index();
        self
    }

    fn rebuild_index(&mut self) {
        let mut fields = BTreeMap::new();
        fields.insert("ok".to_string(), ToolLlmErrorValue::Bool(self.ok));
        fields.insert(
            "tool".to_string(),
            ToolLlmErrorValue::String(self.tool.as_str().to_string()),
        );
        fields.insert(
            "code".to_string(),
            ToolLlmErrorValue::String(format!("{:?}", self.code)),
        );
        fields.insert("field".to_string(), option_string_value(self.field.clone()));
        fields.insert(
            "expected".to_string(),
            option_string_value(self.expected.clone()),
        );
        fields.insert(
            "received".to_string(),
            option_string_value(self.received.clone()),
        );
        fields.insert(
            "message".to_string(),
            ToolLlmErrorValue::String(self.message.clone()),
        );
        fields.insert(
            "snippet".to_string(),
            option_string_value(self.snippet.clone()),
        );
        fields.insert(
            "retry_hint".to_string(),
            option_string_value(self.retry_hint.clone()),
        );
        fields.insert(
            "retry_context".to_string(),
            self.retry_context
                .as_ref()
                .map(retry_context_value)
                .unwrap_or(ToolLlmErrorValue::Null),
        );
        self.index = ToolLlmErrorPayloadIndex { fields };
    }
}

impl ToolLlmErrorValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, ToolLlmErrorValue>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U64(value) => Some(*value),
            Self::String(value) => value.parse().ok(),
            _ => None,
        }
    }
}

impl Index<&str> for ToolLlmErrorPayload {
    type Output = ToolLlmErrorValue;

    fn index(&self, index: &str) -> &Self::Output {
        &self.index.fields[index]
    }
}

fn option_string_value(value: Option<String>) -> ToolLlmErrorValue {
    value
        .map(ToolLlmErrorValue::String)
        .unwrap_or(ToolLlmErrorValue::Null)
}

fn retry_context_value(context: &ToolRetryContext) -> ToolLlmErrorValue {
    let fields = context
        .fields
        .iter()
        .map(|field| (field.name.clone(), retry_context_field_value(&field.value)))
        .collect();
    ToolLlmErrorValue::Object(fields)
}

fn retry_context_field_value(value: &ToolRetryContextValue) -> ToolLlmErrorValue {
    match value {
        ToolRetryContextValue::Null => ToolLlmErrorValue::Null,
        ToolRetryContextValue::Bool(value) => ToolLlmErrorValue::Bool(*value),
        ToolRetryContextValue::Number(value) => value
            .parse()
            .map(ToolLlmErrorValue::U64)
            .unwrap_or_else(|_| ToolLlmErrorValue::String(value.clone())),
        ToolRetryContextValue::String(value) => ToolLlmErrorValue::String(value.clone()),
        ToolRetryContextValue::StringList(value) => ToolLlmErrorValue::Object(
            value
                .iter()
                .enumerate()
                .map(|(idx, item)| (idx.to_string(), ToolLlmErrorValue::String(item.clone())))
                .collect(),
        ),
    }
}

/// Payload carried over event bus when a tool fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolErrorWire {
    pub user: String,
    pub llm: ToolLlmErrorPayload,
    pub system: String,
}

impl ToolErrorWire {
    pub fn parse(s: &str) -> Option<Self> {
        let mut wire: Self = serde_json::from_str(s).ok()?;
        wire.llm.rebuild_index();
        Some(wire)
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

/// Convenience for tools that need to surface a user-facing error.
pub fn tool_ui_error(message: impl Into<String>) -> ploke_error::Error {
    ploke_error::Error::Domain(ploke_error::DomainError::Ui {
        message: message.into(),
    })
}

/// Convenience for tools that need to surface an IO error.
pub fn tool_io_error(message: impl Into<String>) -> ploke_error::Error {
    ploke_error::Error::Domain(ploke_error::DomainError::Io {
        message: message.into(),
    })
}
