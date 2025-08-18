use crate::{
    manager::McpManager,
    types::{McpError, ServerId},
};
use serde_json::json;

/// Typed client for the Context7 MCP server.
pub struct Context7Client<'a> {
    mgr: &'a McpManager,
    id: ServerId,
}

impl<'a> Context7Client<'a> {
    pub fn new(mgr: &'a McpManager) -> Self {
        Self {
            mgr,
            id: ServerId("context7".to_string()),
        }
    }

    /// Resolve a library/package name (e.g., "bevy") to a Context7-compatible ID (e.g., "/bevyengine/bevy").
    pub async fn resolve_library_id(&self, name: &str) -> Result<String, McpError> {
        self.mgr.ensure_started(&self.id).await?;
        let result = self
            .mgr
            .call_tool(&self.id, "resolve-library-id", json!({ "libraryName": name }))
            .await?;
        Ok(result.text)
    }

    /// Fetch documentation for a given Context7-compatible library ID.
    pub async fn get_library_docs(&self, id: &str, tokens: usize, topic: &str) -> Result<String, McpError> {
        self.mgr.ensure_started(&self.id).await?;
        let result = self
            .mgr
            .call_tool(
                &self.id,
                "get-library-docs",
                json!({
                    "context7CompatibleLibraryID": id,
                    "tokens": tokens,
                    "topic": topic
                }),
            )
            .await?;
        Ok(result.text)
    }
}
