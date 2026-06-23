//! Socket path selection and filesystem hygiene for the walk server.
//!
//! This mirrors the important Zellij practice of using a short runtime-dir
//! socket path with private permissions, while keeping the implementation local
//! to the debug harness.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::spec::PrepareError;

#[cfg(target_os = "macos")]
const UNIX_SOCKET_PATH_MAX: usize = 104;
#[cfg(not(target_os = "macos"))]
const UNIX_SOCKET_PATH_MAX: usize = 108;

const CONTEXT_SCHEMA_VERSION: &str = "walk-context.v1";

/// Saved operator defaults for short `ploke-eval loop walk ...` commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WalkContext {
    pub(crate) repo_root: PathBuf,
    pub(crate) socket: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalkContextFile {
    schema_version: String,
    repo_root: PathBuf,
    socket: Option<PathBuf>,
}

impl From<WalkContextFile> for WalkContext {
    fn from(file: WalkContextFile) -> Self {
        Self {
            repo_root: file.repo_root,
            socket: file.socket,
        }
    }
}

impl From<&WalkContext> for WalkContextFile {
    fn from(context: &WalkContext) -> Self {
        Self {
            schema_version: CONTEXT_SCHEMA_VERSION.to_string(),
            repo_root: context.repo_root.clone(),
            socket: context.socket.clone(),
        }
    }
}

/// Resolve the parent checkout root used for socket identity and epoch capture.
///
/// Resolution order is explicit `--repo-root`, saved `walk use` context, then
/// current working directory.
pub(crate) fn resolve_repo_root(repo_root: Option<&Path>) -> Result<PathBuf, PrepareError> {
    if let Some(path) = repo_root {
        return Ok(normalize_path(path));
    }
    if let Some(context) = load_context()? {
        return Ok(context.repo_root);
    }
    current_dir_repo_root()
}

/// Resolve a root for `walk use` without consulting any existing context.
pub(crate) fn resolve_use_repo_root(repo_root: Option<&Path>) -> Result<PathBuf, PrepareError> {
    match repo_root {
        Some(path) => Ok(normalize_path(path)),
        None => current_dir_repo_root(),
    }
}

/// Load the saved walk context, if one exists.
pub(crate) fn load_context() -> Result<Option<WalkContext>, PrepareError> {
    let path = context_path();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PrepareError::ReadManifest { path, source });
        }
    };
    let file: WalkContextFile =
        serde_json::from_str(&text).map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_context_parse",
            detail: format!(
                "failed to parse walk context '{}': {source}",
                path.display()
            ),
        })?;
    if file.schema_version != CONTEXT_SCHEMA_VERSION {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "unsupported walk context schema '{}' at '{}'; expected '{}'",
                file.schema_version,
                path.display(),
                CONTEXT_SCHEMA_VERSION
            ),
        });
    }
    Ok(Some(file.into()))
}

/// Save the active walk context used by later commands.
pub(crate) fn save_context(context: &WalkContext) -> Result<PathBuf, PrepareError> {
    let path = context_path();
    ensure_socket_parent(&path)?;
    let file = WalkContextFile::from(context);
    let body = serde_json::to_string_pretty(&file).map_err(PrepareError::Serialize)?;
    fs::write(&path, body).map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_context_write",
        detail: format!(
            "failed to write walk context '{}': {source}",
            path.display()
        ),
    })?;
    Ok(path)
}

/// Path to the saved walk context file.
pub(crate) fn context_path() -> PathBuf {
    socket_dir().join("context.json")
}

/// Compute the Unix socket path for a repo, honoring explicit overrides.
///
/// Default paths use a short hash of the canonical repo root so long checkout
/// paths do not overflow `sockaddr_un.sun_path`.
pub(crate) fn socket_path(
    repo_root: &Path,
    override_path: Option<&Path>,
) -> Result<PathBuf, PrepareError> {
    if let Some(path) = override_path {
        check_socket_path(path)?;
        return Ok(path.to_path_buf());
    }
    let dir = socket_dir();
    let name = format!("p1walk-{}.sock", repo_hash(repo_root));
    let path = dir.join(name);
    check_socket_path(&path)?;
    Ok(path)
}

/// Create the socket parent directory and restrict it to the current user.
pub(crate) fn ensure_socket_parent(path: &Path) -> Result<(), PrepareError> {
    let Some(parent) = path.parent() else {
        return Err(PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_socket_parent",
            detail: format!("socket path '{}' has no parent directory", path.display()),
        });
    };
    fs::create_dir_all(parent).map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_socket_dir",
        detail: format!(
            "failed to create socket dir '{}': {source}",
            parent.display()
        ),
    })?;
    set_private_dir_permissions(parent)?;
    Ok(())
}

/// Remove an old socket path, treating a missing file as success.
pub(crate) fn remove_socket_file(path: &Path) -> Result<(), PrepareError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_remove_socket",
            detail: format!("failed to remove socket '{}': {source}", path.display()),
        }),
    }
}

fn current_dir_repo_root() -> Result<PathBuf, PrepareError> {
    env::current_dir().map_err(|source| PrepareError::ReadManifest {
        path: PathBuf::from("."),
        source,
    })
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn socket_dir() -> PathBuf {
    if let Ok(path) = env::var("PLOKE_EVAL_WALK_SOCKET_DIR") {
        return PathBuf::from(path);
    }
    if let Ok(path) = env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(path).join("ploke-eval").join("walk");
    }
    let user = env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    env::temp_dir()
        .join(format!("ploke-eval-{user}"))
        .join("walk")
}

fn repo_hash(repo_root: &Path) -> String {
    let canonical = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    hex_prefix(&digest, 16)
}

fn hex_prefix(bytes: &[u8], chars: usize) -> String {
    let mut output = String::with_capacity(chars);
    for byte in bytes {
        if output.len() >= chars {
            break;
        }
        output.push_str(&format!("{byte:02x}"));
    }
    output.truncate(chars);
    output
}

fn check_socket_path(path: &Path) -> Result<(), PrepareError> {
    let len = path.as_os_str().len();
    if len >= UNIX_SOCKET_PATH_MAX {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "walk socket path is too long ({} bytes, max {}): '{}'; set PLOKE_EVAL_WALK_SOCKET_DIR to a shorter private directory or pass --socket",
                len,
                UNIX_SOCKET_PATH_MAX - 1,
                path.display()
            ),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<(), PrepareError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_socket_dir_metadata",
        detail: format!("failed to stat socket dir '{}': {source}", path.display()),
    })?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_socket_dir_permissions",
        detail: format!(
            "failed to chmod 0700 socket dir '{}': {source}",
            path.display()
        ),
    })
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> Result<(), PrepareError> {
    Ok(())
}
