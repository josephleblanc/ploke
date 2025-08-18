use crate::{
    config::{McpConfig, ServerSpec},
    types::{McpError, ServerId},
};
use rmcp::{
    service::{RunningService, ServiceExt},
    transport::{child_process::TokioChildProcess, ConfigureCommandExt},
    RoleClient,
};
use rmcp::model::CallToolRequestParam;
use dashmap::DashMap;
use itertools::Itertools;
use tokio::process::Command;
use crate::types::ToolResult;

/// Orchestrates lifecycle (start/stop/cancel) for multiple MCP servers.
pub struct McpManager {
    cfg: McpConfig,
    registry: DashMap<ServerId, RunningService<RoleClient, ()>>,
}

impl McpManager {
    /// Create a manager from a configuration. Does not start any servers yet.
    pub async fn from_config(cfg: McpConfig) -> Result<Self, McpError> {
        Ok(Self {
            cfg,
            registry: DashMap::new(),
        })
    }

    /// Ensure a server with the given id is started. No-op if already running.
    pub async fn ensure_started(&self, id: &ServerId) -> Result<(), McpError> {
        // Fast path: already started
        if self.is_running(id).await {
            return Ok(());
        }

        // Find spec
        let Some(spec) = self.cfg.get(id) else {
            return Err(McpError::NotFound(format!(
                "No server spec found for id '{}'",
                id
            )));
        };

        // Start without holding the lock to avoid holding across .await
        let service = Self::spawn_service(spec).await?;

        // Insert into registry
        self.registry.insert(id.clone(), service);
        Ok(())
    }

    /// Cancel (stop) a running server. Returns NotFound if not running.
    pub async fn cancel(&self, id: &ServerId) -> Result<(), McpError> {
        // Remove from registry to drop it after cancel
        let service = self.registry.remove(id).map(|(_, svc)| svc);

        match service {
            Some(svc) => {
                svc.cancel().await.map_err(|e| {
                    McpError::Transport(format!("Cancel failed for '{}': {}", id, e))
                })?;
                Ok(())
            }
            None => Err(McpError::NotFound(format!(
                "Server '{}' is not running",
                id
            ))),
        }
    }

    /// Cancel all running servers.
    pub async fn cancel_all(&self) -> Result<(), McpError> {
        // Collect keys first to avoid holding references across awaits.
        let keys: Vec<ServerId> = self.registry.iter().map(|kv| kv.key().clone()).collect();

        for id in keys {
            if let Some((_, svc)) = self.registry.remove(&id) {
                if let Err(e) = svc.cancel().await {
                    // Best-effort: continue canceling others
                    eprintln!("Warning: cancel failed for '{}': {}", id, e);
                }
            }
        }
        Ok(())
    }

    /// Check if a server is currently running.
    pub async fn is_running(&self, id: &ServerId) -> bool {
        self.registry.contains_key(id)
    }

    /// Call a tool on a server and aggregate textual content blocks.
    pub async fn call_tool(
        &self,
        id: &ServerId,
        name: &str,
        args: serde_json::Value,
    ) -> Result<ToolResult, McpError> {
        // Ensure the server is started
        self.ensure_started(id).await?;

        // Get a handle to the running service
        let svc = self
            .registry
            .get(id)
            .ok_or_else(|| McpError::NotFound(format!("Server '{}' is not running", id)))?;

        // Invoke the tool
        let result = svc
            .call_tool(CallToolRequestParam {
                name: name.to_string(),
                arguments: args.as_object().cloned(),
            })
            .await
            .map_err(|e| McpError::Tool(format!("call_tool '{}' on '{}' failed: {}", name, id, e)))?;

        // Aggregate all text content blocks into a single string
        let text = result
            .content
            .into_iter()
            .filter_map(|c| c.as_text().map(|t| t.to_owned().text))
            .join("\n");

        Ok(ToolResult { text })
    }

    /// Convenience: get a typed Context7 client if configured.
    pub fn client_context7(&self) -> Option<crate::clients::context7::Context7Client<'_>> {
        let id: ServerId = "context7".into();
        if self.cfg.get(&id).is_some() {
            Some(crate::clients::context7::Context7Client::new(self))
        } else {
            None
        }
    }

    /// Convenience: get a typed Git client if configured.
    pub fn client_git(&self) -> Option<crate::clients::git::GitClient<'_>> {
        let id: ServerId = "git".into();
        if self.cfg.get(&id).is_some() {
            Some(crate::clients::git::GitClient::new(self))
        } else {
            None
        }
    }

    async fn spawn_service(spec: &ServerSpec) -> Result<RunningService<RoleClient, ()>, McpError> {
        let child = TokioChildProcess::new(
            Command::new(&spec.command).configure(|cmd| {
                if !spec.args.is_empty() {
                    cmd.args(&spec.args);
                }
                if !spec.env.is_empty() {
                    for (k, v) in &spec.env {
                        cmd.env(k, v);
                    }
                }
            }),
        )
        .map_err(|e| McpError::Spawn(format!("Failed to launch '{}': {}", spec.command, e)))?;

        let service = ()
            .serve(child)
            .await
            .map_err(|e| McpError::Transport(format!("Failed to connect to '{}': {}", spec.id, e)))?;

        Ok(service)
    }
}
