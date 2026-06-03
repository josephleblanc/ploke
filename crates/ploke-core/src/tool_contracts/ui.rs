use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ArcStr;
use crate::tool_types::ToolName;

use super::error::{ToolErrorCode, ToolErrorWire};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolVerbosity {
    Minimal,
    Normal,
    Verbose,
}

impl Default for ToolVerbosity {
    fn default() -> Self {
        ToolVerbosity::Normal
    }
}

impl ToolVerbosity {
    pub fn as_str(self) -> &'static str {
        match self {
            ToolVerbosity::Minimal => "minimal",
            ToolVerbosity::Normal => "normal",
            ToolVerbosity::Verbose => "verbose",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolUiField {
    pub name: ArcStr,
    pub value: ArcStr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUiPayload {
    pub tool: ToolName,
    pub call_id: ArcStr,
    pub request_id: Option<Uuid>,
    pub proposal_id: Option<Uuid>,
    pub summary: String,
    pub fields: Vec<ToolUiField>,
    pub details: Option<String>,
    pub verbosity: ToolVerbosity,
    pub error: Option<ToolErrorWire>,
    pub error_code: Option<ToolErrorCode>,
}

impl ToolUiPayload {
    pub fn new(tool: ToolName, call_id: ArcStr, summary: impl Into<String>) -> Self {
        Self {
            tool,
            call_id,
            request_id: None,
            proposal_id: None,
            summary: summary.into(),
            fields: Vec::new(),
            details: None,
            verbosity: ToolVerbosity::Normal,
            error: None,
            error_code: None,
        }
    }

    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn with_proposal_id(mut self, proposal_id: Uuid) -> Self {
        self.proposal_id = Some(proposal_id);
        self
    }

    pub fn with_field(mut self, name: impl Into<ArcStr>, value: impl Into<ArcStr>) -> Self {
        self.fields.push(ToolUiField {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn with_verbosity(mut self, verbosity: ToolVerbosity) -> Self {
        self.verbosity = verbosity;
        self
    }

    pub fn render(&self, verbosity: ToolVerbosity) -> String {
        match verbosity {
            ToolVerbosity::Minimal => {
                format!("Tool: {} — {}", self.tool.as_str(), self.summary)
            }
            ToolVerbosity::Normal => {
                let mut out = String::new();
                out.push_str(&format!("Tool: {}\n", self.tool.as_str()));
                out.push_str(&format!("Summary: {}\n", self.summary));
                if !self.fields.is_empty() {
                    out.push_str("Fields:\n");
                    for field in &self.fields {
                        out.push_str(&format!("- {}: {}\n", field.name, field.value));
                    }
                }
                out.trim_end().to_string()
            }
            ToolVerbosity::Verbose => {
                let mut out = String::new();
                out.push_str(&format!("Tool: {}\n", self.tool.as_str()));
                out.push_str(&format!("Summary: {}\n", self.summary));
                if !self.fields.is_empty() {
                    out.push_str("Fields:\n");
                    for field in &self.fields {
                        out.push_str(&format!("- {}: {}\n", field.name, field.value));
                    }
                }
                if let Some(details) = &self.details {
                    out.push_str("Details:\n");
                    out.push_str(details);
                    out.push('\n');
                }
                out.trim_end().to_string()
            }
        }
    }
}

pub fn tool_error_code_label(code: ToolErrorCode) -> &'static str {
    match code {
        ToolErrorCode::FieldTooLarge => "field_too_large",
        ToolErrorCode::WrongType => "wrong_type",
        ToolErrorCode::MissingField => "missing_field",
        ToolErrorCode::MalformedDiff => "malformed_diff",
        ToolErrorCode::InvalidFormat => "invalid_format",
        ToolErrorCode::Io => "io",
        ToolErrorCode::Timeout => "timeout",
        ToolErrorCode::Internal => "internal",
    }
}
