use crate::types::ServerId;
use std::collections::BTreeMap;

/// Specification for starting and managing a single MCP server.
#[derive(Debug, Clone)]
pub struct ServerSpec {
    pub id: ServerId,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub autostart: bool,
    pub restart_on_exit: bool,
    /// Lower value = higher priority
    pub priority: u8,
}

/// Top-level configuration listing all known servers.
#[derive(Debug, Clone, Default)]
pub struct McpConfig {
    pub servers: Vec<ServerSpec>,
}

impl McpConfig {
    /// Get a server spec by id.
    pub fn get(&self, id: &ServerId) -> Option<&ServerSpec> {
        self.servers.iter().find(|s| &s.id == id)
    }
}
