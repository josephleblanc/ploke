use crate::{
    config::{McpConfig, ServerSpec},
    types::{McpError, ServerId, ToolDescriptor, ToolResult},
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
use std::time::Duration;
use tracing::{debug, error, info, warn};

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
    #[tracing::instrument(skip(self), fields(server_id = %id))]
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
        info!("Starting server");
        let service = Self::spawn_service(spec).await?;

        // Insert into registry
        self.registry.insert(id.clone(), service);
        info!("Server started");
        Ok(())
    }

    /// Cancel (stop) a running server. Returns NotFound if not running.
    #[tracing::instrument(skip(self), fields(server_id = %id))]
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
    #[tracing::instrument(skip(self))]
    pub async fn cancel_all(&self) -> Result<(), McpError> {
        // Collect keys first to avoid holding references across awaits.
        let keys: Vec<ServerId> = self.registry.iter().map(|kv| kv.key().clone()).collect();

        for id in keys {
            if let Some((_, svc)) = self.registry.remove(&id) {
                if let Err(e) = svc.cancel().await {
                    // Best-effort: continue canceling others
                    warn!("Cancel failed for '{}': {}", id, e);
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
    #[tracing::instrument(skip(self, args), fields(server_id = %id, tool = %name))]
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

        // Invoke the tool with a conservative timeout
        debug!("Calling tool '{}'", name);
        let fut = svc.call_tool(CallToolRequestParam {
            name: name.to_string().into(),
            arguments: args.as_object().cloned(),
        });
        let timeout_ms = self
            .cfg
            .get(id)
            .and_then(|s| s.default_timeout_ms)
            .unwrap_or(30_000);
        let result = tokio::time::timeout(Duration::from_millis(timeout_ms), fut)
            .await
            .map_err(|_| McpError::Timeout)?
            .map_err(|e| McpError::Tool(format!("call_tool '{}' on '{}' failed: {}", name, id, e)))?;

        // Aggregate all text content blocks into a single string
        let text = result
            .content
            .into_iter()
            .filter_map(|c| c.as_text().map(|t| t.to_owned().text))
            .join("\n");

        debug!("Tool '{}' returned {} bytes of text", name, text.len());
        Ok(ToolResult { text })
    }

    /// Map rmcp list_tools JSON Value into ToolDescriptor list.
    fn map_tools_from_value(val: &serde_json::Value) -> Result<Vec<ToolDescriptor>, McpError> {
        let tools_val = val.get("tools").and_then(|v| v.as_array()).ok_or_else(|| {
            McpError::Protocol("Unexpected list_tools response shape: missing 'tools' array".into())
        })?;
        let tools = tools_val
            .iter()
            .filter_map(|t| {
                let name = t.get("name").and_then(|n| n.as_str())?;
                let desc = t.get("description").and_then(|d| d.as_str()).map(|s| s.to_string());
                Some(ToolDescriptor {
                    name: name.to_string(),
                    description: desc,
                })
            })
            .collect::<Vec<_>>();
        Ok(tools)
    }

    /// List available tools on a running server.
    #[tracing::instrument(skip(self), fields(server_id = %id))]
    pub async fn list_tools(&self, id: &ServerId) -> Result<Vec<ToolDescriptor>, McpError> {
        // Ensure the server is started
        self.ensure_started(id).await?;
        let svc = self
            .registry
            .get(id)
            .ok_or_else(|| McpError::NotFound(format!("Server '{}' is not running", id)))?;

        // Query tools
        let resp = svc
            .list_tools(Default::default())
            .await
            .map_err(|e| McpError::Protocol(format!("list_tools on '{}' failed: {}", id, e)))?;

        // Map response to our ToolDescriptor using a serde_json bridge for resilience
        let val = serde_json::to_value(&resp)
            .map_err(|e| McpError::Protocol(format!("Failed to serialize list_tools response: {}", e)))?;
        let tools = Self::map_tools_from_value(&val)?;
        Ok(tools)
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

    /// Start all servers marked as `autostart = true`, honoring `priority` (lower starts first).
    #[tracing::instrument(skip(self))]
    pub async fn start_autostart(&self) -> Result<(), McpError> {
        let mut specs: Vec<&ServerSpec> = self.cfg.servers.iter().filter(|s| s.autostart).collect();
        specs.sort_by_key(|s| s.priority);

        for spec in specs {
            if !self.is_running(&spec.id).await {
                let service = Self::spawn_service(spec).await?;
                self.registry.insert(spec.id.clone(), service);
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(spec), fields(server_id = %spec.id, command = %spec.command))]
    async fn spawn_service(spec: &ServerSpec) -> Result<RunningService<RoleClient, ()>, McpError> {
        debug!("Launching '{}' with args {:?} and {} env vars", spec.command, spec.args, spec.env.len());
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

        info!("Connected to server process");
        Ok(service)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_tools_basic() {
        let val = serde_json::json!({
            "tools": [
                { "name": "git_status", "description": "List status" },
                { "name": "git_diff" }
            ]
        });
        let tools = McpManager::map_tools_from_value(&val).expect("ok");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "git_status");
        assert_eq!(tools[0].description.as_deref(), Some("List status"));
        assert_eq!(tools[1].name, "git_diff");
        assert!(tools[1].description.is_none());
    }
}
