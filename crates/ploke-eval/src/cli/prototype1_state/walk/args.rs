//! Adapters between clap command structs and walk protocol DTOs.
//!
//! The public CLI structs live in `cli::args::loop_args`; this module keeps the
//! socket/protocol conversion logic near the walk server implementation.

use std::{path::Path, time::Duration};

use crate::{
    cli::{
        Prototype1StateWalkAuditCommand, Prototype1StateWalkBranchLiveCommand,
        Prototype1StateWalkControlCommand, Prototype1StateWalkLlmFinishCommand,
        Prototype1StateWalkLlmStepCommand, Prototype1StateWalkRecoverCommand,
        Prototype1StateWalkReplayCommand, Prototype1StateWalkReplayMoveCommand,
        Prototype1StateWalkServeCommand, Prototype1StateWalkStartCommand,
        Prototype1StateWalkStepCommand,
    },
    spec::PrepareError,
};

use super::{endpoint, paths, protocol::WalkStartConfig};
use crate::cli::prototype1_state::driver::control::RecoveryDirective;

pub(crate) const DEFAULT_IDLE_TTL_SECS: u64 = 30 * 60;

impl Prototype1StateWalkStartCommand {
    /// Convert the start subcommand into the serializable start payload.
    pub(crate) fn start_config(self) -> WalkStartConfig {
        WalkStartConfig {
            campaign: self.campaign,
            repo_root: self.repo_root,
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

    /// Whether this step request admits active checkout mutation.
    pub(crate) fn allow_git_changes(&self) -> bool {
        self.allow
            .iter()
            .any(|capability| capability == "git-changes")
    }
}

impl Prototype1StateWalkControlCommand {
    /// Borrow the optional repo root used for socket discovery.
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }
}

impl Prototype1StateWalkRecoverCommand {
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.control.repo_root_ref()
    }

    pub(crate) fn directive(&self) -> RecoveryDirective {
        if self.abandon_owner {
            RecoveryDirective::AbandonOwner
        } else if self.abandon_session {
            RecoveryDirective::AbandonSession
        } else if self.admit_epoch {
            RecoveryDirective::AdmitEpoch
        } else {
            RecoveryDirective::Inspect
        }
    }
}

impl Prototype1StateWalkLlmStepCommand {
    /// Whether this step admits current-tool workspace mutation.
    pub(crate) fn allow_workspace_mutation(&self) -> bool {
        self.allow
            .iter()
            .any(|capability| capability == "workspace-mutation")
    }
}

impl Prototype1StateWalkLlmFinishCommand {
    /// Whether finish admits current-tool workspace mutation.
    pub(crate) fn allow_workspace_mutation(&self) -> bool {
        self.allow
            .iter()
            .any(|capability| capability == "workspace-mutation")
    }
}

impl Prototype1StateWalkReplayCommand {
    /// Borrow the optional repo root used for socket discovery.
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.control.repo_root_ref()
    }
}

impl Prototype1StateWalkAuditCommand {
    /// Borrow the optional repo root used for socket discovery.
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.control.repo_root_ref()
    }
}

impl Prototype1StateWalkReplayMoveCommand {
    /// Borrow the optional repo root used for socket discovery.
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.control.repo_root_ref()
    }
}

impl Prototype1StateWalkBranchLiveCommand {
    /// Borrow the optional repo root used for socket discovery.
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.control.repo_root_ref()
    }

    /// Whether this branch provenance request admits writing a provenance record.
    pub(crate) fn allow_provenance_record(&self) -> bool {
        self.allow
            .iter()
            .any(|capability| capability == "provenance-record")
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
    let active = if socket.is_none() {
        endpoint::load(&repo_root)?
            .filter(|endpoint| endpoint.repo_root() == repo_root && endpoint.owns_socket())
    } else {
        None
    };
    let socket_override = match socket {
        Some(path) => Some(path),
        None if active.is_some() => active.as_ref().map(|endpoint| endpoint.socket()),
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
