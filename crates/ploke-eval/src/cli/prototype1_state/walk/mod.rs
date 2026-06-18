//! Debug-only local server for walking Prototype 1 typestate transitions.
//!
//! The walk server owns an in-memory [`controller::WalkState`] and exposes a
//! small local IPC surface over a Unix socket. It is intentionally a debugging
//! harness over the canonical `live_edges`, not production loop authority.

pub(crate) mod args;
pub(crate) mod client;
pub(crate) mod controller;
pub(crate) mod epoch;
pub(crate) mod ipc;
pub(crate) mod paths;
pub(crate) mod phase;
pub(crate) mod protocol;
pub(crate) mod server;
pub(crate) mod summary;

use crate::cli::Prototype1StateWalkCommand;
use crate::spec::PrepareError;

pub(crate) async fn run(command: Prototype1StateWalkCommand) -> Result<(), PrepareError> {
    match command.command {
        crate::cli::Prototype1StateWalkSubcommand::Serve(command) => server::serve(command).await,
        other => client::run(other).await,
    }
}
