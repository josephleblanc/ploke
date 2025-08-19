//! ploke-ty-mcp: typed async client for MCP servers (git, context7) built on rmcp.
//! Provides McpManager, typed clients GitClient and Context7Client, a config loader, timeouts,
//! cancellation, restart-on-exit, and tracing-based observability.
//! End-to-end tests are gated by PLOKE_E2E_MCP=1 and require uvx/npx on PATH.
//!
//! Quick start:
//! ```no_run
//! use ploke_ty_mcp::{McpManager, McpConfig};
//! # async fn run() -> Result<(), ploke_ty_mcp::McpError> {
//! let (cfg, _path) = McpConfig::load_default_file()?;
//! let mgr = McpManager::from_config(cfg).await?;
//! mgr.start_autostart().await?;
//! if let Some(git) = mgr.client_git() {
//!     let status = git.status(".").await?;
//!     println!("{status}");
//! }
//! # Ok(()) }
//! ```
//!
//! See PLAN.md for architecture and roadmap.
pub mod clients;
pub mod config;
pub mod manager;
pub mod types;
pub mod util;

pub use clients::{context7::Context7Client, git::GitClient};
pub use config::{McpConfig, ServerSpec};
pub use manager::McpManager;
pub use types::{
    DocsLookup, GitOps, McpError, PrioritizedServer, ServerId, ToolDescriptor, ToolResult,
};
pub use util::init_tracing_once;

/// Convenience helper: load default config, construct a manager, and autostart prioritized servers.
pub async fn manager_from_default_config_autostart() -> Result<McpManager, McpError> {
    let (cfg, _path) = McpConfig::load_default_file()?;
    let mgr = McpManager::from_config(cfg).await?;
    mgr.start_autostart().await?;
    Ok(mgr)
}
