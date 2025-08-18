pub mod git_client;
pub mod context7_client;

pub mod config;
pub mod manager;
pub mod types;
pub mod clients;

pub use config::{McpConfig, ServerSpec};
pub use manager::McpManager;
pub use clients::{context7::Context7Client, git::GitClient};
pub use types::{McpError, ServerId, ToolDescriptor, ToolResult};

// TODO: Add crate docs
// TODO: Fix doc tests and doc comments to handle backticks correctly
