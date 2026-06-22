//! Prototype 1 checkpoint fixture manifest helpers.
#![cfg_attr(not(test), allow(dead_code))]
//!
//! Checkpoints are test/operator fixtures for resuming consumers at known
//! boundaries. They are not authority: restoring a checkpoint must still leave
//! History, channel, MessageBox, bootstrap, and artifact gates intact.

use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CheckpointManifest {
    pub(crate) schema_version: String,
    pub(crate) checkpoint: String,
    pub(crate) created_by: String,
    pub(crate) campaign_id: String,
    pub(crate) parent_id: String,
    pub(crate) boundary: String,
    #[serde(rename = "requires_live_api_to_regenerate")]
    pub(crate) needs_live_regen: bool,
    #[serde(default)]
    pub(crate) files: Vec<CheckpointFile>,
    #[serde(default)]
    pub(crate) db_snapshots: Vec<CheckpointDb>,
    pub(crate) authority_surfaces: AuthoritySurfaces,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CheckpointFile {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) required: bool,
    pub(crate) authority_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CheckpointDb {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) schema_version: String,
    pub(crate) required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AuthoritySurfaces {
    #[serde(default)]
    pub(crate) history: Vec<String>,
    #[serde(default)]
    pub(crate) channels: Vec<String>,
    #[serde(default)]
    pub(crate) message_boxes: Vec<String>,
    #[serde(default)]
    pub(crate) artifacts: Vec<String>,
}

#[derive(Debug, Error)]
pub(crate) enum CheckpointError {
    #[error("failed to read checkpoint file '{path}': {source}")]
    Read { path: String, source: io::Error },
    #[error("failed to parse checkpoint manifest '{path}': {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("checkpoint required file is missing: '{path}'")]
    Missing { path: String },
    #[error("checkpoint hash mismatch for '{path}': expected {expected}, got {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
}

impl CheckpointManifest {
    pub(crate) fn read(root: &Path) -> Result<Self, CheckpointError> {
        let path = root.join("manifest.json");
        let data = fs::read_to_string(&path).map_err(|source| CheckpointError::Read {
            path: path.display().to_string(),
            source,
        })?;
        serde_json::from_str(&data).map_err(|source| CheckpointError::Parse {
            path: path.display().to_string(),
            source,
        })
    }

    pub(crate) fn verify_hashes(&self, root: &Path) -> Result<(), CheckpointError> {
        for file in &self.files {
            verify_path(root, &file.path, &file.sha256, file.required)?;
        }
        for db in &self.db_snapshots {
            verify_path(root, &db.path, &db.sha256, db.required)?;
        }
        Ok(())
    }
}

fn verify_path(
    root: &Path,
    relpath: &str,
    expected: &str,
    required: bool,
) -> Result<(), CheckpointError> {
    let path = root.join(relpath);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == io::ErrorKind::NotFound && !required => return Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Err(CheckpointError::Missing {
                path: path.display().to_string(),
            });
        }
        Err(source) => {
            return Err(CheckpointError::Read {
                path: path.display().to_string(),
                source,
            });
        }
    };
    let actual = sha256_hex(&bytes);
    if actual == expected {
        Ok(())
    } else {
        Err(CheckpointError::HashMismatch {
            path: path.display().to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;

    const ROOT: &str = "tests/fixtures/prototype1-checkpoints";

    #[test]
    fn prototype1_checkpoint_seed_manifests_verify_hashes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(ROOT);
        for name in ["F0_setup", "F1_ready_parent"] {
            let dir = root.join(name);
            let manifest = CheckpointManifest::read(&dir).expect("manifest reads");
            assert_eq!(manifest.checkpoint, name);
            assert!(!manifest.needs_live_regen);
            manifest.verify_hashes(&dir).expect("hashes verify");
        }
    }

    #[test]
    fn prototype1_checkpoint_missing_required_file_fails() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = CheckpointManifest {
            schema_version: "prototype1-checkpoint-manifest.v1".to_string(),
            checkpoint: "missing".to_string(),
            created_by: "test".to_string(),
            campaign_id: "campaign".to_string(),
            parent_id: "parent".to_string(),
            boundary: "test".to_string(),
            needs_live_regen: false,
            files: vec![CheckpointFile {
                path: "missing.txt".to_string(),
                sha256: sha256_hex(b"missing"),
                required: true,
                authority_class: "fixture_marker".to_string(),
            }],
            db_snapshots: Vec::new(),
            authority_surfaces: AuthoritySurfaces {
                history: Vec::new(),
                channels: Vec::new(),
                message_boxes: Vec::new(),
                artifacts: Vec::new(),
            },
        };

        let err = manifest
            .verify_hashes(tmp.path())
            .expect_err("missing required file must fail");
        assert!(matches!(err, CheckpointError::Missing { .. }));
    }

    #[test]
    fn prototype1_checkpoint_hash_mismatch_fails() {
        let tmp = tempfile::tempdir().expect("tmp");
        fs::write(tmp.path().join("marker.txt"), b"actual").expect("write marker");
        let manifest = CheckpointManifest {
            schema_version: "prototype1-checkpoint-manifest.v1".to_string(),
            checkpoint: "mismatch".to_string(),
            created_by: "test".to_string(),
            campaign_id: "campaign".to_string(),
            parent_id: "parent".to_string(),
            boundary: "test".to_string(),
            needs_live_regen: false,
            files: vec![CheckpointFile {
                path: "marker.txt".to_string(),
                sha256: sha256_hex(b"expected"),
                required: true,
                authority_class: "fixture_marker".to_string(),
            }],
            db_snapshots: Vec::new(),
            authority_surfaces: AuthoritySurfaces {
                history: Vec::new(),
                channels: Vec::new(),
                message_boxes: Vec::new(),
                artifacts: Vec::new(),
            },
        };

        let err = manifest
            .verify_hashes(tmp.path())
            .expect_err("hash mismatch must fail");
        assert!(matches!(err, CheckpointError::HashMismatch { .. }));
    }
}
