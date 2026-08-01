//! Freshness guard for the long-running walk server.
//!
//! A self-editing loop can change the checkout while an older server binary is
//! still holding typed state. `ServerEpoch` is the fail-closed guard that keeps
//! mutating requests from applying stale transition semantics to a newer source
//! tree.

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::UNIX_EPOCH,
};

#[cfg(unix)]
use std::os::unix::{
    ffi::{OsStrExt, OsStringExt},
    fs::MetadataExt,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::spec::PrepareError;

use super::source_guard_paths::SOURCE_GUARD_PATHS;

/// Wire-protocol version for framed JSON walk requests.
pub(crate) const WALK_PROTOCOL_VERSION: u32 = 13;

/// Semantic version for the currently admitted transition graph slice.
pub(crate) const TRANSITION_GRAPH_VERSION: &str = "walk-r0-r14a-v2";

/// Identity of the source/binary snapshot that owns one walk server instance.
///
/// Clients include their own freshly captured epoch on mutating requests. The
/// server compares that client epoch with both its startup epoch and the current
/// filesystem state before it advances typestate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerEpoch {
    pub protocol_version: u32,
    pub transition_graph_version: String,
    pub repo_root: PathBuf,
    pub exe_path: PathBuf,
    pub exe_modified_unix_ms: Option<u64>,
    pub git_head: Option<String>,
    #[serde(default)]
    pub active_branch: Option<String>,
    pub source_status_hash: Option<String>,
    /// Honest-sibling compatibility key for the serialized walk contract.
    ///
    /// This is intentionally not a digest of server-owned runtime engines.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub build_fingerprint: String,
}

impl ServerEpoch {
    /// Capture the epoch for `repo_root` and the currently running executable.
    pub(crate) fn capture(repo_root: &Path) -> Result<Self, PrepareError> {
        let repo_root = repo_root
            .canonicalize()
            .map_err(|source| PrepareError::ReadManifest {
                path: repo_root.to_path_buf(),
                source,
            })?;
        let (exe_path, exe_modified_unix_ms) = running_executable()?;
        let git_head = git_output(&repo_root, &["rev-parse", "HEAD"]);
        let active_branch = git_output(&repo_root, &["symbolic-ref", "--short", "HEAD"]);
        let source_status_hash = source_status_hash(&repo_root)?;
        Ok(Self {
            protocol_version: WALK_PROTOCOL_VERSION,
            transition_graph_version: TRANSITION_GRAPH_VERSION.to_string(),
            repo_root,
            exe_path,
            exe_modified_unix_ms,
            git_head,
            active_branch,
            source_status_hash,
            build_fingerprint: current_build_fingerprint().to_string(),
        })
    }

    /// Validate that a mutating request came from a compatible client build.
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
        self.ensure_executable_path_unchanged()?;
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
        if request_epoch.repo_root != self.repo_root {
            return Err(stale_error(format!(
                "walk repository root mismatch: client='{}' server='{}'",
                request_epoch.repo_root.display(),
                self.repo_root.display()
            )));
        }
        if self.build_fingerprint.is_empty()
            || request_epoch.build_fingerprint != self.build_fingerprint
        {
            return Err(stale_error(
                "walk server build fingerprint differs from client build fingerprint; rebuild the clients and restart the walk server",
            ));
        }
        if request_epoch.git_head != self.git_head
            || request_epoch.active_branch != self.active_branch
            || request_epoch.source_status_hash != self.source_status_hash
        {
            return Err(stale_error(
                "walk server source epoch differs from client source epoch; restart the walk server",
            ));
        }
        Ok(())
    }

    fn ensure_executable_path_unchanged(&self) -> Result<(), PrepareError> {
        let disk_modified = fs::metadata(&self.exe_path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64);
        if disk_modified.is_none() || disk_modified != self.exe_modified_unix_ms {
            return Err(stale_error(
                "walk server executable path changed since startup; restart the walk server",
            ));
        }
        Ok(())
    }

    /// Re-capture the current epoch and reject mutation if the server went stale.
    pub(crate) fn ensure_not_stale_now(&self) -> Result<(), PrepareError> {
        let current = Self::capture(&self.repo_root)?;
        if current.protocol_version != self.protocol_version
            || current.transition_graph_version != self.transition_graph_version
            || current.exe_path != self.exe_path
            || current.exe_modified_unix_ms != self.exe_modified_unix_ms
            || current.git_head != self.git_head
            || current.active_branch != self.active_branch
            || current.source_status_hash != self.source_status_hash
        {
            return Err(stale_error(
                "walk server source/binary changed since startup; restart the walk server",
            ));
        }
        Ok(())
    }
}

pub(crate) fn current_build_fingerprint() -> &'static str {
    env!("PLOKE_WALK_BUILD_FINGERPRINT")
}

fn running_executable() -> Result<(PathBuf, Option<u64>), PrepareError> {
    let path = std::env::current_exe().map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_current_exe",
        detail: source.to_string(),
    })?;
    #[cfg(target_os = "linux")]
    let (path, metadata) = {
        let (metadata, live_inode) = match fs::metadata("/proc/self/exe") {
            Ok(metadata) => (Some(metadata), true),
            Err(_) => (fs::metadata(&path).ok(), false),
        };
        let mut path = path;
        // Cargo replaces the shared output path while the predecessor still
        // owns its running inode. Normalize only when procfs proves that live
        // inode and the logical path is provably absent; the inode mtime below
        // remains the binary-compatibility guard.
        const DELETED: &[u8] = b" (deleted)";
        let raw = path.as_os_str().as_bytes();
        if live_inode && matches!(path.try_exists(), Ok(false)) && raw.ends_with(DELETED) {
            path = raw_path(&raw[..raw.len() - DELETED.len()]);
        }
        (path, metadata)
    };
    #[cfg(not(target_os = "linux"))]
    let metadata = fs::metadata(&path).ok();

    let modified = metadata
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64);
    Ok((path, modified))
}

fn source_status_hash(repo_root: &Path) -> Result<Option<String>, PrepareError> {
    let mut status_args = vec![
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--",
    ];
    status_args.extend(SOURCE_GUARD_PATHS.iter().copied());
    let Some(status) = git_bytes(repo_root, &status_args)? else {
        return Ok(None);
    };

    let mut index_args = vec!["ls-files", "-s", "-z", "--"];
    index_args.extend(SOURCE_GUARD_PATHS.iter().copied());
    let index = git_bytes(repo_root, &index_args)?.ok_or_else(|| {
        epoch_error(
            "prototype1_state_walk_source_index",
            "git ls-files failed after repository status succeeded",
        )
    })?;

    let mut path_args = vec![
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
    ];
    path_args.extend(SOURCE_GUARD_PATHS.iter().copied());
    let raw_paths = git_bytes(repo_root, &path_args)?.ok_or_else(|| {
        epoch_error(
            "prototype1_state_walk_source_paths",
            "git ls-files failed after repository status succeeded",
        )
    })?;

    let mut paths = raw_paths
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();

    let mut hasher = Sha256::new();
    hash_part(&mut hasher, b"status", &status);
    hash_part(&mut hasher, b"index", &index);
    for raw in paths {
        hash_part(&mut hasher, b"path", &raw);
        hash_path(&mut hasher, repo_root, &raw)?;
    }
    let digest = hasher.finalize();
    Ok(Some(
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    ))
}

fn hash_part(hasher: &mut Sha256, label: &[u8], value: &[u8]) {
    hasher.update((label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn hash_path(hasher: &mut Sha256, repo_root: &Path, raw: &[u8]) -> Result<(), PrepareError> {
    let relative = raw_path(raw);
    let path = repo_root.join(&relative);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            hash_part(hasher, b"kind", b"missing");
            return Ok(());
        }
        Err(source) => {
            return Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_source_stat",
                detail: format!(
                    "failed to stat guarded source '{}': {source}",
                    path.display()
                ),
            });
        }
    };

    #[cfg(unix)]
    hash_part(hasher, b"mode", &metadata.mode().to_le_bytes());
    #[cfg(not(unix))]
    hash_part(
        hasher,
        b"readonly",
        &[u8::from(metadata.permissions().readonly())],
    );

    if metadata.file_type().is_symlink() {
        let target = fs::read_link(&path).map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_source_link",
            detail: format!(
                "failed to read guarded source symlink '{}': {source}",
                path.display()
            ),
        })?;
        hash_part(hasher, b"kind", b"symlink");
        hash_os(hasher, b"target", target.as_os_str());
    } else if metadata.is_file() {
        let contents = fs::read(&path).map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_source_read",
            detail: format!(
                "failed to read guarded source '{}': {source}",
                path.display()
            ),
        })?;
        hash_part(hasher, b"kind", b"file");
        hash_part(hasher, b"contents", &contents);
    } else {
        hash_part(hasher, b"kind", b"other");
    }
    Ok(())
}

fn hash_os(hasher: &mut Sha256, label: &[u8], value: &std::ffi::OsStr) {
    #[cfg(unix)]
    hash_part(hasher, label, value.as_bytes());
    #[cfg(not(unix))]
    hash_part(hasher, label, value.to_string_lossy().as_bytes());
}

fn raw_path(raw: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        PathBuf::from(OsString::from_vec(raw.to_vec()))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(raw).into_owned())
    }
}

fn git_bytes(repo_root: &Path, args: &[&str]) -> Result<Option<Vec<u8>>, PrepareError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_git_probe",
            detail: source.to_string(),
        })?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(output.stdout))
}

fn epoch_error(phase: &'static str, detail: impl Into<String>) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase,
        detail: detail.into(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const EXE_HELPER: &str = "PLOKE_EPOCH_EXE_HELPER";

    fn run_git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn epoch_hashes_content() {
        let repo = tempfile::tempdir().expect("temp repo");
        let root = repo.path();
        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "user.name", "Epoch Test"]);
        run_git(root, &["config", "user.email", "epoch@example.invalid"]);
        fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write baseline");
        run_git(root, &["add", "Cargo.toml"]);
        run_git(root, &["commit", "-qm", "baseline"]);

        fs::write(root.join("Cargo.toml"), "[workspace]\n# dirty a\n").expect("first edit");
        let first = source_status_hash(root)
            .expect("first hash")
            .expect("git repository hash");
        fs::write(root.join("Cargo.toml"), "[workspace]\n# dirty b\n").expect("second edit");
        let second = source_status_hash(root)
            .expect("second hash")
            .expect("git repository hash");
        assert_ne!(
            first, second,
            "changing bytes in an already-dirty guarded file must change the epoch"
        );

        let source = root.join("crates/ploke-eval/src/cli/prototype1_state");
        fs::create_dir_all(&source).expect("create source path");
        let untracked = source.join("guard.rs");
        fs::write(&untracked, "const VALUE: u8 = 1;\n").expect("first untracked edit");
        let first = source_status_hash(root)
            .expect("first untracked hash")
            .expect("git repository hash");
        fs::write(&untracked, "const VALUE: u8 = 2;\n").expect("second untracked edit");
        let second = source_status_hash(root)
            .expect("second untracked hash")
            .expect("git repository hash");
        assert_ne!(
            first, second,
            "changing bytes in an already-untracked guarded file must change the epoch"
        );
    }

    #[test]
    fn epoch_binds_normalized_repository_root() {
        let repo = tempfile::tempdir().expect("temp repo");
        let nested = repo.path().join("nested");
        fs::create_dir(&nested).expect("create nested path");
        let epoch = ServerEpoch::capture(&nested.join("..")).expect("capture normalized epoch");
        assert_eq!(
            epoch.repo_root,
            repo.path().canonicalize().expect("canonical repo root")
        );

        let other = tempfile::tempdir().expect("other repo");
        let mut request = epoch.clone();
        request.repo_root = other.path().canonicalize().expect("canonical other root");
        let error = epoch
            .ensure_compatible_request(Some(&request))
            .expect_err("different repository roots must be incompatible");
        assert!(
            error.to_string().contains("repository root mismatch"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn deleted_executable_helper() {
        let Some(repo) = std::env::var_os(EXE_HELPER) else {
            return;
        };
        let repo = PathBuf::from(repo);
        let ready = repo.join("ready");
        let go = repo.join("go");
        let result = repo.join("epoch.json");
        fs::write(&ready, []).expect("publish helper readiness");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !go.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "parent did not replace the helper executable"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let epoch = ServerEpoch::capture(&repo).expect("capture replaced executable epoch");
        let guard_error = epoch
            .ensure_compatible_request(Some(&epoch))
            .expect_err("replaced logical executable path must reject mutation");
        fs::write(
            result,
            serde_json::to_vec(&epoch).expect("encode captured epoch"),
        )
        .expect("persist captured epoch");
        fs::write(repo.join("path-guard-error.txt"), guard_error.to_string())
            .expect("persist executable path guard error");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn epoch_tracks_running_executable_after_path_replacement() {
        let temp = tempfile::tempdir().expect("epoch replacement tempdir");
        let repo = temp.path();
        let source = std::env::current_exe().expect("resolve current test executable");
        let copied = repo.join("epoch-test");
        fs::copy(&source, &copied).expect("copy test executable");
        let modified = fs::metadata(&copied)
            .expect("copied executable metadata")
            .modified()
            .expect("copied executable mtime")
            .duration_since(UNIX_EPOCH)
            .expect("copied executable timestamp")
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        let mut child = Command::new(&copied)
            .arg("--exact")
            .arg("cli::prototype1_state::walk::epoch::tests::deleted_executable_helper")
            .arg("--nocapture")
            .env(EXE_HELPER, repo)
            .spawn()
            .expect("spawn copied test executable");
        let ready = repo.join("ready");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !ready.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "copied test executable did not become ready"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        fs::remove_file(&copied).expect("unlink running test executable");
        let mut replacement_mtime = modified;
        for attempt in 0..100 {
            fs::write(&copied, format!("replacement-{attempt}")).expect("replace executable path");
            replacement_mtime = fs::metadata(&copied)
                .expect("replacement executable metadata")
                .modified()
                .expect("replacement executable mtime")
                .duration_since(UNIX_EPOCH)
                .expect("replacement executable timestamp")
                .as_millis()
                .min(u128::from(u64::MAX)) as u64;
            if replacement_mtime != modified {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_ne!(
            replacement_mtime, modified,
            "replacement executable must have a distinct modification time"
        );
        fs::write(repo.join("go"), []).expect("release helper capture");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().expect("poll copied test executable") {
                break status;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "copied test executable did not exit"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert!(status.success(), "copied test executable failed: {status}");

        let epoch: ServerEpoch = serde_json::from_slice(
            &fs::read(repo.join("epoch.json")).expect("read captured epoch"),
        )
        .expect("decode captured epoch");
        assert_eq!(epoch.exe_path, copied);
        assert_eq!(epoch.exe_modified_unix_ms, Some(modified));
        let guard_error =
            fs::read_to_string(repo.join("path-guard-error.txt")).expect("read path guard error");
        assert!(
            guard_error.contains("executable path changed"),
            "unexpected compatibility error: {guard_error}"
        );
    }

    #[test]
    fn request_compatibility_accepts_sibling_executable_identity() {
        let repo = tempfile::tempdir().expect("epoch sibling tempdir");
        let epoch = ServerEpoch::capture(repo.path()).expect("capture server epoch");
        let mut client = epoch.clone();
        client.exe_path = repo.path().join("sibling-ploke-walk-ui");
        client.exe_modified_unix_ms = epoch
            .exe_modified_unix_ms
            .map(|mtime| mtime.wrapping_add(1));

        epoch
            .ensure_compatible_request(Some(&client))
            .expect("sibling executable with the same build fingerprint must be compatible");
    }

    #[test]
    fn request_compatibility_rejects_mismatched_build_fingerprint() {
        let repo = tempfile::tempdir().expect("epoch fingerprint tempdir");
        let epoch = ServerEpoch::capture(repo.path()).expect("capture server epoch");
        let mut client = epoch.clone();
        client.build_fingerprint = "0".repeat(64);

        let error = epoch
            .ensure_compatible_request(Some(&client))
            .expect_err("different guarded-source builds must be incompatible");
        assert!(
            error.to_string().contains("build fingerprint differs"),
            "unexpected compatibility error: {error}"
        );

        client.build_fingerprint.clear();
        let error = epoch
            .ensure_compatible_request(Some(&client))
            .expect_err("missing guarded-source build identity must be incompatible");
        assert!(
            error.to_string().contains("build fingerprint differs"),
            "unexpected missing-fingerprint error: {error}"
        );
    }

    #[test]
    fn request_compatibility_rejects_server_executable_path_drift() {
        let repo = tempfile::tempdir().expect("epoch executable tempdir");
        let mut epoch = ServerEpoch::capture(repo.path()).expect("capture server epoch");
        let client = epoch.clone();
        epoch.exe_modified_unix_ms = Some(
            epoch
                .exe_modified_unix_ms
                .expect("test executable has a modification time")
                .wrapping_add(1),
        );

        let error = epoch
            .ensure_compatible_request(Some(&client))
            .expect_err("changed server executable path identity must be stale");
        assert!(
            error.to_string().contains("executable path changed"),
            "unexpected compatibility error: {error}"
        );
    }
}
