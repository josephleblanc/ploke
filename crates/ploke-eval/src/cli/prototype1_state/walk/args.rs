//! Adapters between clap command structs and walk protocol DTOs.
//!
//! The public CLI structs live in `cli::args::loop_args`; this module keeps the
//! socket/protocol conversion logic near the walk server implementation.

use std::{path::Path, time::Duration};

use crate::{
    cli::{
        Prototype1StateWalkControlCommand, Prototype1StateWalkServeCommand,
        Prototype1StateWalkStartCommand, Prototype1StateWalkStepCommand,
    },
    spec::PrepareError,
};

use super::{paths, protocol::WalkStartConfig};

pub(crate) const DEFAULT_IDLE_TTL_SECS: u64 = 30 * 60;

impl Prototype1StateWalkStartCommand {
    /// Convert the start subcommand into the serializable start payload.
    pub(crate) fn start_config(self) -> WalkStartConfig {
        WalkStartConfig {
            campaign: self.campaign,
            node_id: self.node_id,
            repo_root: self.repo_root,
            init_parent_identity: self.init_parent_identity,
            identity_branch: self.identity_branch,
            identity_instance: self.identity_instance,
            handoff_invocation: self.handoff_invocation,
            stop_after: self.stop_after,
            successor_selection: self.successor_selection,
            successor_selection_seed: self.successor_selection_seed,
            successor_selection_metrics: self.successor_selection_metrics,
            candidate_generator: self.candidate_generator,
            format: self.format,
        }
    }

    /// Borrow the optional repo root before the command is consumed.
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }

    /// Idle TTL requested for an auto-spawned server.
    pub(crate) fn idle_ttl(&self) -> Result<Option<Duration>, PrepareError> {
        idle_ttl(self.ttl_secs, self.no_ttl)
    }
}

impl Prototype1StateWalkServeCommand {
    /// Idle TTL requested for this server process.
    pub(crate) fn idle_ttl(&self) -> Result<Option<Duration>, PrepareError> {
        idle_ttl(self.ttl_secs, self.no_ttl)
    }
}

impl Prototype1StateWalkStepCommand {
    /// Borrow the optional repo root used for socket discovery.
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }
}

impl Prototype1StateWalkControlCommand {
    /// Borrow the optional repo root used for socket discovery.
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }
}

/// Resolve both repo root and socket path for a client command.
///
/// Resolution order is explicit arguments, saved `walk use` context, then the
/// current working directory and repo-hashed runtime socket.
pub(crate) fn resolve_socket(
    repo_root: Option<&Path>,
    socket: Option<&Path>,
) -> Result<(std::path::PathBuf, std::path::PathBuf), PrepareError> {
    let context = if repo_root.is_none() || socket.is_none() {
        paths::load_context()?
    } else {
        None
    };
    let repo_root = match repo_root {
        Some(path) => paths::resolve_repo_root(Some(path))?,
        None => context
            .as_ref()
            .map(|context| context.repo_root.clone())
            .map(Ok)
            .unwrap_or_else(|| paths::resolve_repo_root(None))?,
    };
    let socket_override = match socket {
        Some(path) => Some(path),
        None if repo_root_matches_context(&repo_root, context.as_ref()) => context
            .as_ref()
            .and_then(|context| context.socket.as_deref()),
        None => None,
    };
    let socket = paths::socket_path(&repo_root, socket_override)?;
    Ok((repo_root, socket))
}

fn repo_root_matches_context(repo_root: &Path, context: Option<&paths::WalkContext>) -> bool {
    context
        .map(|context| context.repo_root == repo_root)
        .unwrap_or(false)
}

fn idle_ttl(ttl_secs: Option<u64>, no_ttl: bool) -> Result<Option<Duration>, PrepareError> {
    if no_ttl {
        return Ok(None);
    }
    let seconds = ttl_secs.unwrap_or(DEFAULT_IDLE_TTL_SECS);
    if seconds == 0 {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "walk --ttl-secs must be greater than zero; use --no-ttl to disable the idle timeout"
                .to_string(),
        });
    }
    Ok(Some(Duration::from_secs(seconds)))
}
