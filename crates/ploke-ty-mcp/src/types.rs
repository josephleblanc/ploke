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

#[derive(Debug, Clone)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub text: String,
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

/// Async traits to decouple typed clients from their backends.
/// These enable swapping MCP-backed clients for native implementations later.
#[async_trait::async_trait]
pub trait GitOps: Send + Sync {
    async fn status(&self, repo_path: &str) -> Result<String, McpError>;
    async fn diff(&self, repo_path: &str, args: Option<Vec<String>>) -> Result<String, McpError>;
    async fn add(&self, repo_path: &str, paths: Vec<String>) -> Result<String, McpError>;
    async fn commit(&self, repo_path: &str, message: &str, all: bool) -> Result<String, McpError>;
    async fn checkout(&self, repo_path: &str, branch: &str, create: bool) -> Result<String, McpError>;
    async fn branch(&self, repo_path: &str) -> Result<String, McpError>;
}

#[async_trait::async_trait]
pub trait DocsLookup: Send + Sync {
    async fn resolve_library_id(&self, name: &str) -> Result<String, McpError>;
    async fn get_library_docs(&self, id: &str, tokens: usize, topic: &str) -> Result<String, McpError>;
}
