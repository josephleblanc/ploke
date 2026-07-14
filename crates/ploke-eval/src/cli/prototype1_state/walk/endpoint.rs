//! Durable ownership record for one walk-server Unix socket.

use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::{fd::AsRawFd, unix::fs::MetadataExt};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{durable_io, spec::PrepareError};

use super::paths;

const SCHEMA_VERSION: &str = "prototype1-walk-endpoint.v1";

/// Exact socket inode and process that own one reachable walk endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServerEndpoint {
    schema_version: String,
    repo_root: PathBuf,
    socket: PathBuf,
    owner: Uuid,
    pid: u32,
    device: u64,
    inode: u64,
}

impl ServerEndpoint {
    pub(crate) fn from_bound(repo_root: PathBuf, socket: PathBuf) -> Result<Self, PrepareError> {
        let metadata = fs::metadata(&socket).map_err(|source| {
            endpoint_error(
                "prototype1_walk_endpoint_stat",
                format!("cannot stat bound socket '{}': {source}", socket.display()),
            )
        })?;
        #[cfg(unix)]
        let (device, inode) = (metadata.dev(), metadata.ino());
        #[cfg(not(unix))]
        let (device, inode) = (0, 0);
        Ok(Self {
            schema_version: SCHEMA_VERSION.to_string(),
            repo_root,
            socket,
            owner: Uuid::new_v4(),
            pid: std::process::id(),
            device,
            inode,
        })
    }

    pub(crate) fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub(crate) fn socket(&self) -> &Path {
        &self.socket
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn activate(&self) -> Result<(), PrepareError> {
        self.validate()?;
        let path = paths::endpoint_path(&self.repo_root)?;
        with_lock(&path, || {
            if let Some(active) = load_path(&path)?
                && active.owner != self.owner
            {
                let same_socket = active.socket == self.socket;
                let live_owner = if same_socket {
                    active.owns_socket()
                } else {
                    socket_reachable(&active.socket)?
                };
                if live_owner {
                    return Err(endpoint_error(
                        "prototype1_walk_endpoint_activate",
                        format!(
                            "cannot replace live endpoint owner {} at '{}' with owner {} at '{}'",
                            active.owner,
                            active.socket.display(),
                            self.owner,
                            self.socket.display()
                        ),
                    ));
                }
            }
            write_pointer(&path, self)
        })
    }

    /// Replace only the exact endpoint observed before predecessor transfer.
    /// A changed pointer is newer authority and cannot be displaced by a
    /// delayed successor, even when the old expected listener is still live.
    pub(crate) fn take_over(&self, expected: Option<&Self>) -> Result<(), PrepareError> {
        self.validate()?;
        let path = paths::endpoint_path(&self.repo_root)?;
        with_lock(&path, || {
            let active = load_path(&path)?;
            let unchanged = match (active.as_ref(), expected) {
                (None, _) => true,
                (Some(active), _) if active.owner == self.owner => true,
                (Some(active), _) if active.socket == self.socket && !active.owns_socket() => true,
                (Some(active), Some(expected)) if active == expected => true,
                (Some(active), None) => !socket_reachable(&active.socket)?,
                (Some(_), Some(_)) => false,
            };
            if !unchanged {
                let active = active.expect("changed endpoint exists");
                return Err(endpoint_error(
                    "prototype1_walk_endpoint_takeover",
                    format!(
                        "endpoint authority changed before takeover: expected {}, found owner {} at '{}'",
                        expected
                            .map(|endpoint| endpoint.owner.to_string())
                            .unwrap_or_else(|| "no live owner".to_string()),
                        active.owner,
                        active.socket.display()
                    ),
                ));
            }
            write_pointer(&path, self)
        })
    }

    /// Remove only the pointer and socket inode created by this owner.
    pub(crate) fn cleanup(&self) -> Result<(), PrepareError> {
        let path = paths::endpoint_path(&self.repo_root)?;
        with_lock(&path, || {
            if load_path(&path)?
                .as_ref()
                .is_some_and(|active| active.owner == self.owner)
            {
                remove_file(&path, "prototype1_walk_endpoint_remove")?;
            }
            if self.owns_socket() {
                remove_file(&self.socket, "prototype1_state_walk_remove_socket")?;
            }
            Ok(())
        })
    }

    pub(crate) fn owns_socket(&self) -> bool {
        let Ok(metadata) = fs::metadata(&self.socket) else {
            return false;
        };
        #[cfg(unix)]
        {
            metadata.dev() == self.device && metadata.ino() == self.inode
        }
        #[cfg(not(unix))]
        {
            let _ = metadata;
            false
        }
    }

    pub(crate) fn validate(&self) -> Result<(), PrepareError> {
        self.validate_persisted()?;
        if !self.owns_socket() {
            return Err(endpoint_error(
                "prototype1_walk_endpoint_validate",
                format!(
                    "endpoint owner {} does not own socket '{}'",
                    self.owner,
                    self.socket.display()
                ),
            ));
        }
        Ok(())
    }

    /// Validate durable endpoint identity without requiring its historical
    /// socket inode to remain present after a clean Stop or process crash.
    pub(crate) fn validate_persisted(&self) -> Result<(), PrepareError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(endpoint_error(
                "prototype1_walk_endpoint_validate",
                format!("unsupported endpoint schema '{}'", self.schema_version),
            ));
        }
        if self.pid == 0 || self.owner.is_nil() {
            return Err(endpoint_error(
                "prototype1_walk_endpoint_validate",
                "endpoint has no durable owner identity".to_string(),
            ));
        }
        Ok(())
    }
}

fn write_pointer(path: &Path, endpoint: &ServerEndpoint) -> Result<(), PrepareError> {
    let bytes = serde_json::to_vec_pretty(endpoint).map_err(PrepareError::Serialize)?;
    durable_io::write_atomic(path, &bytes).map_err(|source| {
        endpoint_error(
            "prototype1_walk_endpoint_write",
            format!("cannot write endpoint '{}': {source}", path.display()),
        )
    })
}

#[cfg(unix)]
fn socket_reachable(socket: &Path) -> Result<bool, PrepareError> {
    match std::os::unix::net::UnixStream::connect(socket) {
        Ok(_) => Ok(true),
        Err(source)
            if matches!(
                source.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) =>
        {
            Ok(false)
        }
        Err(source) => Err(endpoint_error(
            "prototype1_walk_endpoint_probe",
            format!("cannot probe endpoint '{}': {source}", socket.display()),
        )),
    }
}

#[cfg(not(unix))]
fn socket_reachable(_socket: &Path) -> Result<bool, PrepareError> {
    Ok(true)
}

pub(crate) fn load(repo_root: &Path) -> Result<Option<ServerEndpoint>, PrepareError> {
    load_path(&paths::endpoint_path(repo_root)?)
}

fn load_path(path: &Path) -> Result<Option<ServerEndpoint>, PrepareError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(endpoint_error(
                "prototype1_walk_endpoint_read",
                format!("cannot read endpoint '{}': {source}", path.display()),
            ));
        }
    };
    let endpoint: ServerEndpoint = serde_json::from_str(&text).map_err(|source| {
        endpoint_error(
            "prototype1_walk_endpoint_parse",
            format!("cannot parse endpoint '{}': {source}", path.display()),
        )
    })?;
    if endpoint.schema_version != SCHEMA_VERSION {
        return Err(endpoint_error(
            "prototype1_walk_endpoint_parse",
            format!(
                "unsupported endpoint schema '{}' at '{}'",
                endpoint.schema_version,
                path.display()
            ),
        ));
    }
    Ok(Some(endpoint))
}

fn with_lock<T>(
    path: &Path,
    operation: impl FnOnce() -> Result<T, PrepareError>,
) -> Result<T, PrepareError> {
    let lock_path = path.with_extension("lock");
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            endpoint_error(
                "prototype1_walk_endpoint_dir",
                format!(
                    "cannot create endpoint dir '{}': {source}",
                    parent.display()
                ),
            )
        })?;
    }
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| {
            endpoint_error(
                "prototype1_walk_endpoint_lock",
                format!(
                    "cannot open endpoint lock '{}': {source}",
                    lock_path.display()
                ),
            )
        })?;
    lock_exclusive(&lock, &lock_path)?;
    operation()
}

#[cfg(unix)]
fn lock_exclusive(file: &File, path: &Path) -> Result<(), PrepareError> {
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if result == 0 {
        return Ok(());
    }
    Err(endpoint_error(
        "prototype1_walk_endpoint_lock",
        format!(
            "cannot lock '{}': {}",
            path.display(),
            io::Error::last_os_error()
        ),
    ))
}

#[cfg(not(unix))]
fn lock_exclusive(_file: &File, path: &Path) -> Result<(), PrepareError> {
    Err(endpoint_error(
        "prototype1_walk_endpoint_lock",
        format!("endpoint locking is unsupported for '{}'", path.display()),
    ))
}

fn remove_file(path: &Path, phase: &'static str) -> Result<(), PrepareError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(endpoint_error(
            phase,
            format!("cannot remove '{}': {source}", path.display()),
        )),
    }
}

fn endpoint_error(phase: &'static str, detail: String) -> PrepareError {
    PrepareError::DatabaseSetup { phase, detail }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{ffi::OsString, os::unix::net::UnixListener};

    use tempfile::tempdir;

    use crate::test_support::env_guard_os;

    use super::*;

    #[test]
    fn stopped_endpoint_remains_persisted() {
        let repo = tempdir().expect("repo tempdir");
        let home = tempdir().expect("eval home tempdir");
        let _env = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(home.path()))]);
        let socket = home.path().join("stopped.sock");
        let listener = UnixListener::bind(&socket).expect("bind endpoint");
        let endpoint = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket)
            .expect("capture endpoint");
        endpoint.validate().expect("live endpoint validates");

        drop(listener);
        endpoint.cleanup().expect("clean stopped endpoint");

        endpoint
            .validate_persisted()
            .expect("durable endpoint identity survives Stop");
        assert!(
            endpoint.validate().is_err(),
            "stopped endpoint must not claim current liveness"
        );
    }

    #[test]
    fn old_owner_cleanup_preserves_replacement_pointer_and_socket() {
        let repo = tempdir().expect("repo tempdir");
        let home = tempdir().expect("eval home tempdir");
        let _env = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(home.path()))]);
        let socket = home.path().join("replacement.sock");

        let old_listener = UnixListener::bind(&socket).expect("bind old socket");
        let old = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket.clone())
            .expect("capture old endpoint");
        old.activate().expect("publish old endpoint");

        drop(old_listener);
        fs::remove_file(&socket).expect("unlink old socket");
        let replacement_listener = UnixListener::bind(&socket).expect("bind replacement socket");
        let replacement = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket.clone())
            .expect("capture replacement endpoint");
        replacement
            .activate()
            .expect("publish replacement endpoint");

        old.cleanup().expect("cleanup old owner");

        assert_eq!(
            load(repo.path()).expect("load replacement endpoint"),
            Some(replacement.clone()),
            "old owner removed the replacement pointer"
        );
        assert!(socket.exists(), "old owner removed the replacement socket");
        assert!(
            replacement.owns_socket(),
            "replacement no longer owns its socket inode"
        );

        replacement.cleanup().expect("cleanup replacement owner");
        drop(replacement_listener);
    }

    #[test]
    fn activation_rejects_live_owner_and_replaces_stale_pointer() {
        let repo = tempdir().expect("repo tempdir");
        let home = tempdir().expect("eval home tempdir");
        let _env = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(home.path()))]);
        let active_socket = home.path().join("active.sock");
        let next_socket = home.path().join("next.sock");
        let active_listener = UnixListener::bind(&active_socket).expect("bind active endpoint");
        let next_listener = UnixListener::bind(&next_socket).expect("bind next endpoint");
        let active = ServerEndpoint::from_bound(repo.path().to_path_buf(), active_socket)
            .expect("capture active endpoint");
        let next = ServerEndpoint::from_bound(repo.path().to_path_buf(), next_socket)
            .expect("capture next endpoint");
        active.activate().expect("publish active endpoint");

        let error = next
            .activate()
            .expect_err("live endpoint owner must fence replacement activation");
        assert!(
            error
                .to_string()
                .contains("cannot replace live endpoint owner")
        );
        assert_eq!(
            load(repo.path()).expect("load active endpoint"),
            Some(active.clone())
        );

        drop(active_listener);
        // A parallel test may fork while the listener is open and briefly
        // inherit its descriptor until exec. Unlink the pathname so stale
        // reachability is deterministic even during that window.
        fs::remove_file(active.socket()).expect("unlink stale active endpoint");
        next.activate()
            .expect("unreachable active pointer may be replaced under lock");
        assert_eq!(
            load(repo.path()).expect("load replacement endpoint"),
            Some(next.clone())
        );

        active.cleanup().expect("cleanup stale active socket");
        next.cleanup().expect("cleanup replacement endpoint");
        drop(next_listener);
    }

    #[test]
    fn takeover_replaces_exact_predecessor_and_fences_pointer_change() {
        let repo = tempdir().expect("repo tempdir");
        let home = tempdir().expect("eval home tempdir");
        let _env = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(home.path()))]);
        let bind = |name: &str| {
            let socket = home.path().join(name);
            let listener = UnixListener::bind(&socket).expect("bind endpoint");
            let endpoint = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket)
                .expect("capture endpoint");
            (listener, endpoint)
        };
        let (first_listener, first) = bind("first.sock");
        let (next_listener, next) = bind("next.sock");
        let (newer_listener, newer) = bind("newer.sock");
        let (delayed_listener, delayed) = bind("delayed.sock");
        first.activate().expect("publish first endpoint");

        next.take_over(Some(&first))
            .expect("released successor may replace exact live predecessor");
        newer
            .take_over(Some(&next))
            .expect("next transfer may replace its exact predecessor");
        let error = delayed
            .take_over(Some(&first))
            .expect_err("delayed successor must not replace changed authority");
        assert!(
            error
                .to_string()
                .contains("authority changed before takeover")
        );
        assert_eq!(
            load(repo.path()).expect("load newest endpoint"),
            Some(newer.clone())
        );

        first.cleanup().expect("cleanup first endpoint");
        next.cleanup().expect("cleanup next endpoint");
        delayed.cleanup().expect("cleanup delayed endpoint");
        newer.cleanup().expect("cleanup newest endpoint");
        drop((
            first_listener,
            next_listener,
            newer_listener,
            delayed_listener,
        ));
    }

    #[test]
    fn takeover_replaces_stale_owner_at_same_socket() {
        let repo = tempdir().expect("repo tempdir");
        let home = tempdir().expect("eval home tempdir");
        let _env = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(home.path()))]);
        let socket = home.path().join("successor.sock");
        let prior_listener = UnixListener::bind(&socket).expect("bind prior endpoint");
        let prior = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket.clone())
            .expect("capture prior endpoint");
        prior.activate().expect("publish prior endpoint");
        drop(prior_listener);
        fs::remove_file(&socket).expect("remove stale socket inode");

        let next_listener = UnixListener::bind(&socket).expect("rebind successor endpoint");
        let next = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket)
            .expect("capture rebound endpoint");
        next.take_over(None)
            .expect("rebound owner may replace stale same-slot pointer");
        assert_eq!(
            load(repo.path()).expect("load rebound endpoint"),
            Some(next.clone())
        );

        next.cleanup().expect("cleanup rebound endpoint");
        drop(next_listener);
    }
}
