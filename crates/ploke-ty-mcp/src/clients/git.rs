use crate::{
    manager::McpManager,
    types::{McpError, ServerId},
};
use serde_json::json;

/// Typed client for the Git MCP server.
pub struct GitClient<'a> {
    mgr: &'a McpManager,
    id: ServerId,
}

impl<'a> GitClient<'a> {
    pub fn new(mgr: &'a McpManager) -> Self {
        Self {
            mgr,
            id: ServerId("git".to_string()),
        }
    }

    /// Get `git status` for a repository.
    pub async fn status(&self, repo_path: &str) -> Result<String, McpError> {
        self.mgr.ensure_started(&self.id).await?;
        let result = self
            .mgr
            .call_tool(&self.id, "git_status", json!({ "repo_path": repo_path }))
            .await?;
        Ok(result.text)
    }
}
