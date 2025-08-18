
pub mod config;
pub mod manager;
pub mod types;
pub mod clients;
pub mod util;

pub use config::{McpConfig, ServerSpec};
pub use manager::McpManager;
pub use clients::{context7::Context7Client, git::GitClient};
pub use types::{McpError, ServerId, ToolDescriptor, ToolResult, GitOps, DocsLookup, PrioritizedServer};
pub use util::init_tracing_once;

/// Convenience helper: load default config, construct a manager, and autostart prioritized servers.
pub async fn manager_from_default_config_autostart() -> Result<McpManager, McpError> {
    let (cfg, _path) = McpConfig::load_default_file()?;
    let mgr = McpManager::from_config(cfg).await?;
    mgr.start_autostart().await?;
    Ok(mgr)
}

// TODO: Add crate docs
// TODO: Fix doc tests and doc comments to handle backticks correctly
