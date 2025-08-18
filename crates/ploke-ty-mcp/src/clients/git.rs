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

    /// Get `git diff` for a repository. Optional args mirror CLI flags (e.g., ["--staged"]).
    pub async fn diff(&self, repo_path: &str, args: Option<Vec<String>>) -> Result<String, McpError> {
        self.mgr.ensure_started(&self.id).await?;
        let mut payload = serde_json::Map::new();
        payload.insert("repo_path".to_string(), json!(repo_path));
        if let Some(a) = args {
            payload.insert("args".to_string(), json!(a));
        }
        let result = self
            .mgr
            .call_tool(&self.id, "git_diff", serde_json::Value::Object(payload))
            .await?;
        Ok(result.text)
    }

    /// Run `git add` for a list of paths.
    pub async fn add(&self, repo_path: &str, paths: Vec<String>) -> Result<String, McpError> {
        self.mgr.ensure_started(&self.id).await?;
        let result = self
            .mgr
            .call_tool(
                &self.id,
                "git_add",
                json!({
                    "repo_path": repo_path,
                    "paths": paths,
                }),
            )
            .await?;
        Ok(result.text)
    }

    /// Run `git commit` with a message. If `all` is true, includes `-a`.
    pub async fn commit(&self, repo_path: &str, message: &str, all: bool) -> Result<String, McpError> {
        self.mgr.ensure_started(&self.id).await?;
        let result = self
            .mgr
            .call_tool(
                &self.id,
                "git_commit",
                json!({
                    "repo_path": repo_path,
                    "message": message,
                    "all": all,
                }),
            )
            .await?;
        Ok(result.text)
    }

    /// Run `git checkout` to switch or create a branch.
    pub async fn checkout(&self, repo_path: &str, branch: &str, create: bool) -> Result<String, McpError> {
        self.mgr.ensure_started(&self.id).await?;
        let result = self
            .mgr
            .call_tool(
                &self.id,
                "git_checkout",
                json!({
                    "repo_path": repo_path,
                    "branch": branch,
                    "create": create,
                }),
            )
            .await?;
        Ok(result.text)
    }

    /// List or show the current branch using `git branch`.
    pub async fn branch(&self, repo_path: &str) -> Result<String, McpError> {
        self.mgr.ensure_started(&self.id).await?;
        let result = self
            .mgr
            .call_tool(&self.id, "git_branch", json!({ "repo_path": repo_path }))
            .await?;
        Ok(result.text)
    }
}

#[async_trait::async_trait]
impl<'a> crate::types::GitOps for GitClient<'a> {
    async fn status(&self, repo_path: &str) -> Result<String, McpError> {
        GitClient::status(self, repo_path).await
    }

    async fn diff(&self, repo_path: &str, args: Option<Vec<String>>) -> Result<String, McpError> {
        GitClient::diff(self, repo_path, args).await
    }

    async fn add(&self, repo_path: &str, paths: Vec<String>) -> Result<String, McpError> {
        GitClient::add(self, repo_path, paths).await
    }

    async fn commit(&self, repo_path: &str, message: &str, all: bool) -> Result<String, McpError> {
        GitClient::commit(self, repo_path, message, all).await
    }

    async fn checkout(&self, repo_path: &str, branch: &str, create: bool) -> Result<String, McpError> {
        GitClient::checkout(self, repo_path, branch, create).await
    }

    async fn branch(&self, repo_path: &str) -> Result<String, McpError> {
        GitClient::branch(self, repo_path).await
    }
}
