use crate::{
    config::{McpConfig, ServerSpec},
    types::{McpError, ServerId},
};
use rmcp::{
    service::{RunningService, ServiceExt},
    transport::{child_process::TokioChildProcess, ConfigureCommandExt},
    RoleClient,
};
use std::{collections::HashMap, sync::Arc};
use tokio::{process::Command, sync::Mutex};

/// Orchestrates lifecycle (start/stop/cancel) for multiple MCP servers.
pub struct McpManager {
    cfg: McpConfig,
    registry: Arc<Mutex<HashMap<ServerId, RunningService<RoleClient, ()>>>>,
}

impl McpManager {
    /// Create a manager from a configuration. Does not start any servers yet.
    pub async fn from_config(cfg: McpConfig) -> Result<Self, McpError> {
        Ok(Self {
            cfg,
            registry: Arc::new(Mutex::new(HashMap::new())),
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
        let mut guard = self.registry.lock().await;
        guard.insert(id.clone(), service);
        Ok(())
    }

    /// Cancel (stop) a running server. Returns NotFound if not running.
    pub async fn cancel(&self, id: &ServerId) -> Result<(), McpError> {
        // Remove from registry to drop it after cancel
        let service = {
            let mut guard = self.registry.lock().await;
            guard.remove(id)
        };

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
        // Take ownership of all services to avoid holding lock across awaits.
        let services: Vec<(ServerId, RunningService<RoleClient, ()>)> = {
            let mut guard = self.registry.lock().await;
            let map = std::mem::take(&mut *guard);
            map.into_iter().collect()
        };

        for (id, svc) in services {
            if let Err(e) = svc.cancel().await {
                // Best-effort: continue canceling others
                eprintln!("Warning: cancel failed for '{}': {}", id, e);
            }
        }
        Ok(())
    }

    /// Check if a server is currently running.
    pub async fn is_running(&self, id: &ServerId) -> bool {
        let guard = self.registry.lock().await;
        guard.contains_key(id)
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
