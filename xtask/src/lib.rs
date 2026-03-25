//! xtask - Ploke workspace automation commands
//!
//! This crate provides a command framework for automating workspace tasks,
//! including parsing, transformation, database operations, and testing utilities.
//!
//! # Architecture
//!
//! The xtask system is built around several core components:
//!
//! - **Commands**: Types implementing the [`Command`] trait define executable operations
//! - **Executor**: The [`CommandExecutor`] manages command lifecycle and resource coordination
//! - **Registry**: The [`CommandRegistry`] provides command registration and discovery
//! - **Context**: The [`CommandContext`] provides lazy-initialized shared resources
//! - **Usage Tracking**: The [`UsageTracker`] records command statistics and triggers suggestions
//! - **Test Harness**: The [`CommandTestHarness`] enables comprehensive command testing
//!
//! # Example
//!
//! ```ignore
//! use xtask::executor::{Command, CommandExecutor, ExecutorConfig};
//! use xtask::context::CommandContext;
//! use xtask::error::XtaskError;
//!
//! // Define a command
//! #[derive(Debug)]
//! struct MyCommand;
//!
//! impl Command for MyCommand {
//!     type Output = String;
//!     type Error = XtaskError;
//!
//!     fn name(&self) -> &'static str { "my-command" }
//!     fn category(&self) -> CommandCategory { CommandCategory::Utility }
//!     fn requires_async(&self) -> bool { false }
//!
//!     fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
//!         Ok("Hello, world!".to_string())
//!     }
//! }
//!
//! // Execute the command
//! let executor = CommandExecutor::new(ExecutorConfig::default()).unwrap();
//! let result = executor.execute(MyCommand).unwrap();
//! ```

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod cli;
pub mod commands;
pub mod context;
pub mod error;
pub mod executor;
pub mod test_harness;
pub mod usage;

// Re-export commonly used types
pub use context::CommandContext;
pub use error::XtaskError;
pub use executor::{
    Command, CommandCategory, CommandExecutor, CommandRegistry, ExecutorConfig, ResourceRequirements,
};
pub use test_harness::{expect_command_ok, CommandTestHarness};
pub use usage::{UsageStats, UsageTracker};

/// Version of the xtask crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get the workspace root directory.
///
/// This searches for the Cargo.toml file to identify the workspace root.
pub fn workspace_root() -> Result<std::path::PathBuf, XtaskError> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // xtask is at <workspace>/xtask, so workspace root is the parent
    manifest_dir
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| XtaskError::new("Could not determine workspace root"))
}

/// Utility function to display a path relative to the workspace root.
pub fn display_relative(path: &std::path::Path, root: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_workspace_root() {
        let root = workspace_root().unwrap();
        assert!(root.exists());
        assert!(root.join("Cargo.toml").exists());
    }

    #[test]
    fn test_display_relative() {
        let root = std::path::PathBuf::from("/workspace");
        let path = std::path::PathBuf::from("/workspace/src/main.rs");
        assert_eq!(display_relative(&path, &root), "src/main.rs");

        // Path outside root should return full path
        let outside = std::path::PathBuf::from("/other/file.rs");
        assert_eq!(display_relative(&outside, &root), "/other/file.rs");
    }
}
