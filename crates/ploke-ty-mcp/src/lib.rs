pub mod git_client;
pub mod context7_client;

pub mod config;
pub mod manager;
pub mod types;

pub use config::{McpConfig, ServerSpec};
pub use manager::McpManager;
pub use types::{McpError, ServerId};

// TODO: Add crate docs
// TODO: Fix doc tests and doc comments to handle backticks correctly
