use std::collections::BTreeMap;
use std::ops::Index;

use serde::{Deserialize, Deserializer, Serialize};

use crate::tool_types::ToolName;

/// Bounded retry context attached to tool errors.
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
    pub fn from_parts(
        tool: ToolName,
        code: ToolErrorCode,
        field: Option<String>,
        expected: Option<String>,
        received: Option<String>,
        message: String,
        snippet: Option<String>,
        retry_hint: Option<String>,
        retry_context: Option<ToolRetryContext>,
    ) -> Self {
        Self {
            ok: false,
            tool,
            code,
            field,
            expected,
            received,
            message,
            snippet,
            retry_hint,
            retry_context,
            index: ToolLlmErrorPayloadIndex::default(),
        }
        .with_index()
    }

    fn with_index(mut self) -> Self {
        self.rebuild_index();
        self
    }

    pub fn rebuild_index(&mut self) {
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

/// Payload carried when a tool fails.
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
