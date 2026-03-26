//! xtask - Ploke workspace automation commands
//!
//! This crate provides `xtask`’s clap-driven command types and shared utilities.
//!
//! The `xtask` binary dispatches a small set of workspace maintenance helpers
//! (fixture/backup DB utilities, profiling helpers), then falls back to the
//! structured clap CLI (`parse`, `db`, …) defined in [`crate::cli`].
//!
//! # Architecture
//!
//! The current shape is intentionally minimal:
//!
//! - **Commands**: clap subcommands implement [`crate::executor::Command`]
//! - **Context**: [`CommandContext`] provides lazy-initialized shared resources
//! - **Errors**: [`XtaskError`] unifies command-boundary failures
//!
//! # Example
//!
//! ```ignore
//! use xtask::context::CommandContext;
//! use xtask::error::XtaskError;
//!
//! // Define a command
//! #[derive(Debug)]
//! struct MyCommand;
//!
//! impl xtask::executor::Command for MyCommand {
//!     type Output = String;
//!     type Error = XtaskError;
//!
//!     fn name(&self) -> &'static str { "my-command" }
//!     fn category(&self) -> xtask::executor::CommandCategory { xtask::executor::CommandCategory::Utility }
//!     fn requires_async(&self) -> bool { false }
//!
//!     fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
//!         Ok("Hello, world!".to_string())
//!     }
//! }
//!
//! // Execute the command directly (this is what the clap CLI does).
//! let ctx = CommandContext::new().unwrap();
//! let result = MyCommand.execute(&ctx).unwrap();
//! ```

#![deny(missing_docs)]
#![warn(rust_2018_idioms)]

/// Clap CLI definition and structured subcommand dispatch (`parse`, `db`, …).
pub mod cli;
/// Structured command implementations grouped by responsibility.
pub mod commands;
/// Shared command context and lazy resource initialization.
pub mod context;
/// Unified error types and recovery hints for command boundaries.
pub mod error;
/// Minimal command trait shared by clap subcommands.
pub mod executor;

// Re-export commonly used types for tests and consumers.
pub use context::CommandContext;
pub use error::PARSE_FAILURE_DIAGNOSTIC_HINT;
pub use error::XtaskError;
pub use executor::Command;

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
