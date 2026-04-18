use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::inner::core::{FrozenRunSpec, RunIntent, RunStorageRoots};

/// Schema version for run registration records.
pub const RUN_REGISTRATION_SCHEMA_VERSION: &str = "run-registration.v1";

/// Persisted authority record for one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRegistration {
    /// Registration schema version.
    pub schema_version: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Original caller intent.
    pub intent: RunIntent,
    /// Frozen executable specification.
    pub frozen_spec: FrozenRunSpec,
    /// Stable fingerprint of the frozen spec.
    pub spec_fingerprint: String,
}

/// Errors produced while building or persisting a registration.
#[derive(Debug, Error)]
pub enum RunRegistrationError {
    #[error("failed to fingerprint frozen run spec: {source}")]
    Fingerprint { source: serde_json::Error },
    #[error("failed to serialize run registration: {source}")]
    Serialize { source: serde_json::Error },
    #[error("failed to create directory '{path}': {source}")]
    CreateDir { path: PathBuf, source: std::io::Error },
    #[error("failed to write run registration '{path}': {source}")]
    Write { path: PathBuf, source: std::io::Error },
    #[error("failed to read run registration '{path}': {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("failed to parse run registration '{path}': {source}")]
    Parse { path: PathBuf, source: serde_json::Error },
}

impl RunRegistration {
    /// Build a registration from caller intent.
    pub fn register(intent: RunIntent) -> Result<Self, RunRegistrationError> {
        let frozen_spec = intent.freeze();
        let spec_fingerprint = frozen_spec
            .fingerprint()
            .map_err(|source| RunRegistrationError::Fingerprint { source })?;
        let run_id = run_id_from_fingerprint(&frozen_spec.task_id, &spec_fingerprint);

        Ok(Self {
            schema_version: RUN_REGISTRATION_SCHEMA_VERSION.to_string(),
            run_id,
            intent,
            frozen_spec,
            spec_fingerprint,
        })
    }

    fn storage_roots(&self) -> &RunStorageRoots {
        &self.frozen_spec.storage_roots
    }

    /// Canonical registration file path.
    pub fn registry_path(&self) -> PathBuf {
        self.storage_roots().registry_path(&self.run_id)
    }

    /// Canonical per-run artifact directory.
    pub fn run_root(&self) -> PathBuf {
        self.storage_roots().run_root(&self.run_id)
    }

    /// Canonical run record path.
    pub fn record_path(&self) -> PathBuf {
        self.storage_roots().record_path(&self.run_id)
    }

    /// Canonical final DB attachment path.
    pub fn final_db_path(&self) -> PathBuf {
        self.storage_roots().final_db_path(&self.run_id)
    }

    /// Persist the registration to its canonical registry path.
    pub fn persist(&self) -> Result<(), RunRegistrationError> {
        let registry_path = self.registry_path();
        let registry_dir = registry_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.storage_roots().registries_dir.join("runs"));
        let run_root = self.run_root();

        fs::create_dir_all(&registry_dir).map_err(|source| RunRegistrationError::CreateDir {
            path: registry_dir.clone(),
            source,
        })?;
        fs::create_dir_all(&run_root).map_err(|source| RunRegistrationError::CreateDir {
            path: run_root.clone(),
            source,
        })?;

        let json =
            serde_json::to_string_pretty(self).map_err(|source| RunRegistrationError::Serialize {
                source,
            })?;
        fs::write(&registry_path, json).map_err(|source| RunRegistrationError::Write {
            path: registry_path,
            source,
        })
    }

    /// Load a registration from a canonical registry path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RunRegistrationError> {
        let path = path.as_ref().to_path_buf();
        let json = fs::read_to_string(&path).map_err(|source| RunRegistrationError::Read {
            path: path.clone(),
            source,
        })?;
        serde_json::from_str(&json).map_err(|source| RunRegistrationError::Parse { path, source })
    }

    /// Discover a registration by run id from canonical storage roots.
    pub fn discover(
        storage_roots: &RunStorageRoots,
        run_id: &str,
    ) -> Result<Self, RunRegistrationError> {
        Self::load(storage_roots.registry_path(run_id))
    }
}

fn run_id_from_fingerprint(task_id: &str, spec_fingerprint: &str) -> String {
    let short = &spec_fingerprint[..12];
    format!("run-{}-{short}", sanitize_task_id(task_id))
}

fn sanitize_task_id(task_id: &str) -> String {
    let mut out = String::with_capacity(task_id.len());
    let mut pending_dash = false;

    for ch in task_id.chars() {
        let mapped = if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            ch
        } else {
            '-'
        };

        if mapped == '-' {
            if !pending_dash {
                out.push(mapped);
                pending_dash = true;
            }
        } else {
            out.push(mapped);
            pending_dash = false;
        }
    }

    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "run".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::EvalBudget;
    use tempfile::tempdir;

    fn sample_intent(base: &Path) -> RunIntent {
        RunIntent {
            task_id: "BurntSushi__ripgrep-2209".to_string(),
            repo_root: base.join("repo"),
            storage_roots: RunStorageRoots::new(base.join("registries"), base.join("runs")),
            base_sha: Some("deadbeef".to_string()),
            budget: EvalBudget {
                max_turns: 8,
                max_tool_calls: 20,
                wall_clock_secs: 600,
            },
            model_id: Some("anthropic/claude-sonnet-4".to_string()),
            provider_slug: Some("openrouter".to_string()),
        }
    }

    #[test]
    fn stable_run_id_generation_matches_identical_intent() {
        let tmp = tempdir().expect("tempdir");
        let first = RunRegistration::register(sample_intent(tmp.path())).expect("register");
        let second = RunRegistration::register(sample_intent(tmp.path())).expect("register");

        assert_eq!(first.run_id, second.run_id);
        assert_eq!(first.spec_fingerprint, second.spec_fingerprint);
        assert!(first.run_id.starts_with("run-BurntSushi__ripgrep-2209-"));
    }

    #[test]
    fn artifact_roots_are_derived_from_run_id() {
        let tmp = tempdir().expect("tempdir");
        let intent = sample_intent(tmp.path());
        let roots = intent.storage_roots.clone();
        let registration = RunRegistration::register(intent).expect("register");

        assert_eq!(
            registration.registry_path(),
            roots
                .registries_dir
                .join("runs")
                .join(format!("{}.json", registration.run_id))
        );
        assert_eq!(registration.run_root(), roots.runs_dir.join(&registration.run_id));
        assert_eq!(
            registration.record_path(),
            roots
                .runs_dir
                .join(&registration.run_id)
                .join("record.json.gz")
        );
        assert_eq!(
            registration.final_db_path(),
            roots
                .runs_dir
                .join(&registration.run_id)
                .join("state-final.db")
        );
    }

    #[test]
    fn registration_round_trips_through_discovery() {
        let tmp = tempdir().expect("tempdir");
        let intent = sample_intent(tmp.path());
        let roots = intent.storage_roots.clone();
        let registration = RunRegistration::register(intent).expect("register");

        registration.persist().expect("persist");
        let loaded = RunRegistration::discover(&roots, &registration.run_id).expect("discover");

        assert_eq!(loaded, registration);
        assert!(registration.registry_path().exists());
        assert!(registration.run_root().exists());
    }

    #[test]
    fn registration_read_and_write_failures_report_path_provenance() {
        let tmp = tempdir().expect("tempdir");
        let intent = sample_intent(tmp.path());
        let roots = intent.storage_roots.clone();
        let registration = RunRegistration::register(intent).expect("register");

        let read_path = roots.registry_path("missing-run");
        let read_err = RunRegistration::load(&read_path).expect_err("load should fail");
        match read_err {
            RunRegistrationError::Read { path, .. } => assert_eq!(path, read_path),
            other => panic!("expected read error, got {other:?}"),
        }

        std::fs::create_dir_all(registration.registry_path()).expect("stub registry path");
        let write_err = registration.persist().expect_err("persist should fail");
        match write_err {
            RunRegistrationError::Write { path, .. } => {
                assert_eq!(path, registration.registry_path())
            }
            other => panic!("expected write error, got {other:?}"),
        }
    }
}
