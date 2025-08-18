use std::fmt;

/// Strongly-typed identifier for an MCP server instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerId(pub String);

impl ServerId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ServerId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for ServerId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for ServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Unified error type for MCP client operations.
#[derive(Debug)]
pub enum McpError {
    Transport(String),
    Protocol(String),
    Tool(String),
    Spawn(String),
    Timeout,
    Canceled,
    NotFound(String),
    Config(String),
    Io(std::io::Error),
    Anyhow(anyhow::Error),
}

impl From<std::io::Error> for McpError {
    fn from(value: std::io::Error) -> Self {
        McpError::Io(value)
    }
}

impl From<anyhow::Error> for McpError {
    fn from(value: anyhow::Error) -> Self {
        McpError::Anyhow(value)
    }
}
