//! Freshness guard for the long-running walk server.
//!
//! A self-editing loop can change the checkout while an older server binary is
//! still holding typed state. `ServerEpoch` is the fail-closed guard that keeps
//! mutating requests from applying stale transition semantics to a newer source
//! tree.

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::spec::PrepareError;

/// Wire-protocol version for framed JSON walk requests.
pub(crate) const WALK_PROTOCOL_VERSION: u32 = 3;

/// Semantic version for the currently admitted transition graph slice.
pub(crate) const TRANSITION_GRAPH_VERSION: &str = "walk-r0-r13a-v1";

/// Repo paths whose dirty/clean status participates in the server epoch.
///
/// This is deliberately narrower than the entire repository. It covers the CLI,
/// walk server, and Prototype 1 transition code that can change stepping
/// semantics. See the active agent doc for the known limitation: this is a
/// status hash, not a full content hash of every dirty file.
const SOURCE_GUARD_PATHS: &[&str] = &[
    "Cargo.toml",
    "crates/ploke-eval/Cargo.toml",
    "crates/ploke-eval/src/cli/args",
    "crates/ploke-eval/src/cli/handlers/prototype1_loop.rs",
    "crates/ploke-eval/src/cli/prototype1_state",
];

/// Identity of the source/binary snapshot that owns one walk server instance.
///
/// Clients include their own freshly captured epoch on mutating requests. The
/// server compares that client epoch with both its startup epoch and the current
/// filesystem state before it advances typestate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ServerEpoch {
    pub(crate) protocol_version: u32,
    pub(crate) transition_graph_version: String,
    pub(crate) repo_root: PathBuf,
    pub(crate) exe_path: PathBuf,
    pub(crate) exe_modified_unix_ms: Option<u64>,
    pub(crate) git_head: Option<String>,
    pub(crate) source_status_hash: Option<String>,
}

impl ServerEpoch {
    /// Capture the epoch for `repo_root` and the currently running executable.
    pub(crate) fn capture(repo_root: &Path) -> Result<Self, PrepareError> {
        let repo_root = repo_root.to_path_buf();
        let exe_path = std::env::current_exe().map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_current_exe",
            detail: source.to_string(),
        })?;
        let exe_modified_unix_ms = std::fs::metadata(&exe_path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64);
        let git_head = git_output(&repo_root, &["rev-parse", "HEAD"]);
        let source_status_hash = source_status_hash(&repo_root);
        Ok(Self {
            protocol_version: WALK_PROTOCOL_VERSION,
            transition_graph_version: TRANSITION_GRAPH_VERSION.to_string(),
            repo_root,
            exe_path,
            exe_modified_unix_ms,
            git_head,
            source_status_hash,
        })
    }

    /// Validate that a mutating request came from a compatible client binary.
    ///
    /// Read-only requests intentionally bypass this check so a stale server can
    /// still be inspected and stopped.
    pub(crate) fn ensure_compatible_request(
        &self,
        request_epoch: Option<&ServerEpoch>,
    ) -> Result<(), PrepareError> {
        let Some(request_epoch) = request_epoch else {
            return Err(stale_error(
                "mutating request omitted client epoch; restart the client command with the current binary",
            ));
        };
        if request_epoch.protocol_version != self.protocol_version {
            return Err(stale_error(format!(
                "walk protocol mismatch: client={} server={}",
                request_epoch.protocol_version, self.protocol_version
            )));
        }
        if request_epoch.transition_graph_version != self.transition_graph_version {
            return Err(stale_error(format!(
                "transition graph mismatch: client={} server={}",
                request_epoch.transition_graph_version, self.transition_graph_version
            )));
        }
        if request_epoch.exe_path != self.exe_path
            || request_epoch.exe_modified_unix_ms != self.exe_modified_unix_ms
        {
            return Err(stale_error(
                "walk server binary differs from client binary; restart the walk server",
            ));
        }
        if request_epoch.git_head != self.git_head
            || request_epoch.source_status_hash != self.source_status_hash
        {
            return Err(stale_error(
                "walk server source epoch differs from client source epoch; restart the walk server",
            ));
        }
        Ok(())
    }

    /// Re-capture the current epoch and reject mutation if the server went stale.
    pub(crate) fn ensure_not_stale_now(&self) -> Result<(), PrepareError> {
        let current = Self::capture(&self.repo_root)?;
        if current.exe_path != self.exe_path
            || current.exe_modified_unix_ms != self.exe_modified_unix_ms
            || current.git_head != self.git_head
            || current.source_status_hash != self.source_status_hash
        {
            return Err(stale_error(
                "walk server source/binary changed since startup; restart the walk server",
            ));
        }
        Ok(())
    }
}

fn source_status_hash(repo_root: &Path) -> Option<String> {
    let mut args = vec!["status", "--porcelain=v1", "--"];
    args.extend(SOURCE_GUARD_PATHS.iter().copied());
    git_output(repo_root, &args).map(|status| {
        let mut hasher = Sha256::new();
        hasher.update(status.as_bytes());
        let digest = hasher.finalize();
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    })
}

fn git_output(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn stale_error(detail: impl Into<String>) -> PrepareError {
    PrepareError::InvalidBatchSelection {
        detail: format!("stale walk server: {}", detail.into()),
    }
}
