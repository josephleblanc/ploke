//! Command module for xtask utilities.
//!
//! This module provides command implementations organized by crate responsibility:
//! - `parse` - syn_parser integration (A.1)
//! - `db` - ploke_db integration (A.4)
//! - `transform` - ploke_transform integration (A.2) [M.4]
//! - `ingest` - ploke_embed integration (A.3) [M.4]
//!
//! ## Architecture
//!
//! This module integrates with the core xtask architecture from `lib.rs`:
//! - [`crate::context::CommandContext`] - Resource management
//! - [`crate::error::XtaskError`] - Error handling
//! - [`OutputFormat`] - CLI output formatting
//!
//! ## Usage
//!
//! ```rust,ignore
//! use xtask::commands::{Command, CommandContext, OutputFormat};
//!
//! fn run_command(cmd: impl Command) -> Result<(), XtaskError> {
//!     let ctx = CommandContext::new()?;
//!     cmd.execute(&ctx)?;
//!     Ok(())
//! }
//! ```

use serde::Serialize;

// Re-export command modules
pub mod db;
pub mod parse;
pub mod parse_debug;

// Re-export types from core architecture
pub use crate::context::CommandContext;
pub use crate::error::XtaskError;

/// Output format for command results.
///
/// This type is used by the CLI to determine how to format command output.
/// It is separate from any executor framework to provide CLI-specific formatting options.
#[derive(Debug, Clone, Copy, Default, Serialize, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable formatted output with colors and indentation
    #[default]
    Human,
    /// JSON output for programmatic consumption
    Json,
    /// Tab-separated table output
    Table,
    /// Compact single-line output
    Compact,
}

impl OutputFormat {
    /// Format a serializable value according to this format.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn format<T: Serialize>(&self, value: &T) -> std::result::Result<String, XtaskError> {
        match self {
            OutputFormat::Human => format_human(value),
            OutputFormat::Json => {
                serde_json::to_string_pretty(value).map_err(|e| XtaskError::new(e.to_string()))
            }
            OutputFormat::Table => format_table(value),
            OutputFormat::Compact => {
                serde_json::to_string(value).map_err(|e| XtaskError::new(e.to_string()))
            }
        }
    }
}

/// Format a value for human-readable output.
fn format_human<T: Serialize>(value: &T) -> std::result::Result<String, XtaskError> {
    // For now, use pretty JSON as a reasonable human-readable format
    // This can be enhanced with custom formatting in M.4
    serde_json::to_string_pretty(value).map_err(|e| XtaskError::new(e.to_string()))
}

/// Format a value as a table.
fn format_table<T: Serialize>(_value: &T) -> std::result::Result<String, XtaskError> {
    // Placeholder - full implementation in M.4
    // For now, return a generic error indicating not implemented
    Err(XtaskError::new("Table formatting not yet implemented"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_default() {
        let fmt = OutputFormat::default();
        assert!(matches!(fmt, OutputFormat::Human));
    }

    #[test]
    fn test_output_format_json() {
        let data = serde_json::json!({"key": "value"});
        let formatted = OutputFormat::Json.format(&data).unwrap();
        assert!(formatted.contains("key"));
        assert!(formatted.contains("value"));
    }

    #[test]
    fn test_output_format_human() {
        let data = serde_json::json!({"key": "value"});
        let formatted = OutputFormat::Human.format(&data).unwrap();
        assert!(formatted.contains("key"));
    }

    #[test]
    fn test_output_format_table_not_implemented() {
        let data = serde_json::json!({"key": "value"});
        let result = OutputFormat::Table.format(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not yet implemented"));
    }
}
