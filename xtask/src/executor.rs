//! Minimal command trait shared by clap subcommands.
//!
//! `xtask` originally experimented with a richer executor/registry framework.
//! That work is intentionally paused (see the coordination docs) and has been
//! trimmed to avoid shipping unused infrastructure and `todo!()` landmines.
//!
//! The current command model is simple:
//! - Clap parses a nested subcommand (e.g. `parse …`, `db …`)
//! - The subcommand implements [`Command`]
//! - [`crate::cli::Cli::execute`] calls `cmd.execute(&CommandContext)`

use serde::Serialize;

use crate::context::CommandContext;
use crate::error::XtaskError;

/// The fundamental trait that structured clap subcommands implement.
///
/// The clap CLI constructs a command value (e.g. a `parse …` or `db …` subcommand)
/// and calls [`Command::execute`] directly.
///
/// # Example
/// ```ignore
/// impl Command for MyCommand {
///     type Output = MyOutput;
///     type Error = MyError;
///
///     fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
///         // Command implementation
///     }
/// }
/// ```
pub trait Command: Send + Sync + 'static {
    /// The type of output this command produces.
    /// Must be serializable for JSON output support.
    type Output: Serialize + Send + 'static;

    /// The type of error this command can return.
    /// Must be convertible to `XtaskError`.
    type Error: Into<XtaskError>;

    /// Execute the command with access to shared resources.
    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error>;
}
