use crate::{
    config::{McpConfig, ServerSpec},
    types::{McpError, ServerId, ToolDescriptor, ToolResult},
};
use dashmap::DashMap;
use itertools::Itertools;
use rand::Rng;
use rmcp::model::CallToolRequestParam;
use rmcp::{
    RoleClient,
    service::{RunningService, ServiceExt},
    transport::{ConfigureCommandExt, child_process::TokioChildProcess},
};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tracing::{debug, error, info, warn};
use which::which;

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
        if self.is_running(id) {
            return Ok(());
        }

        // Find spec
        let Some(spec) = self.cfg.get(id) else {
            return Err(McpError::NotFound(format!(
                "No server spec found for id '{}'",
                id
            )));
        };

        // Start with bounded exponential backoff and health check
        info!("Starting server");
        let service = Self::spawn_with_backoff(spec, id).await?;
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
    pub fn is_running(&self, id: &ServerId) -> bool {
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
        let timeout_ms = self
            .cfg
            .get(id)
            .and_then(|s| s.default_timeout_ms)
            .unwrap_or(30_000);
        self.do_call_tool(id, name, args, Duration::from_millis(timeout_ms))
            .await
    }

    /// Call a tool with an explicit timeout, overriding any configured default.
    #[tracing::instrument(skip(self, args, timeout), fields(server_id = %id, tool = %name))]
    pub async fn call_tool_with_timeout(
        &self,
        id: &ServerId,
        name: &str,
        args: serde_json::Value,
        timeout: Duration,
    ) -> Result<ToolResult, McpError> {
        self.do_call_tool(id, name, args, timeout).await
    }

    /// Internal helper to perform a tool call with the provided timeout.
    async fn do_call_tool(
        &self,
        id: &ServerId,
        name: &str,
        args: serde_json::Value,
        timeout: Duration,
    ) -> Result<ToolResult, McpError> {
        // Ensure the server is started
        self.ensure_started(id).await?;

        let restart_on_exit = self.cfg.get(id).map(|s| s.restart_on_exit).unwrap_or(false);

        let start_total = Instant::now();
        let max_attempts = if restart_on_exit { 2 } else { 1 };
        let mut attempt = 0usize;

        loop {
            // Compute remaining timeout budget
            let elapsed = start_total.elapsed();
            let remaining = if elapsed >= timeout {
                Duration::from_millis(0)
            } else {
                timeout - elapsed
            };
            if remaining.is_zero() {
                return Err(McpError::Timeout);
            }

            // Get a handle to the running service (drop guard before potential respawn)
            let result = {
                let svc = {
                    let svc_guard = self.registry.get(id).ok_or_else(|| {
                        McpError::NotFound(format!("Server '{}' is not running", id))
                    })?;
                    svc_guard.clone()
                };

                debug!("Calling tool '{}', attempt {}", name, attempt + 1);
                let start = Instant::now();
                let fut = svc.call_tool(CallToolRequestParam {
                    name: name.to_string().into(),
                    arguments: args.as_object().cloned(),
                });

                tokio::time::timeout(remaining, fut)
                    .await
                    .map_err(|_| McpError::Timeout)
                    .and_then(|r| {
                        r.map_err(|e| {
                            McpError::Tool(format!(
                                "call_tool '{}' on '{}' failed: {}",
                                name, id, e
                            ))
                        })
                    })
                    .map(|res| {
                        let text = res
                            .content
                            .into_iter()
                            .filter_map(|c| c.as_text().map(|t| t.to_owned().text))
                            .join("\n");
                        let elapsed_ms = start.elapsed().as_millis();
                        debug!(
                            "Tool '{}' returned {} bytes of text in {} ms",
                            name,
                            text.len(),
                            elapsed_ms
                        );
                        ToolResult { text }
                    })
            };

            match result {
                Ok(out) => return Ok(out),
                Err(err) => {
                    attempt += 1;
                    if attempt >= max_attempts {
                        return Err(err);
                    }
                    // Try to respawn and retry once if allowed
                    warn!(
                        "Tool call failed; attempting respawn of '{}' and retry once: {}",
                        id,
                        format!("{err:?}")
                    );
                    if let Some(spec) = self.cfg.get(id).cloned() {
                        // Best-effort respawn with backoff
                        if let Err(respawn_err) = self.respawn_with_backoff(id, &spec).await {
                            error!("Respawn failed for '{}': {}", id, respawn_err);
                            // Give up and return original error
                            return Err(err);
                        }
                        // Loop to retry with remaining timeout
                        continue;
                    } else {
                        return Err(err);
                    }
                }
            }
        }
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
                let desc = t
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string());
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

        let restart_on_exit = self.cfg.get(id).map(|s| s.restart_on_exit).unwrap_or(false);
        let max_attempts = if restart_on_exit { 2 } else { 1 };
        let mut attempt = 0usize;

        loop {
            let result = {
                let svc = {
                    let svc_guard = self.registry.get(id).ok_or_else(|| {
                        McpError::NotFound(format!("Server '{}' is not running", id))
                    })?;
                    svc_guard.clone()
                };

                let fut = svc.list_tools(Default::default());

                fut.await.map_err(|e| {
                    McpError::Protocol(format!("list_tools on '{}' failed: {}", id, e))
                })
            };

            match result {
                Ok(resp) => {
                    // Map response to our ToolDescriptor using a serde_json bridge for resilience
                    let val = serde_json::to_value(&resp).map_err(|e| {
                        McpError::Protocol(format!(
                            "Failed to serialize list_tools response: {}",
                            e
                        ))
                    })?;
                    let tools = Self::map_tools_from_value(&val)?;
                    return Ok(tools);
                }
                Err(err) => {
                    attempt += 1;
                    if attempt >= max_attempts {
                        return Err(err);
                    }
                    warn!(
                        "list_tools failed; attempting respawn of '{}' and retry once: {}",
                        id,
                        format!("{err:?}")
                    );
                    if let Some(spec) = self.cfg.get(id).cloned() {
                        if let Err(respawn_err) = self.respawn_with_backoff(id, &spec).await {
                            error!("Respawn failed for '{}': {}", id, respawn_err);
                            return Err(err);
                        }
                        continue;
                    } else {
                        return Err(err);
                    }
                }
            }
        }
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
            if !self.is_running(&spec.id) {
                let service = Self::spawn_with_backoff(spec, &spec.id).await?;
                self.registry.insert(spec.id.clone(), service);
            }
        }
        Ok(())
    }

    /// Perform a basic health check on a newly started service by calling list_tools within a timeout.
    async fn health_check(
        id: &ServerId,
        svc: &RunningService<RoleClient, ()>,
        timeout: Duration,
    ) -> Result<(), McpError> {
        debug!("Health checking '{}'", id);
        let fut = svc.list_tools(Default::default());
        tokio::time::timeout(timeout, fut)
            .await
            .map_err(|_| McpError::Timeout)
            .and_then(|r| {
                r.map(|_| ()).map_err(|e| {
                    McpError::Protocol(format!("health_check list_tools on '{}' failed: {}", id, e))
                })
            })
    }

    /// Spawn a service with bounded exponential backoff and jitter, including an initial health check.
    async fn spawn_with_backoff(
        spec: &ServerSpec,
        id: &ServerId,
    ) -> Result<RunningService<RoleClient, ()>, McpError> {
        let mut attempt: u32 = 0;
        let max_attempts: u32 = 5;
        let mut last_err: Option<McpError> = None;

        loop {
            match Self::spawn_service(spec).await {
                Ok(service) => {
                    // Health check (list_tools) with a short timeout to verify responsiveness
                    let hc_timeout =
                        Duration::from_millis(spec.default_timeout_ms.unwrap_or(30_000).min(5_000));
                    match Self::health_check(id, &service, hc_timeout).await {
                        Ok(()) => return Ok(service),
                        Err(e) => {
                            // Best-effort cancel on failed health check and retry with backoff
                            let _ = service.cancel().await;
                            warn!(
                                "Health check failed for '{}': {}. Will retry with backoff.",
                                id, e
                            );
                            last_err = Some(e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Spawn failed for '{}': {}. Will retry with backoff.", id, e);
                    last_err = Some(e);
                }
            }

            attempt += 1;
            if attempt >= max_attempts {
                return Err(last_err
                    .unwrap_or_else(|| McpError::Spawn(format!("Failed to start '{}'", id))));
            }

            // Exponential backoff with jitter: base 500ms, cap ~8s
            let base = 500u64;
            let backoff_ms = (base.saturating_mul(1u64 << attempt.min(4))) // 500,1_000,2_000,4_000,8_000
                .min(8_000);
            let jitter: u64 = rand::thread_rng().gen_range(0..=250);
            let sleep_ms = backoff_ms + jitter;
            debug!(
                "Backoff sleeping for {} ms before retrying '{}'",
                sleep_ms, id
            );
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }
    }

    /// Best-effort respawn: cancel existing (if any), then spawn_with_backoff and insert.
    async fn respawn_with_backoff(&self, id: &ServerId, spec: &ServerSpec) -> Result<(), McpError> {
        if let Some((_, old)) = self.registry.remove(id) {
            let _ = old.cancel().await;
        }
        let service = Self::spawn_with_backoff(spec, id).await?;
        self.registry.insert(id.clone(), service);
        Ok(())
    }

    #[tracing::instrument(skip(spec), fields(server_id = %spec.id, command = %spec.command))]
    async fn spawn_service(spec: &ServerSpec) -> Result<RunningService<RoleClient, ()>, McpError> {
        debug!(
            "Launching '{}' with args {:?} and {} env vars",
            spec.command,
            spec.args,
            spec.env.len()
        );
        // Preflight: if command is not a path, ensure it exists on PATH for clearer errors
        if !spec.command.contains(std::path::MAIN_SEPARATOR) {
            if which::which(&spec.command).is_err() {
                return Err(McpError::Spawn(format!(
                    "Command '{}' not found on PATH",
                    spec.command
                )));
            }
        }
        let child = TokioChildProcess::new(Command::new(&spec.command).configure(|cmd| {
            if !spec.args.is_empty() {
                cmd.args(&spec.args);
            }
            if !spec.env.is_empty() {
                for (k, v) in &spec.env {
                    cmd.env(k, v);
                }
            }
        }))
        .map_err(|e| McpError::Spawn(format!("Failed to launch '{}': {}", spec.command, e)))?;

        let service = ().serve(child).await.map_err(|e| {
            McpError::Transport(format!("Failed to connect to '{}': {}", spec.id, e))
        })?;

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
