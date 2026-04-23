use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::inner::core::{FrozenRunSpec, RegisteredRunRole, RunIntent, RunStorageRoots};

/// Schema version for run registration records.
pub const RUN_REGISTRATION_SCHEMA_VERSION: &str = "run-registration.v2";

/// Coarse execution status for one concrete run attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunExecutionStatus {
    Registered,
    Running,
    Completed,
    Failed,
}

/// Phase-local lifecycle status within one run attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunPhaseStatus {
    NotStarted,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

/// Coarse submission outcome for benchmark packaging output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunSubmissionStatus {
    Missing,
    EmptyPatch,
    NonemptyPatch,
}

/// Stable artifact references for one run attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunArtifactRefs {
    pub run_manifest: PathBuf,
    pub run_root: PathBuf,
    pub repo_state: PathBuf,
    pub execution_log: PathBuf,
    pub indexing_status: PathBuf,
    pub parse_failure: PathBuf,
    pub snapshot_status: PathBuf,
    pub indexing_checkpoint_db: PathBuf,
    pub indexing_failure_db: PathBuf,
    pub record_path: PathBuf,
    pub final_snapshot: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_trace: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_summary: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_response_trace: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msb_submission: Option<PathBuf>,
    pub protocol_artifacts_dir: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_anchor: Option<PathBuf>,
}

/// Lifecycle state for one named execution phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPhaseLifecycle {
    pub status: RunPhaseStatus,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Canonical lifecycle state for one run attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunLifecycle {
    pub created_at: String,
    pub updated_at: String,
    pub execution_status: RunExecutionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    pub setup: RunPhaseLifecycle,
    pub patching: RunPhaseLifecycle,
    pub validation: RunPhaseLifecycle,
    pub packaging: RunPhaseLifecycle,
    pub protocol: RunPhaseLifecycle,
    pub submission_status: RunSubmissionStatus,
}

/// Persisted authority record for one run.
///
/// 2026-04-21 transition note:
/// `RunRegistration` is the canonical authority surface for run identity,
/// lifecycle state, and artifact refs. General CLI resolution should use
/// registrations and per-attempt `runs/run-*` dirs, not legacy top-level
/// instance-root artifacts.
///
/// New authority path:
/// - this type: `inner::registry::RunRegistration`
/// - live registry helpers: `run_registry.rs`
/// - runner writes/updates: `runner.rs`
/// - authority-first readers: `run_history.rs`, `closure.rs`, campaign export,
///   and protocol artifact sync
///
/// Remaining compatibility is limited to explicit record-path consumers and the
/// compatibility record layer in `record.rs` around `RunRecord` / `record.json.gz`.
/// Legacy top-level instance-root artifacts may still exist on disk from older
/// runs, but they are not part of the authoritative discovery path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRegistration {
    /// Registration schema version.
    pub schema_version: String,
    /// Stable run identifier for this concrete attempt.
    pub run_id: String,
    /// Original caller intent.
    pub intent: RunIntent,
    /// Frozen executable specification.
    pub frozen_spec: FrozenRunSpec,
    /// Stable fingerprint of the frozen spec.
    pub spec_fingerprint: String,
    /// Canonical lifecycle state for this run attempt.
    pub lifecycle: RunLifecycle,
    /// Canonical artifact references for this run attempt.
    pub artifacts: RunArtifactRefs,
}

/// Errors produced while building or persisting a registration.
#[derive(Debug, Error)]
pub enum RunRegistrationError {
    #[error("failed to fingerprint frozen run spec: {source}")]
    Fingerprint { source: serde_json::Error },
    #[error("failed to serialize run registration: {source}")]
    Serialize { source: serde_json::Error },
    #[error("failed to create directory '{path}': {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write run registration '{path}': {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read run registration '{path}': {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse run registration '{path}': {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl RunRegistration {
    /// Build a registration from caller intent with a fingerprint-derived run id.
    pub fn register(intent: RunIntent) -> Result<Self, RunRegistrationError> {
        let frozen_spec = intent.freeze();
        let spec_fingerprint = frozen_spec
            .fingerprint()
            .map_err(|source| RunRegistrationError::Fingerprint { source })?;
        let run_id = run_id_from_fingerprint(&frozen_spec.task_id, &spec_fingerprint);
        Self::build(intent, frozen_spec, spec_fingerprint, run_id)
    }

    /// Build a registration for a concrete run attempt with an explicit run id.
    pub fn register_with_run_id(
        intent: RunIntent,
        run_id: impl Into<String>,
    ) -> Result<Self, RunRegistrationError> {
        let frozen_spec = intent.freeze();
        let spec_fingerprint = frozen_spec
            .fingerprint()
            .map_err(|source| RunRegistrationError::Fingerprint { source })?;
        Self::build(intent, frozen_spec, spec_fingerprint, run_id.into())
    }

    fn build(
        intent: RunIntent,
        frozen_spec: FrozenRunSpec,
        spec_fingerprint: String,
        run_id: String,
    ) -> Result<Self, RunRegistrationError> {
        let artifacts = RunArtifactRefs::from_frozen_spec(&frozen_spec, &run_id);
        let lifecycle = RunLifecycle::new(initial_patching_state(frozen_spec.run_role));
        Ok(Self {
            schema_version: RUN_REGISTRATION_SCHEMA_VERSION.to_string(),
            run_id,
            intent,
            frozen_spec,
            spec_fingerprint,
            lifecycle,
            artifacts,
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

    /// Mark the run attempt as actively executing.
    pub fn mark_execution_started(&mut self, detail: impl Into<Option<String>>) {
        let now = now_rfc3339();
        self.lifecycle.execution_status = RunExecutionStatus::Running;
        self.lifecycle.started_at = Some(now.clone());
        self.lifecycle.updated_at = now.clone();
        self.lifecycle.failure = None;
        self.lifecycle.setup = RunPhaseLifecycle {
            status: RunPhaseStatus::InProgress,
            updated_at: now,
            detail: detail.into(),
        };
    }

    /// Update one lifecycle phase.
    pub fn update_phase(
        &mut self,
        phase: RunLifecyclePhase,
        status: RunPhaseStatus,
        detail: impl Into<Option<String>>,
    ) {
        let now = now_rfc3339();
        let phase_state = RunPhaseLifecycle {
            status,
            updated_at: now.clone(),
            detail: detail.into(),
        };
        match phase {
            RunLifecyclePhase::Setup => self.lifecycle.setup = phase_state,
            RunLifecyclePhase::Patching => self.lifecycle.patching = phase_state,
            RunLifecyclePhase::Validation => self.lifecycle.validation = phase_state,
            RunLifecyclePhase::Packaging => self.lifecycle.packaging = phase_state,
            RunLifecyclePhase::Protocol => self.lifecycle.protocol = phase_state,
        }
        self.lifecycle.updated_at = now;
    }

    /// Mark the overall run as failed and fail any active phase.
    pub fn mark_failed(&mut self, detail: impl Into<String>) {
        let now = now_rfc3339();
        let detail = detail.into();
        self.lifecycle.execution_status = RunExecutionStatus::Failed;
        self.lifecycle.finished_at = Some(now.clone());
        self.lifecycle.updated_at = now.clone();
        self.lifecycle.failure = Some(detail.clone());
        self.fail_active_phase(detail, now);
    }

    /// Mark the run as completed successfully.
    pub fn mark_completed(&mut self) {
        let now = now_rfc3339();
        self.lifecycle.execution_status = RunExecutionStatus::Completed;
        self.lifecycle.finished_at = Some(now.clone());
        self.lifecycle.updated_at = now;
        self.lifecycle.failure = None;
    }

    /// Update the submission status using one concrete patch payload.
    pub fn update_submission_status(&mut self, fix_patch: Option<&str>) {
        self.lifecycle.submission_status = match fix_patch {
            Some(patch) if !patch.trim().is_empty() => RunSubmissionStatus::NonemptyPatch,
            Some(_) => RunSubmissionStatus::EmptyPatch,
            None => RunSubmissionStatus::Missing,
        };
        self.lifecycle.updated_at = now_rfc3339();
    }

    /// Update the latest protocol anchor path and status.
    pub fn update_protocol_anchor(&mut self, path: Option<PathBuf>) {
        self.artifacts.protocol_anchor = path;
        self.lifecycle.updated_at = now_rfc3339();
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

        let json = serde_json::to_string_pretty(self)
            .map_err(|source| RunRegistrationError::Serialize { source })?;
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

    fn fail_active_phase(&mut self, detail: String, now: String) {
        for phase in [
            &mut self.lifecycle.protocol,
            &mut self.lifecycle.packaging,
            &mut self.lifecycle.validation,
            &mut self.lifecycle.patching,
            &mut self.lifecycle.setup,
        ] {
            if phase.status == RunPhaseStatus::InProgress {
                phase.status = RunPhaseStatus::Failed;
                phase.detail = Some(detail);
                phase.updated_at = now;
                return;
            }
        }
        self.lifecycle.setup.status = RunPhaseStatus::Failed;
        self.lifecycle.setup.detail = Some(detail);
        self.lifecycle.setup.updated_at = now;
    }
}

/// Named phase within the run lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunLifecyclePhase {
    Setup,
    Patching,
    Validation,
    Packaging,
    Protocol,
}

impl RunArtifactRefs {
    fn from_frozen_spec(frozen_spec: &FrozenRunSpec, run_id: &str) -> Self {
        let run_root = frozen_spec.storage_roots.run_root(run_id);
        Self {
            run_manifest: run_root
                .parent()
                .and_then(|parent| parent.parent())
                .unwrap_or(&run_root)
                .join("run.json"),
            run_root: run_root.clone(),
            repo_state: run_root.join("repo-state.json"),
            execution_log: run_root.join("execution-log.json"),
            indexing_status: run_root.join("indexing-status.json"),
            parse_failure: run_root.join("parse-failure.json"),
            snapshot_status: run_root.join("snapshot-status.json"),
            indexing_checkpoint_db: run_root.join("indexing-checkpoint.db"),
            indexing_failure_db: run_root.join("indexing-failure.db"),
            record_path: run_root.join("record.json.gz"),
            final_snapshot: run_root.join("final-snapshot.db"),
            turn_trace: None,
            turn_summary: None,
            full_response_trace: None,
            msb_submission: None,
            protocol_artifacts_dir: crate::layout::protocol_artifacts_dir_for_run(&run_root),
            protocol_anchor: None,
        }
    }
}

impl RunLifecycle {
    fn new(initial_patching_state: RunPhaseStatus) -> Self {
        let now = now_rfc3339();
        Self {
            created_at: now.clone(),
            updated_at: now.clone(),
            execution_status: RunExecutionStatus::Registered,
            started_at: None,
            finished_at: None,
            failure: None,
            setup: RunPhaseLifecycle::new(RunPhaseStatus::NotStarted, now.clone()),
            patching: RunPhaseLifecycle::new(initial_patching_state, now.clone()),
            validation: RunPhaseLifecycle::new(RunPhaseStatus::NotStarted, now.clone()),
            packaging: RunPhaseLifecycle::new(RunPhaseStatus::NotStarted, now.clone()),
            protocol: RunPhaseLifecycle::new(RunPhaseStatus::NotStarted, now),
            submission_status: RunSubmissionStatus::Missing,
        }
    }
}

impl RunPhaseLifecycle {
    fn new(status: RunPhaseStatus, updated_at: String) -> Self {
        Self {
            status,
            updated_at,
            detail: None,
        }
    }
}

fn initial_patching_state(role: RegisteredRunRole) -> RunPhaseStatus {
    match role {
        RegisteredRunRole::Control => RunPhaseStatus::Skipped,
        RegisteredRunRole::Treatment => RunPhaseStatus::NotStarted,
    }
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
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
            campaign_id: Some("baseline-smoke".to_string()),
            batch_id: Some("ripgrep-2209".to_string()),
            run_arm_id: "structured-current-policy".to_string(),
            run_role: RegisteredRunRole::Treatment,
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
        assert_eq!(
            registration.run_root(),
            roots.runs_dir.join(&registration.run_id)
        );
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
        assert_eq!(
            registration.artifacts.protocol_artifacts_dir,
            registration.run_root().join("protocol-artifacts")
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
    fn explicit_run_id_allows_multiple_attempts_for_same_spec() {
        let tmp = tempdir().expect("tempdir");
        let one = RunRegistration::register_with_run_id(sample_intent(tmp.path()), "run-a")
            .expect("register one");
        let two = RunRegistration::register_with_run_id(sample_intent(tmp.path()), "run-b")
            .expect("register two");

        assert_ne!(one.run_id, two.run_id);
        assert_eq!(one.spec_fingerprint, two.spec_fingerprint);
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
